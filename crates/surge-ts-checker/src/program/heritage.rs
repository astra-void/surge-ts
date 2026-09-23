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
    ParsedClassDeclaration, ParsedClassMember, ParsedInterfaceDeclaration, ParsedNamedType,
    ParsedType, TextSpan,
};
use surge_ts_types::{Type, is_assignable_to};

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
/// everything, so both are left alone rather than reported.
fn comparable(source: &Type, target: &Type) -> bool {
    !source.is_unknown()
        && !target.is_unknown()
        && !matches!(source, Type::Any)
        && !matches!(target, Type::Any)
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
/// `base_name` incompatibly. Returns whether anything was reported, which is
/// what gates the broad diagnostic.
pub(crate) fn report_incompatible_heritage_members(
    class: &ParsedClassDeclaration,
    base_name: &str,
    ctx: &mut CheckerContext,
) -> bool {
    if class.is_declare || !class.type_parameters.is_empty() {
        return false;
    }

    let own_type = map_parsed_type(named_type(&class.name, class.name_span), ctx);
    // Resolved as the heritage position resolves it, where a base that names a
    // value (`extends Mixed`) takes its instance type from the constructor.
    let base_type = crate::program::with_dts_expansion_reason(
        crate::program::DtsExpansionReason::InterfaceHeritageResolution,
        || map_parsed_type(named_type(base_name, None), ctx),
    );
    let members = class_instance_members(class);
    let incompatible = incompatible_members(&members, &own_type, &base_type);
    if incompatible.is_empty() {
        return false;
    }

    // tsc names the base *type*; for a value base that is the constructed
    // instance, not the identifier in the `extends` clause.
    let base_display = if ctx.lookup_type_declaration(base_name).is_some() {
        base_name.to_string()
    } else {
        base_type.name()
    };
    for (name, name_span) in incompatible {
        let diagnostic =
            Diagnostic::ts2416(&name, &class.name, &base_display, ctx.file_name.clone());
        ctx.push(match name_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        });
    }
    true
}

/// TS2430: an interface may only override an inherited member with a type
/// assignable to it. Unlike the class checks this reports once, on the
/// interface name, however many members conflict.
pub(crate) fn check_interface_heritage(
    interface: &ParsedInterfaceDeclaration,
    ctx: &mut CheckerContext,
) {
    if interface.extends.is_empty() || !interface.type_parameters.is_empty() {
        return;
    }

    let members: Vec<DeclaredMember> = interface
        .members
        .iter()
        .map(|member| DeclaredMember {
            name: member.name.clone(),
            name_span: member.name_span,
        })
        .collect();
    let own_type = map_parsed_type(named_type(&interface.name, interface.name_span), ctx);

    for base in &interface.extends {
        if !base.type_arguments.is_empty() {
            continue;
        }
        // A base that names no type is already reported, at its own span.
        if !base.name.contains('.') && ctx.lookup_type_declaration(&base.name).is_none() {
            continue;
        }
        let base_type = map_parsed_type(named_type(&base.name, None), ctx);
        if incompatible_members(&members, &own_type, &base_type).is_empty() {
            continue;
        }
        let diagnostic =
            Diagnostic::ts2430(&interface.name, &base.name, ctx.file_name.clone());
        ctx.push(match interface.name_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        });
    }
}
