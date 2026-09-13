//! Class declaration binding and member-body checking.
//!
//! A class binds two things: an instance *type* (fields + methods, registered as
//! an interface so it resolves in type position and on `new` results) and a
//! constructor/static *value* (a `Type::Object` carrying static members plus a
//! construct signature, registered as a value symbol so `new Class(...)`,
//! `Class.staticMember`, and `typeof Class` all work).

use surge_ts_syntax::{
    ParsedBindingName, ParsedClassAccessor, ParsedClassDeclaration, ParsedClassMember,
    ParsedClassMethod, ParsedClassProperty, ParsedFunctionParameter, ParsedFunctionType,
    ParsedFunctionTypeParameter, ParsedInterfaceMember, ParsedNamedType, ParsedType,
};
use surge_ts_types::{FunctionType, ObjectProperty, ObjectType, PropertyMap, Type};

use std::sync::Arc;

use surge_ts_diagnostics::Diagnostic;

use crate::checks::function::{
    check_function_body_with_signature_and_this, map_function_signature,
};
use crate::context::{CheckerContext, convert_span};
use crate::infer::map_parsed_type;
use crate::symbols::{InterfaceInfo, SymbolInfo, SymbolKind, SymbolTable, TypeDeclarationInfo};

/// Builds the instance-side interface (fields + instance methods) for a class.
/// Static members and the constructor are excluded; they live on the value side.
pub(crate) fn class_instance_interface_info(
    class: &ParsedClassDeclaration,
    file_name: Arc<str>,
) -> InterfaceInfo {
    let mut members: Vec<_> = class
        .members
        .iter()
        .filter_map(class_member_to_interface_member)
        .collect();
    members.extend(constructor_parameter_property_members(class));

    InterfaceInfo::new(
        class.name.clone(),
        file_name,
        class.name_span,
        class.type_parameters.clone(),
        class.extends.clone(),
        members,
        None,
        None,
        Vec::new(),
        None,
    )
}

fn class_member_to_interface_member(member: &ParsedClassMember) -> Option<ParsedInterfaceMember> {
    match member {
        ParsedClassMember::Property(property) if !property.is_static => {
            Some(ParsedInterfaceMember {
                name: property.name.clone(),
                name_span: property.name_span,
                optional: property.optional,
                is_abstract: property.is_abstract,
                is_method: false,
                ty: property.declared_type.clone().unwrap_or(ParsedType::Any),
            })
        }
        ParsedClassMember::Method(method) if !method.is_static => Some(ParsedInterfaceMember {
            name: method.name.clone(),
            name_span: method.name_span,
            optional: false,
            is_abstract: method.is_abstract,
            is_method: true,
            ty: method_function_type(method),
        }),
        ParsedClassMember::Accessor(accessor) if !accessor.is_static => {
            Some(ParsedInterfaceMember {
                name: accessor.name.clone(),
                name_span: accessor.name_span,
                optional: false,
                is_abstract: accessor.is_abstract,
                is_method: false,
                ty: accessor_property_type(accessor),
            })
        }
        _ => None,
    }
}

/// Synthesizes instance members for constructor parameter properties — a
/// parameter carrying a `public`/`private`/`protected`/`readonly` modifier
/// declares a field of the same name and type. Only identifier-named parameters
/// can be parameter properties (TS rejects destructuring patterns here).
fn constructor_parameter_property_members(
    class: &ParsedClassDeclaration,
) -> Vec<ParsedInterfaceMember> {
    class
        .members
        .iter()
        .find_map(|member| match member {
            ParsedClassMember::Constructor(constructor) => Some(&constructor.parameters),
            _ => None,
        })
        .map(|parameters| {
            parameters
                .iter()
                .filter(|parameter| parameter.is_parameter_property)
                .filter_map(|parameter| {
                    let ParsedBindingName::Identifier { name, span } = &parameter.binding_name
                    else {
                        return None;
                    };
                    Some(ParsedInterfaceMember {
                        name: name.clone(),
                        name_span: *span,
                        optional: parameter.optional,
                        is_abstract: false,
                        is_method: false,
                        ty: parameter.declared_type.clone().unwrap_or(ParsedType::Any),
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Lowers an accessor to the type of the property it presents. A getter's
/// return type wins (the read type); a setter-only accessor falls back to its
/// parameter type. Missing annotations degrade to `any`, matching the implicit
/// type tsc infers for an un-annotated accessor.
fn accessor_property_type(accessor: &ParsedClassAccessor) -> ParsedType {
    accessor
        .getter_return_type
        .clone()
        .or_else(|| accessor.setter_param_type.clone())
        .unwrap_or(ParsedType::Any)
}

fn method_function_type(method: &ParsedClassMethod) -> ParsedType {
    ParsedType::Function(std::sync::Arc::new(ParsedFunctionType {
        parameters: method
            .parameters
            .iter()
            .map(parameter_to_type_parameter)
            .collect(),
        return_type: Box::new(method.return_type.clone().unwrap_or(ParsedType::Any)),
        type_parameters: method.type_parameters.clone(),
    }))
}

fn parameter_to_type_parameter(parameter: &ParsedFunctionParameter) -> ParsedFunctionTypeParameter {
    let (name, name_span) = match &parameter.binding_name {
        ParsedBindingName::Identifier { name, span } => (Some(name.clone()), *span),
        _ => (None, None),
    };

    ParsedFunctionTypeParameter {
        name,
        name_span,
        ty: parameter.declared_type.clone().unwrap_or(ParsedType::Any),
        optional: parameter.optional,
        is_this: false,
        rest: parameter.rest,
    }
}

/// The written constructor signature of a generic class, carried on its `any`
/// value side so `new C(arg)` can infer the class's type arguments from the
/// arguments instead of falling back to their declared defaults
/// (`MutationObserver<…, TVariables = void>` stayed at `void`). Only an explicit
/// constructor contributes: with none there is nothing to infer from.
fn generic_class_constructor_signature(
    class: &ParsedClassDeclaration,
    ctx: &CheckerContext,
) -> Option<Arc<crate::symbols::FunctionSignatureInfo>> {
    let constructor = class.members.iter().find_map(|member| match member {
        ParsedClassMember::Constructor(constructor) => Some(constructor),
        _ => None,
    })?;
    Some(crate::checks::function::function_signature_info(
        &class.type_parameters,
        &constructor.parameters,
        None,
        &ctx.file_name,
    ))
}

/// Builds the constructor/static-side value symbol: a `Type::Object` whose
/// properties are the static members and whose construct signature yields the
/// instance type.
pub(crate) fn build_class_value_symbol(
    class: &ParsedClassDeclaration,
    ctx: &mut CheckerContext,
) -> SymbolInfo {
    build_class_value_symbol_with_scope(class, None, ctx)
}

/// [`build_class_value_symbol`] with the in-progress value table the class is
/// being bound into, which is where a base class declared earlier in the same
/// scope lives — `ctx.symbols` does not have it yet at binding time.
pub(crate) fn build_class_value_symbol_with_scope(
    class: &ParsedClassDeclaration,
    scope: Option<&SymbolTable>,
    ctx: &mut CheckerContext,
) -> SymbolInfo {
    // Generic classes are out of scope for this slice. Model their value side as
    // `any` so `new C<T>(...)` and `C.member` stay non-cascading rather than
    // resolving the self type without type arguments (which would mis-report).
    // A static object over them was measured to open TS2351/TS2554 across zod,
    // trpc and ofetch (2026-09-12), so the `any` stays.
    if !class.type_parameters.is_empty() {
        return SymbolInfo {
            ty: Type::Any,
            kind: SymbolKind::Const,
            function_signature: generic_class_constructor_signature(class, ctx),
        };
    }

    let instance_type = class_instance_type(class, ctx);
    let construct_signature = class_construct_signature(class, instance_type.clone(), ctx);

    // Statics are inherited: `class D extends B {}` makes every static of `B`
    // reachable as `D.x`. The base's static side is a value, not part of the
    // instance-side interface classes are bound as, so it is read from the
    // value environment — which means an unbound base simply contributes
    // nothing rather than being wrong.
    let mut properties = inherited_static_properties(class, scope, ctx);
    // Every class value carries `prototype`, typed as the instance — the shape
    // `Object.setPrototypeOf(this, C.prototype)` reads. A static member named
    // `prototype` is illegal in TypeScript, so nothing below can overwrite it.
    // It is also the one inherited entry that must not survive: `D.prototype`
    // is a `D`, not a `B`.
    properties.insert(
        "prototype".into(),
        ObjectProperty::required(instance_type),
    );
    for member in &class.members {
        match member {
            ParsedClassMember::Property(property) if property.is_static => {
                let property_type = static_property_type(property, ctx);
                let object_property = if property.optional {
                    ObjectProperty::optional(property_type)
                } else {
                    ObjectProperty::required(property_type)
                };
                properties.insert(property.name.as_str().into(), object_property);
            }
            ParsedClassMember::Method(method) if method.is_static => {
                let mut function_type = map_function_signature(
                    &method.parameters,
                    method.return_type.as_ref(),
                    &method.type_parameters,
                    None,
                    ctx,
                );
                // A generic or predicate static (`static assert(v): asserts v is
                // E`) keeps its written signature on the handle, so a call can
                // instantiate it and a guard can read the predicate.
                if !method.type_parameters.is_empty()
                    || matches!(method.return_type, Some(ParsedType::Predicate(_)))
                {
                    function_type = function_type.with_declaration(Arc::new(
                        crate::checks::call::DeclaredMemberSignature {
                            signature: crate::checks::function::function_signature_info(
                                &method.type_parameters,
                                &method.parameters,
                                method.return_type.as_ref(),
                                &ctx.file_name,
                            ),
                            outer_type_arguments: Vec::new(),
                        },
                    ));
                }
                properties.insert(
                    method.name.as_str().into(),
                    ObjectProperty::required(Type::Function(function_type)),
                );
            }
            ParsedClassMember::Accessor(accessor) if accessor.is_static => {
                let property_type = map_parsed_type(accessor_property_type(accessor), ctx);
                properties.insert(
                    accessor.name.as_str().into(),
                    ObjectProperty::required(property_type),
                );
            }
            _ => {}
        }
    }

    let static_type = ObjectType::new(properties, None)
        .with_construct_signature(construct_signature)
        .with_alias_name(format!("typeof {}", class.name));

    SymbolInfo {
        ty: Type::Object(static_type),
        kind: SymbolKind::Const,
        function_signature: None,
    }
}

/// The base class's static members, as the starting point for a derived class's
/// static side. Empty when the class has no base, when the base's value is not
/// in scope yet, or when the base's static side is not an object (a generic
/// class models its value as `any`).
fn inherited_static_properties(
    class: &ParsedClassDeclaration,
    scope: Option<&SymbolTable>,
    ctx: &CheckerContext,
) -> PropertyMap {
    let Some(base) = class.extends.first() else {
        return PropertyMap::default();
    };
    // A base bound by an import is not in the scope being built: inside
    // `declare module "stream"`, `class Stream extends EventEmitter` names the
    // block's `import { EventEmitter } from "node:events"`, which the export
    // table build seeds as the value fallback.
    let Some(base_symbol) = scope
        .and_then(|scope| scope.get(&base.name))
        .or_else(|| ctx.symbols.get(&base.name))
        .or_else(|| {
            ctx.module_value_fallback
                .as_ref()
                .and_then(|fallback| fallback.get(&base.name))
        })
    else {
        return PropertyMap::default();
    };
    let Type::Object(base_static) = base_symbol.ty.peeled() else {
        return PropertyMap::default();
    };
    base_static.properties.as_ref().clone()
}

fn static_property_type(property: &ParsedClassProperty, ctx: &mut CheckerContext) -> Type {
    match property.declared_type.clone() {
        Some(declared_type) => map_parsed_type(declared_type, ctx),
        None => Type::Any,
    }
}

fn class_instance_type(class: &ParsedClassDeclaration, ctx: &mut CheckerContext) -> Type {
    map_parsed_type(
        ParsedType::Named(std::sync::Arc::new(ParsedNamedType {
            name: class.name.clone(),
            span: class.name_span,
            type_arguments: Vec::new(),
        })),
        ctx,
    )
}

fn class_construct_signature(
    class: &ParsedClassDeclaration,
    instance_type: Type,
    ctx: &mut CheckerContext,
) -> FunctionType {
    let named_instance = ParsedType::Named(std::sync::Arc::new(ParsedNamedType {
        name: class.name.clone(),
        span: class.name_span,
        type_arguments: Vec::new(),
    }));

    for member in &class.members {
        if let ParsedClassMember::Constructor(constructor) = member {
            return map_function_signature(
                &constructor.parameters,
                Some(&named_instance),
                &[],
                None,
                ctx,
            );
        }
    }

    if !class.extends.is_empty() {
        // The constructor is inherited from the base class, whose declaration is
        // not reachable from here (classes are bound as instance-side interfaces,
        // which exclude the constructor). Accept any argument list rather than
        // report the base's arity as zero: `new ZodString({ … })` on a derived
        // class was TS2554 "Expected 0 arguments".
        return FunctionType::new(vec![Type::Any], instance_type, true, 0);
    }

    // A class with no explicit constructor is constructible with zero arguments.
    FunctionType::new(vec![], instance_type, false, 0)
}

/// Type-checks a class's constructor and method bodies, binding `this` to the
/// instance type (instance members/constructor) or static side (static methods).
pub(crate) fn check_class_declaration(class: &ParsedClassDeclaration, ctx: &mut CheckerContext) {
    // Ambient classes have no bodies; generic classes are out of scope and would
    // resolve member/`this` types against unbound type parameters.
    if class.is_declare || !class.type_parameters.is_empty() {
        return;
    }

    let instance_type = class_instance_type(class, ctx);
    let static_value = build_class_value_symbol(class, ctx);
    let static_type = static_value.ty;

    if ctx.options.no_implicit_override && !class.extends.is_empty() {
        check_implicit_override(class, ctx);
    }

    for member in &class.members {
        match member {
            ParsedClassMember::Constructor(constructor) => {
                let function_type =
                    map_function_signature(&constructor.parameters, None, &[], None, ctx);
                check_function_body_with_signature_and_this(
                    None,
                    constructor.parameters.clone(),
                    constructor.body.clone(),
                    &function_type,
                    &[],
                    None,
                    false,
                    None,
                    Some(instance_type.clone()),
                    true,
                    None,
                    false,
                    ctx,
                );
            }
            ParsedClassMember::Method(method) => {
                let function_type = map_function_signature(
                    &method.parameters,
                    method.return_type.as_ref(),
                    &method.type_parameters,
                    None,
                    ctx,
                );
                let this_type = if method.is_static {
                    static_type.clone()
                } else {
                    instance_type.clone()
                };
                check_function_body_with_signature_and_this(
                    None,
                    method.parameters.clone(),
                    method.body.clone(),
                    &function_type,
                    &method.type_parameters,
                    None,
                    method.return_type.is_some(),
                    method.name_span,
                    Some(this_type),
                    false,
                    method.has_body.then(|| method.body_reads.as_slice()),
                    method.is_generator,
                    ctx,
                );
            }
            ParsedClassMember::Property(_) | ParsedClassMember::Accessor(_) => {}
        }
    }
}

/// TS4114 under `noImplicitOverride`: an instance member that overrides a
/// resolvable base-class member must carry the `override` modifier. Base-member
/// resolution is conservative — only locally-declared base classes are walked
/// (a builtin/imported base leaves its members out of the set), so an
/// unresolvable base yields no diagnostic rather than a false positive. Only
/// TS4114 (missing `override`) is reported, never TS4113 (spurious `override`),
/// since the latter needs the full base type to prove a member is *not* inherited.
fn check_implicit_override(class: &ParsedClassDeclaration, ctx: &mut CheckerContext) {
    let inherited = collect_inherited_instance_member_names(&class.extends, ctx);
    if inherited.is_empty() {
        return;
    }
    let base_name = class.extends.first().map(|base| base.name.clone());
    let Some(base_name) = base_name else {
        return;
    };

    for member in &class.members {
        let (name, name_span, is_static, is_override) = match member {
            ParsedClassMember::Method(method) => (
                &method.name,
                method.name_span,
                method.is_static,
                method.is_override,
            ),
            ParsedClassMember::Property(property) => (
                &property.name,
                property.name_span,
                property.is_static,
                property.is_override,
            ),
            ParsedClassMember::Accessor(accessor) => (
                &accessor.name,
                accessor.name_span,
                accessor.is_static,
                accessor.is_override,
            ),
            ParsedClassMember::Constructor(_) => continue,
        };
        if is_static || is_override || !inherited.contains(name) {
            continue;
        }
        let diagnostic = Diagnostic::ts4114(&base_name, ctx.file_name.clone());
        let diagnostic = match name_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };
        ctx.push(diagnostic);
    }
}

/// Instance member names reachable through a chain of locally-declared base
/// classes (registered as interfaces). Non-local bases (builtins, imports) are
/// simply absent, keeping the override check conservative.
fn collect_inherited_instance_member_names(
    extends: &[ParsedNamedType],
    ctx: &CheckerContext,
) -> std::collections::HashSet<String> {
    let mut names = std::collections::HashSet::new();
    let mut visited = std::collections::HashSet::new();
    let mut stack: Vec<String> = extends.iter().map(|base| base.name.clone()).collect();

    while let Some(base_name) = stack.pop() {
        if !visited.insert(base_name.clone()) {
            continue;
        }
        if let Some(TypeDeclarationInfo::Interface(info)) = ctx.lookup_type_declaration(&base_name)
        {
            // Only source-declared base classes participate. A base resolved from a
            // declaration file (a dependency, an ambient module like
            // `cloudflare:workers`, or a generated `.d.ts`) may not resolve the same
            // way under the oracle's `tsc`, so treating it as a real base risks a
            // false positive; skip it.
            if info.file_name.ends_with(".d.ts") {
                continue;
            }
            for member in &info.body.members {
                // Implementing an abstract member does not require `override`.
                if member.is_abstract {
                    continue;
                }
                names.insert(member.name.clone());
            }
            for parent in &info.body.extends {
                stack.push(parent.name.clone());
            }
        }
    }

    names
}

/// Inserts a class's instance-side interface into the current type-declaration
/// table. Mirrors `collect_interface` for first-wins / duplicate behaviour.
pub(crate) fn collect_class(class: &ParsedClassDeclaration, ctx: &mut CheckerContext) {
    let info = class_instance_interface_info(class, ctx.file_name_arc());
    // A class declaration-merges with a same-named interface of its file
    // (`declare interface Emitter<T> { on(…): this }` + `class Emitter<T>
    // extends EventEmitter {}`): the interface's members and the class's
    // heritage both belong to the instance. The class-first order already
    // merges through `collect_interface`; this is the interface-first order.
    let merged = match ctx.type_declarations.get(&class.name) {
        Some(TypeDeclarationInfo::Interface(existing)) if existing.file_name == info.file_name => {
            Some(crate::symbols::merge_interface_infos(existing, &info))
        }
        _ => None,
    };
    match merged {
        Some(merged) => ctx
            .type_declarations
            .upsert(&class.name, TypeDeclarationInfo::Interface(merged)),
        None => {
            let _ = ctx
                .type_declarations
                .insert(class.name.clone(), TypeDeclarationInfo::Interface(info));
        }
    }
}
