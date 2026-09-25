//! Heritage-clause member compatibility, ported from
//! `tsc/internal/checker/checker.go`.
//!
//! tsc's `issueMemberSpecificError` runs before it reports that a class as a
//! whole is incompatible with something it extends or implements: it walks the
//! class's own non-static members and reports each one that is not assignable
//! to the same-named base member (TS2416). Only when that walk reports nothing
//! does the broad diagnostic fire — which is what makes a *missing* member
//! TS2415/TS2420 and a *mistyped* one TS2416.
//!
//! An interface has no member-specific pass: extending an interface can only
//! fail by overriding a member incompatibly, so that single condition is the
//! whole rule and tsc reports it as TS2430 on the interface name.

use surge_ts_syntax::{
    ParsedClassDeclaration, ParsedClassMember, ParsedInterfaceDeclaration,
    ParsedMemberAccessibility, ParsedNamedType, ParsedType, TextSpan,
};

use crate::symbols::{InterfaceInfo, TypeDeclarationInfo};
use surge_ts_types::{Type, is_assignable_to};

use std::sync::Arc;

use surge_ts_diagnostics::Diagnostic;

use crate::context::{CheckerContext, convert_span};
use crate::infer::map_parsed_type;

/// A member a heritage walk compares: its name, and where tsc anchors an error
/// about it.
pub(crate) struct DeclaredMember {
    pub(crate) name: String,
    pub(crate) name_span: Option<TextSpan>,
}

pub(crate) fn class_instance_members(class: &ParsedClassDeclaration) -> Vec<DeclaredMember> {
    class
        .members
        .iter()
        .filter_map(|member| match member {
            ParsedClassMember::Property(property) if !property.is_static => Some(DeclaredMember {
                name: property.name.clone(),
                name_span: property.name_span,
            }),
            ParsedClassMember::Method(method) if !method.is_static => Some(DeclaredMember {
                name: method.name.clone(),
                name_span: method.name_span,
            }),
            ParsedClassMember::Accessor(accessor) if !accessor.is_static => Some(DeclaredMember {
                name: accessor.name.clone(),
                name_span: accessor.name_span,
            }),
            _ => None,
        })
        .collect()
}

fn named_type(name: &str, span: Option<TextSpan>) -> ParsedType {
    ParsedType::Named(std::sync::Arc::new(ParsedNamedType {
        name: name.to_string(),
        span,
        type_arguments: Vec::new(),
    }))
}

/// The member's type as the receiver's own surface answers it. `None` when the
/// receiver is not an object surface surge resolved, which is what keeps a
/// heritage name it could not model from reading as a mismatch.
fn property_type(receiver: &Type, name: &str) -> Option<Type> {
    match receiver.peeled() {
        Type::Object(object) => object.properties.get(name).map(|property| {
            if property.optional {
                surge_ts_types::union_type(vec![property.ty.clone(), Type::Undefined])
            } else {
                property.ty.clone()
            }
        }),
        _ => None,
    }
}

/// Whether the two types are comparable at all. A modelling failure on either
/// side is not evidence of an incompatible override, and `any` relates to
/// everything, so both are left alone rather than reported. A type variable of
/// the declaration being checked is a type like any other.
fn comparable(source: &Type, target: &Type) -> bool {
    // surge merges an intersection's object operands and drops a tuple or
    // array one (`TType & { [tag]: V }`), so a merged intersection is not the
    // type tsc relates.
    let modelled = |ty: &Type| {
        !ty.is_unmodelled()
            && !matches!(ty, Type::ErrorType | Type::Any)
            && !matches!(ty.peeled(), Type::Object(object) if object.is_intersection)
    };
    modelled(source) && modelled(target)
}

/// The member itself, as a one-member object: a method compares its
/// parameters bivariantly (tsc's `compareSignaturesRelated` for a method
/// declaration), which only the member, not its function type, records. Only
/// the types are related (`issueMemberSpecificError`), so neither a
/// `private`/`protected` restriction nor `readonly` takes part.
fn member_alone(receiver: &Type, name: &str) -> Option<Type> {
    let Type::Object(object) = receiver.peeled() else {
        return None;
    };
    let mut property = object.properties.get(name)?.clone();
    property.restriction = None;
    property.readonly = false;
    let mut properties = surge_ts_types::PropertyMap::default();
    properties.insert(name.into(), property);
    Some(Type::Object(crate::metrics::alloc_object_type(properties, None)))
}

/// Names every declared member of `members` whose type is not assignable to the
/// same-named member of `base_type`, with the span tsc anchors on.
fn incompatible_members(
    members: &[DeclaredMember],
    own_type: &Type,
    base_type: &Type,
) -> Vec<(String, Option<TextSpan>)> {
    let mut incompatible = Vec::new();
    for member in members {
        let (Some(own), Some(base)) = (
            property_type(own_type, &member.name),
            property_type(base_type, &member.name),
        ) else {
            continue;
        };
        if !comparable(&own, &base) {
            continue;
        }
        let assignable = match (
            member_alone(own_type, &member.name),
            member_alone(base_type, &member.name),
        ) {
            (Some(own_member), Some(base_member)) => is_assignable_to(&own_member, &base_member),
            _ => is_assignable_to(&own, &base),
        };
        if !assignable {
            incompatible.push((member.name.clone(), member.name_span));
        }
    }
    incompatible
}

/// TS2416 for every member of `class` that overrides a same-named member of
/// `base` incompatibly. Returns whether anything was reported, which is what
/// gates the broad diagnostic. A generic class is related through its own type
/// variables, and a base written with type arguments through them.
pub(crate) fn report_incompatible_heritage_members(
    class: &ParsedClassDeclaration,
    base: &ParsedNamedType,
    ctx: &mut CheckerContext,
) -> bool {
    if class.is_declare {
        return false;
    }
    if class.type_parameters.is_empty() {
        return report_incompatible_heritage_members_in_scope(class, base, ctx);
    }
    let _type_variables = crate::checks::function::enter_body_type_variables(&class.type_parameters, ctx);
    crate::checks::function::with_type_parameter_scope(&class.type_parameters, ctx, |ctx| {
        report_incompatible_heritage_members_in_scope(class, base, ctx)
    })
}

fn report_incompatible_heritage_members_in_scope(
    class: &ParsedClassDeclaration,
    base: &ParsedNamedType,
    ctx: &mut CheckerContext,
) -> bool {
    let own_arguments: Vec<ParsedType> =
        class.type_parameters.iter().map(|parameter| named_type(&parameter.name, None)).collect();
    // Both are the clause's own lookups, whose failures its own check reports.
    let checkpoint = ctx.diagnostics().len();
    let own_type = map_parsed_type(named_type_with(&class.name, class.name_span, own_arguments), ctx);
    // Resolved as the heritage position resolves it, where a base that names a
    // value (`extends Mixed`) takes its instance type from the constructor.
    let base_type = crate::program::with_dts_expansion_reason(
        crate::program::DtsExpansionReason::InterfaceHeritageResolution,
        || map_parsed_type(named_type_with(&base.name, None, base.type_arguments.clone()), ctx),
    );
    ctx.truncate_diagnostics_releasing_utility_keys(checkpoint);
    let members = class_instance_members(class);
    let incompatible = incompatible_members(&members, &own_type, &base_type);
    if incompatible.is_empty() {
        return false;
    }

    // tsc names the base *type*; for a value base that is the constructed
    // instance, not the identifier in the `extends` clause.
    let base_display = if base.type_arguments.is_empty() && ctx.lookup_type_declaration(&base.name).is_some() {
        base.name.clone()
    } else {
        base_type.name()
    };
    let own_display = type_display(
        &class.name,
        &class.type_parameters.iter().map(|parameter| parameter.name.clone()).collect::<Vec<_>>(),
    );
    for (name, name_span) in incompatible {
        let diagnostic = Diagnostic::ts2416(&name, &own_display, &base_display, ctx.file_name.clone());
        ctx.push(match name_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        });
    }
    true
}

/// tsc's base-type circularity: a class whose `extends` chain leads back to
/// itself cannot resolve its base constructor (`getBaseConstructorTypeOfClass`,
/// TS2506), and an interface's base types cannot resolve at all
/// (`reportCircularBaseType`, TS2310, at each of its declarations). Each base
/// name is resolved where the declaration naming it is written.
pub(crate) fn report_circular_base(
    name: &str,
    name_span: Option<TextSpan>,
    type_parameters: &[surge_ts_syntax::ParsedTypeParameter],
    is_class: bool,
    ctx: &mut CheckerContext,
) {
    let Some(TypeDeclarationInfo::Interface(start)) = ctx.lookup_type_declaration(name) else {
        return;
    };
    let start = start.clone();
    if !base_chain_reaches(&start, ctx) {
        return;
    }
    let diagnostic = if is_class {
        Diagnostic::ts2506(name, ctx.file_name.clone())
    } else if type_parameters.is_empty() {
        Diagnostic::ts2310(name, ctx.file_name.clone())
    } else {
        let parameters: Vec<&str> = type_parameters.iter().map(|parameter| parameter.name.as_str()).collect();
        Diagnostic::ts2310(format!("{name}<{}>", parameters.join(", ")), ctx.file_name.clone())
    };
    ctx.push(match name_span {
        Some(span) => diagnostic.with_span(convert_span(span)),
        None => diagnostic,
    });
}

fn base_chain_reaches(start: &InterfaceInfo, ctx: &mut CheckerContext) -> bool {
    let key = |info: &InterfaceInfo| (info.file_name.clone(), info.name_span.map(|span| span.start));
    let target = key(start);
    // A class has one base, its first `extends` type (`getEffectiveBaseTypeNode`).
    let bases = |info: &InterfaceInfo| {
        let count = if info.is_class_instance { 1 } else { info.body.extends.len() };
        info.body
            .extends
            .iter()
            .take(count)
            .map(|base| (info.resolution_scope.clone(), base.name.clone()))
            .collect::<Vec<_>>()
    };
    let mut pending = bases(start);
    let mut visited = Vec::new();
    while let Some((scope, name)) = pending.pop() {
        let found = crate::infer::types::with_type_declaration_scope(&scope, ctx, |ctx| {
            ctx.lookup_type_declaration(&name).cloned()
        });
        let Some(TypeDeclarationInfo::Interface(info)) = found else { continue };
        let info_key = key(&info);
        if info_key == target {
            return true;
        }
        if visited.contains(&info_key) {
            continue;
        }
        visited.push(info_key);
        pending.extend(bases(&info));
    }
    false
}

/// tsc's `checkInterfaceDeclaration` heritage checks, on the interface as its
/// own type parameters instantiate it: bases that supply the same member must
/// supply identical ones (`checkInheritedPropertiesAreIdentical`, TS2320),
/// and only then may the interface override an inherited member with nothing
/// but a type assignable to it (TS2430). Both report on the interface's name.
pub(crate) fn check_interface_heritage(
    interface: &ParsedInterfaceDeclaration,
    ctx: &mut CheckerContext,
) {
    if interface.extends.is_empty() {
        return;
    }
    let _variables = crate::checks::function::enter_body_type_variables(&interface.type_parameters, ctx);
    crate::checks::function::with_type_parameter_scope(&interface.type_parameters, ctx, |ctx| {
        check_interface_heritage_in_scope(interface, ctx);
    });
}

fn check_interface_heritage_in_scope(interface: &ParsedInterfaceDeclaration, ctx: &mut CheckerContext) {
    let members: Vec<DeclaredMember> = interface
        .members
        .iter()
        .map(|member| DeclaredMember {
            name: member.name.clone(),
            name_span: member.name_span,
        })
        .collect();
    let own_arguments: Vec<ParsedType> = interface
        .type_parameters
        .iter()
        .map(|parameter| named_type(&parameter.name, None))
        .collect();
    let own_type = map_parsed_type(named_type_with(&interface.name, interface.name_span, own_arguments), ctx);
    let own_display = type_display(&interface.name, &interface.type_parameters.iter().map(|p| p.name.clone()).collect::<Vec<_>>());

    let mut bases = Vec::new();
    for base in &interface.extends {
        // A base that names no type is already reported, at its own span.
        if !base.name.contains('.') && ctx.lookup_type_declaration(&base.name).is_none() {
            continue;
        }
        let base_type = map_parsed_type(named_type_with(&base.name, None, base.type_arguments.clone()), ctx);
        let display = if base.type_arguments.is_empty() { base.name.clone() } else { base_type.name() };
        bases.push((display, base_type));
    }

    if bases.len() > 1 && report_non_identical_inherited_members(interface, &own_display, &members, &bases, ctx) {
        return;
    }
    for (display, base_type) in &bases {
        if incompatible_members(&members, &own_type, base_type).is_empty() {
            continue;
        }
        let diagnostic = Diagnostic::ts2430(&own_display, display, ctx.file_name.clone());
        ctx.push(match interface.name_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        });
    }
}

/// `checkInheritedPropertiesAreIdentical`: a member the interface does not
/// declare itself, supplied by two bases, must be the same member in both.
fn report_non_identical_inherited_members(
    interface: &ParsedInterfaceDeclaration,
    own_display: &str,
    members: &[DeclaredMember],
    bases: &[(String, Type)],
    ctx: &mut CheckerContext,
) -> bool {
    let mut seen: Vec<(Arc<str>, surge_ts_types::ObjectProperty, usize)> = Vec::new();
    let mut identical = true;
    for (index, (_, base_type)) in bases.iter().enumerate() {
        let Type::Object(object) = base_type.peeled() else {
            continue;
        };
        for (name, property) in object.properties.iter() {
            if members.iter().any(|member| member.name == name.as_ref()) {
                continue;
            }
            match seen.iter().find(|(seen_name, _, _)| seen_name == name) {
                None => seen.push((name.clone(), property.clone(), index)),
                Some((_, existing, existing_index)) => {
                    if properties_identical(existing, property) {
                        continue;
                    }
                    identical = false;
                    let diagnostic = Diagnostic::ts2320(
                        own_display,
                        &bases[*existing_index].0,
                        &bases[index].0,
                        ctx.file_name.clone(),
                    );
                    ctx.push(match interface.name_span {
                        Some(span) => diagnostic.with_span(convert_span(span)),
                        None => diagnostic,
                    });
                }
            }
        }
    }
    !identical
}

/// `compareProperties` under the identity relation. A member surge could not
/// model is not evidence of a conflict.
fn properties_identical(left: &surge_ts_types::ObjectProperty, right: &surge_ts_types::ObjectProperty) -> bool {
    if !comparable(&left.ty, &right.ty)
        || surge_ts_types::parameter_type_is_degraded(&left.ty)
        || surge_ts_types::parameter_type_is_degraded(&right.ty)
    {
        return true;
    }
    left.restriction == right.restriction
        && (left.restriction.is_some() || left.optional == right.optional)
        && left.readonly == right.readonly
        && surge_ts_types::is_type_identical_to(&left.ty, &right.ty)
}

pub(crate) fn named_type_with(name: &str, span: Option<TextSpan>, type_arguments: Vec<ParsedType>) -> ParsedType {
    ParsedType::Named(std::sync::Arc::new(ParsedNamedType {
        name: name.to_string(),
        span,
        type_arguments,
    }))
}

/// `typeToString` of a generic declaration's own type: `I<T, U>`.
pub(crate) fn type_display(name: &str, type_parameters: &[String]) -> String {
    if type_parameters.is_empty() {
        name.to_string()
    } else {
        format!("{name}<{}>", type_parameters.join(", "))
    }
}

/// The `extends` relation tsc checks once the member walk found nothing:
/// `typeWithThis` against `baseWithThis` (TS2415), then, only when that
/// holds, the static sides (TS2417). Two of `propertyRelatedTo`'s failures
/// are visible without types — a `private` or `protected` member on one side
/// that is not on the other — and the static side adds the type comparison
/// the instance walk already did. What is not modelled here: an inherited
/// index signature the class's own members violate, and `#private` names,
/// which the parser drops.
pub(crate) fn check_base_class_relation(
    class: &ParsedClassDeclaration,
    base_name: &str,
    ctx: &mut CheckerContext,
) {
    if class.is_declare || !class.type_parameters.is_empty() {
        return;
    }
    let Some(TypeDeclarationInfo::Interface(base)) = ctx.lookup_type_declaration(base_name) else {
        return;
    };
    if !base.is_class_instance {
        return;
    }
    let base = base.clone();
    let base_display = base
        .declared_name
        .as_deref()
        .unwrap_or(&base.name)
        .to_string();

    let own_accessibility = |name: &str, is_static: bool| {
        class
            .restricted_members
            .iter()
            .find(|member| member.name == name && member.is_static == is_static)
            .map(|member| member.accessibility)
    };
    let instance_members: Vec<String> = class_instance_members(class)
        .into_iter()
        .map(|member| member.name)
        .chain(
            super::classes::constructor_parameter_property_members(class)
                .into_iter()
                .map(|member| member.name),
        )
        .collect();
    let instance_incompatible = instance_members.iter().any(|name| {
        let Some(base_accessibility) = base_member_accessibility(&base, name, false, ctx) else {
            return false;
        };
        !accessibility_related(own_accessibility(name, false), base_accessibility)
    });
    if instance_incompatible {
        let diagnostic = Diagnostic::ts2415(&class.name, &base_display, ctx.file_name.clone());
        ctx.push(match class.name_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        });
        return;
    }

    let static_members: Vec<String> = class
        .members
        .iter()
        .filter_map(|member| match member {
            ParsedClassMember::Property(property) if property.is_static => Some(property.name.clone()),
            ParsedClassMember::Method(method) if method.is_static => Some(method.name.clone()),
            ParsedClassMember::Accessor(accessor) if accessor.is_static => Some(accessor.name.clone()),
            _ => None,
        })
        .collect();
    if static_members.is_empty() {
        return;
    }
    let static_type = |name: &str| ctx.symbols.get(name).map(|symbol| symbol.ty.peeled());
    let (own_static, base_static) = (static_type(&class.name), static_type(base_name));
    let static_incompatible = static_members.iter().any(|name| {
        let Some(base_accessibility) = base_member_accessibility(&base, name, true, ctx) else {
            return false;
        };
        if !accessibility_related(own_accessibility(name, true), base_accessibility) {
            return true;
        }
        let (Some(own), Some(base)) = (
            own_static.as_ref().and_then(|ty| property_type(ty, name)),
            base_static.as_ref().and_then(|ty| property_type(ty, name)),
        ) else {
            return false;
        };
        comparable(&own, &base) && !is_assignable_to(&own, &base)
    });
    if static_incompatible {
        let diagnostic = Diagnostic::ts2417(
            format!("typeof {}", class.name),
            format!("typeof {base_display}"),
            ctx.file_name.clone(),
        );
        ctx.push(match class.name_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        });
    }
}

/// The modifier the base side of the relation sees for `name`: `Some(None)`
/// for a public member the base chain declares, `None` when no class in the
/// chain declares it at all.
fn base_member_accessibility(
    base: &InterfaceInfo,
    name: &str,
    is_static: bool,
    ctx: &CheckerContext,
) -> Option<Option<ParsedMemberAccessibility>> {
    let mut current = base.clone();
    for _ in 0..32 {
        if let Some(restricted) = current
            .body
            .restricted_members
            .iter()
            .find(|member| member.name == name && member.is_static == is_static)
        {
            return Some(Some(restricted.accessibility));
        }
        if !is_static && current.body.members.iter().any(|member| member.name == name) {
            return Some(None);
        }
        if is_static && ctx.symbols.get(&current.name).is_some_and(|symbol| {
            matches!(symbol.ty.peeled(), Type::Object(object) if object.properties.get(name).is_some())
        }) {
            return Some(None);
        }
        current = crate::checks::expr::base_interface(&current, ctx)?;
    }
    None
}

/// tsc's `propertyRelatedTo` on modifiers, for a derived class against its
/// base: a private member must be the very same declaration on both sides,
/// which a redeclaration never is; a protected base member may be widened by
/// the subclass; a protected derived member may not hide a public one.
fn accessibility_related(
    own: Option<ParsedMemberAccessibility>,
    base: Option<ParsedMemberAccessibility>,
) -> bool {
    match (own, base) {
        (Some(ParsedMemberAccessibility::Private), _) | (_, Some(ParsedMemberAccessibility::Private)) => false,
        (_, Some(ParsedMemberAccessibility::Protected)) => true,
        (Some(ParsedMemberAccessibility::Protected), None) => false,
        (None, None) => true,
    }
}
