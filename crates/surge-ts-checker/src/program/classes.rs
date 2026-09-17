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

    let mut info = InterfaceInfo::new(
        class.name.clone(),
        file_name,
        class.name_span,
        class.type_parameters.clone(),
        class.extends.clone(),
        members,
        class.string_index_type.clone(),
        class.number_index_type.clone(),
        None,
        Vec::new(),
        None,
    );
    info.is_abstract_class = class.is_abstract;
    info
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
                ty: property
                    .declared_type
                    .clone()
                    .unwrap_or_else(|| initializer_property_type(property)),
                readonly: property.readonly,
                write_ty: None,
            })
        }
        ParsedClassMember::Method(method) if !method.is_static => Some(ParsedInterfaceMember {
            name: method.name.clone(),
            name_span: method.name_span,
            optional: false,
            is_abstract: method.is_abstract,
            is_method: true,
            ty: method_function_type(method),
            readonly: false,
            write_ty: None,
        }),
        ParsedClassMember::Accessor(accessor) if !accessor.is_static => {
            Some(ParsedInterfaceMember {
                name: accessor.name.clone(),
                name_span: accessor.name_span,
                optional: false,
                is_abstract: accessor.is_abstract,
                is_method: false,
                ty: accessor_property_type(accessor),
                // A getter with no setter is read-only, exactly as
                // `isReadonlySymbol` has it.
                readonly: accessor.has_getter && !accessor.has_setter,
                write_ty: accessor_write_type(accessor),
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
                        // A parameter property written with a default
                        // (`readonly encoder: E = noopEncoder`) declares a
                        // *required* member: the parameter is optional at the
                        // call, the property never is. The parser folds a
                        // default into `optional` so arity accepts the omitted
                        // argument, so the initializer is what tells the two
                        // apart here.
                        optional: parameter.optional && parameter.initializer.is_none(),
                        is_abstract: false,
                        is_method: false,
                        ty: parameter.declared_type.clone().unwrap_or(ParsedType::Any),
                        readonly: parameter.is_readonly_parameter_property,
                        write_ty: None,
                    })
                })
                .collect()
        })
        .unwrap_or_default()
}

/// What a write to an accessor is checked against, when that differs from what
/// a read of it produces: the setter's parameter type of a pair whose getter
/// declares something else. tsc keeps the two apart as `getTypeOfSymbol` and
/// `getWriteTypeOfSymbol`, and writes against the latter, so
/// `set value(next: number | string)` beside `get value(): number` accepts a
/// string. `None` when reading and writing share a type, which is every member
/// that is not such a pair.
fn accessor_write_type(accessor: &ParsedClassAccessor) -> Option<ParsedType> {
    let setter_param_type = accessor.setter_param_type.clone()?;
    if !accessor.has_getter || accessor.getter_return_type.is_none() {
        return None;
    }
    (accessor.getter_return_type.as_ref() != Some(&setter_param_type)).then_some(setter_param_type)
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
        return_type: Box::new(
            method
                .return_type
                .clone()
                .or_else(|| syntactic_method_return_type(method))
                .unwrap_or(ParsedType::Any),
        ),
        type_parameters: method.type_parameters.clone(),
    }))
}

/// The return type an unannotated method takes from its body when every
/// `return` in it hands back a literal: the widened union of those literals'
/// primitives, as `getReturnTypeFromBody` gives it, `void` when no `return`
/// carries a value, and either wrapped in `Promise` for an `async` method.
/// Anything else — another kind of returned value, a bare `return` beside a
/// valued one, a generator — is left for inference surge does not do here.
fn syntactic_method_return_type(method: &ParsedClassMethod) -> Option<ParsedType> {
    use surge_ts_syntax::{ParsedExpression, ParsedFunctionBodyStatement as Statement};

    fn collect(body: &[Statement], kinds: &mut Vec<ParsedType>, bare_return: &mut bool) -> Option<()> {
        for statement in body {
            match statement {
                Statement::Return(return_statement) => {
                    let Some(expression) = return_statement.expression.as_ref() else {
                        *bare_return = true;
                        continue;
                    };
                    let kind = match expression {
                        ParsedExpression::StringLiteral(_) => ParsedType::String,
                        ParsedExpression::TemplateLiteral { expressions, .. }
                            if expressions.is_empty() =>
                        {
                            ParsedType::String
                        }
                        ParsedExpression::NumberLiteral(_) => ParsedType::Number,
                        ParsedExpression::BooleanLiteral(_) => ParsedType::Boolean,
                        _ => return None,
                    };
                    if !kinds.contains(&kind) {
                        kinds.push(kind);
                    }
                }
                Statement::Block(block) => collect(block, kinds, bare_return)?,
                Statement::If(if_statement) => {
                    collect(&if_statement.then_body, kinds, bare_return)?;
                    collect(&if_statement.else_body, kinds, bare_return)?;
                }
                Statement::While(while_statement) => collect(&while_statement.body, kinds, bare_return)?,
                Statement::ForOf(for_of) => collect(&for_of.body, kinds, bare_return)?,
                Statement::Switch(switch_statement) => {
                    for case in &switch_statement.cases {
                        collect(&case.consequent, kinds, bare_return)?;
                    }
                }
                Statement::Try(try_statement) => {
                    collect(&try_statement.block, kinds, bare_return)?;
                    if let Some(handler) = &try_statement.handler {
                        collect(&handler.body, kinds, bare_return)?;
                    }
                    collect(&try_statement.finalizer, kinds, bare_return)?;
                }
                _ => {}
            }
        }
        Some(())
    }

    if method.is_generator || !method.has_body {
        return None;
    }
    let mut kinds = Vec::new();
    let mut bare_return = false;
    collect(&method.body, &mut kinds, &mut bare_return)?;
    let returned = match kinds.len() {
        // No value is returned anywhere: the method is `void`.
        0 => ParsedType::Void,
        _ if bare_return => return None,
        1 => kinds.pop()?,
        _ => ParsedType::Union(Arc::new(kinds)),
    };
    Some(if method.is_async {
        ParsedType::Named(Arc::new(ParsedNamedType {
            name: "Promise".to_string(),
            span: None,
            type_arguments: vec![returned],
        }))
    } else {
        returned
    })
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

/// Writes `class`'s own static properties, methods and accessors into
/// `properties`.
fn collect_static_members(
    class: &ParsedClassDeclaration,
    properties: &mut PropertyMap,
    ctx: &mut CheckerContext,
) {
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
}

/// Whether `class` declares a static whose return type narrows its argument
/// (`static assert(v): asserts v is E`, `static is(v): v is E`). Every other
/// static survives the `any` value side intact — a call site reads `any` and
/// carries on — but a predicate one does not: the narrowing is the whole point
/// of the call, and `any` silently drops it.
fn declares_narrowing_static(class: &ParsedClassDeclaration) -> bool {
    class.members.iter().any(|member| match member {
        ParsedClassMember::Method(method) => {
            method.is_static && matches!(method.return_type, Some(ParsedType::Predicate(_)))
        }
        _ => false,
    })
}

/// A generic class's static side. The instance type is unavailable without type
/// arguments, so the surface stays deliberately permissive: an injected open
/// index answers anything not written as a static with `any`, and the construct
/// signature accepts any arguments and yields `any`. That leaves `new C(...)`
/// and unknown-member reads exactly where the earlier plain `any` value left
/// them — a static object *without* the openness was measured to open
/// TS2351/TS2554 across zod, trpc and ofetch (2026-09-12) — while letting a
/// written static resolve, which a predicate or assertion one
/// (`static assert(v): asserts v is E`) needs to narrow at all.
fn generic_class_value_symbol(
    class: &ParsedClassDeclaration,
    ctx: &mut CheckerContext,
) -> SymbolInfo {
    if !declares_narrowing_static(class) {
        return SymbolInfo {
            ty: Type::Any,
            kind: SymbolKind::Const,
            function_signature: generic_class_constructor_signature(class, ctx),
        };
    }

    let mut properties = PropertyMap::default();
    collect_static_members(class, &mut properties, ctx);

    let static_type = ObjectType::new(properties, Some(Type::Any))
        .with_construct_signature(FunctionType::new(vec![Type::Any], Type::Any, true, 0))
        .with_alias_name(format!("typeof {}", class.name))
        .with_open_index_marker();

    SymbolInfo {
        ty: Type::Object(static_type),
        kind: SymbolKind::Const,
        function_signature: generic_class_constructor_signature(class, ctx),
    }
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
    if !class.type_parameters.is_empty() {
        return generic_class_value_symbol(class, ctx);
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
    collect_static_members(class, &mut properties, ctx);

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
        None => map_parsed_type(initializer_property_type(property), ctx),
    }
}

/// The type an unannotated property takes from its initializer, as tsc's
/// `getWidenedTypeForVariableLikeDeclaration` gives it: widened, except that a
/// `readonly` property keeps its literal. The instance side is built from
/// written types before any expression is checked, so only initializers whose
/// type is evident from their syntax are lowered; the rest stay `any`.
fn initializer_property_type(property: &ParsedClassProperty) -> ParsedType {
    property
        .initializer
        .as_ref()
        .and_then(|initializer| syntactic_initializer_type(initializer, property.readonly))
        .unwrap_or(ParsedType::Any)
}

fn syntactic_initializer_type(initializer: &surge_ts_syntax::ParsedExpression, keep_literal: bool) -> Option<ParsedType> {
    use surge_ts_syntax::ParsedExpression;
    Some(match initializer {
        ParsedExpression::StringLiteral(value) if keep_literal => ParsedType::StringLiteral(value.clone()),
        ParsedExpression::NumberLiteral(value) if keep_literal => ParsedType::NumberLiteral(value.clone()),
        ParsedExpression::BooleanLiteral(value) if keep_literal => ParsedType::BooleanLiteral(*value),
        ParsedExpression::StringLiteral(_) => ParsedType::String,
        ParsedExpression::NumberLiteral(_) => ParsedType::Number,
        ParsedExpression::BooleanLiteral(_) => ParsedType::Boolean,
        ParsedExpression::TemplateLiteral { expressions, .. } if expressions.is_empty() => {
            ParsedType::String
        }
        // An empty array literal is `never[]` under strictNullChecks: nothing
        // widens it to an evolving array the way a `let` binding would be.
        ParsedExpression::ArrayLiteral { elements, .. } if elements.is_empty() => {
            ParsedType::Array(Arc::new(ParsedType::Never))
        }
        ParsedExpression::ArrayLiteral { elements, .. } => {
            let mut element_types = elements.iter().map(|element| match &element.expression {
                _ if element.spread => None,
                ParsedExpression::StringLiteral(_) => Some(ParsedType::String),
                ParsedExpression::NumberLiteral(_) => Some(ParsedType::Number),
                ParsedExpression::BooleanLiteral(_) => Some(ParsedType::Boolean),
                _ => None,
            });
            let first = element_types.next()??;
            if !element_types.all(|element| element.as_ref() == Some(&first)) {
                return None;
            }
            ParsedType::Array(Arc::new(first))
        }
        ParsedExpression::ObjectLiteral { properties, .. } => {
            let mut members = Vec::with_capacity(properties.len());
            for property in properties {
                if property.is_spread || property.is_accessor || property.computed_key.is_some() {
                    return None;
                }
                // A member is a mutable location, so its literal widens even
                // when the property holding the object is `readonly`.
                let ty = if property.is_method {
                    ParsedType::Any
                } else {
                    syntactic_initializer_type(&property.value, false).unwrap_or(ParsedType::Any)
                };
                members.push(surge_ts_syntax::ParsedObjectTypeProperty {
                    name: property.name.clone(),
                    name_span: property.name_span,
                    ty,
                    optional: false,
                    is_method: property.is_method,
                    readonly: false,
                    write_ty: None,
                });
            }
            ParsedType::Object(Arc::new(surge_ts_syntax::ParsedObjectType {
                properties: members,
                string_index_type: None,
                number_index_type: None,
                call_signature: None,
                call_signature_overloads: Vec::new(),
                construct_signature: None,
                non_primitive: false,
            }))
        }
        _ => return None,
    })
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
/// TS2515/TS2654/TS2655: a class that is not itself abstract has to implement
/// every abstract member it inherits. tsc picks the code by how many are
/// missing — one names it, two to five list them, six or more list the first
/// four and count the rest — and names the *direct* base class.
///
/// Base resolution is conservative in the same way `check_implicit_override`
/// is: a base that does not resolve to a source-declared class leaves the whole
/// check quiet rather than risking a false positive.
fn check_inherited_abstract_members(class: &ParsedClassDeclaration, ctx: &mut CheckerContext) {
    const LISTED_WHEN_TRUNCATED: usize = 4;
    const MAX_LISTED: usize = 5;

    if class.is_abstract || class.is_declare {
        return;
    }
    let Some(base) = class.extends.first() else {
        return;
    };

    let mut satisfied: std::collections::HashSet<String> = class
        .members
        .iter()
        .filter_map(implemented_member_name)
        .collect();
    satisfied.extend(
        constructor_parameter_property_members(class)
            .into_iter()
            .map(|member| member.name),
    );

    let mut missing: Vec<String> = Vec::new();
    let mut seen: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut next_base = Some(base.name.clone());
    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();

    while let Some(base_name) = next_base.take() {
        if !visited.insert(base_name.clone()) {
            break;
        }
        let Some(TypeDeclarationInfo::Interface(info)) = ctx.lookup_type_declaration(&base_name)
        else {
            return;
        };
        // A base whose declaration surge resolved out of a declaration file may
        // not be the class the oracle sees; stay quiet rather than guess.
        if info.file_name.ends_with(".d.ts") {
            return;
        }
        for member in &info.body.members {
            if satisfied.contains(&member.name) || !seen.insert(member.name.clone()) {
                continue;
            }
            if member.is_abstract {
                missing.push(member.name.clone());
            }
        }
        next_base = info.body.extends.first().map(|parent| parent.name.clone());
    }

    if missing.is_empty() {
        return;
    }

    // tsc names the base as written, type arguments included (`Base<string>`).
    let base_display = if base.type_arguments.is_empty() {
        base.name.clone()
    } else {
        let arguments: Vec<String> = base
            .type_arguments
            .iter()
            .map(|argument| map_parsed_type(argument.clone(), ctx).name())
            .collect();
        format!("{}<{}>", base.name, arguments.join(", "))
    };
    let quoted = |names: &[String]| {
        names
            .iter()
            .map(|name| format!("'{name}'"))
            .collect::<Vec<_>>()
            .join(", ")
    };
    let diagnostic = match missing.len() {
        1 => Diagnostic::ts2515(
            &class.name,
            &missing[0],
            &base_display,
            ctx.file_name.clone(),
        ),
        count if count <= MAX_LISTED => Diagnostic::ts2654(
            &class.name,
            &base_display,
            quoted(&missing),
            ctx.file_name.clone(),
        ),
        count => Diagnostic::ts2655(
            &class.name,
            &base_display,
            quoted(&missing[..LISTED_WHEN_TRUNCATED]),
            count - LISTED_WHEN_TRUNCATED,
            ctx.file_name.clone(),
        ),
    };
    let diagnostic = match class.name_span {
        Some(span) => diagnostic.with_span(convert_span(span)),
        None => diagnostic,
    };
    ctx.push(diagnostic);
}

/// The name a class member declares when it is not itself abstract — what
/// implements an inherited abstract member of the same name.
fn implemented_member_name(member: &ParsedClassMember) -> Option<String> {
    match member {
        ParsedClassMember::Property(property) if !property.is_abstract && !property.is_static => {
            Some(property.name.clone())
        }
        ParsedClassMember::Method(method) if !method.is_abstract && !method.is_static => {
            Some(method.name.clone())
        }
        ParsedClassMember::Accessor(accessor) if !accessor.is_abstract && !accessor.is_static => {
            Some(accessor.name.clone())
        }
        _ => None,
    }
}

/// TS2420: a class has to declare every required member of each interface it
/// implements — an `implements` clause contributes nothing to the class, it
/// only constrains it.
///
/// Only the *missing member* half of tsc's check runs here; a member that is
/// present but whose type does not match is TS2416, which surge does not report
/// yet. Interface resolution is conservative in the same way the abstract-member
/// check is: anything that does not resolve to a source-declared interface
/// leaves that clause unchecked.
fn check_implemented_interfaces(class: &ParsedClassDeclaration, ctx: &mut CheckerContext) {
    if class.implements.is_empty() || class.is_declare {
        return;
    }

    let Some(declared) = declared_instance_member_names(class, ctx) else {
        // Part of the class's own surface is out of reach — a base class from a
        // declaration file, an unresolved heritage name — so a member that looks
        // missing may simply be inherited. Report nothing.
        return;
    };

    for implemented in &class.implements {
        // tsc reports the member-specific errors first and falls back to the
        // broad one only when that walk found nothing. A clause carrying type
        // arguments is left to the broad check: resolving the interface by name
        // alone would compare the class against its *uninstantiated* members
        // (`v: number` against `v: T`).
        if implemented.type_arguments.is_empty()
            && super::heritage::report_incompatible_heritage_members(class, &implemented.name, ctx)
        {
            continue;
        }
        let Some(required) = unimplemented_interface_members(&implemented.name, &declared, ctx)
        else {
            continue;
        };
        if required.is_empty() {
            continue;
        }
        let diagnostic =
            Diagnostic::ts2420(&class.name, &implemented.name, ctx.file_name.clone());
        let diagnostic = match class.name_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        };
        ctx.push(diagnostic);
    }
}

/// TS2416 for a member that overrides a base *class* member incompatibly.
///
/// Only the member-specific half of tsc's `extends` check runs here. The broad
/// TS2415 needs a whole-type relation against the base plus the missing-member
/// notion the `implements` path has, and surge's class instance surfaces are
/// not complete enough for that to be free of false positives.
fn check_extended_base_class(class: &ParsedClassDeclaration, ctx: &mut CheckerContext) {
    if class.is_declare {
        return;
    }
    for base in &class.extends {
        if !base.type_arguments.is_empty() {
            continue;
        }
        super::heritage::report_incompatible_heritage_members(class, &base.name, ctx);
    }
}

/// Every instance member name the class has: what its body declares, what a
/// same-named interface merged into it declares, and what it inherits. `None`
/// when any part of that chain is not source-declared, which is what keeps an
/// inherited member surge cannot see from reading as a missing one.
fn declared_instance_member_names(
    class: &ParsedClassDeclaration,
    ctx: &CheckerContext,
) -> Option<std::collections::HashSet<String>> {
    let mut declared: std::collections::HashSet<String> = class
        .members
        .iter()
        .filter_map(declared_member_name)
        .collect();
    declared.extend(
        constructor_parameter_property_members(class)
            .into_iter()
            .map(|member| member.name),
    );

    // The instance-side declaration surge built for this class is where a
    // merged `interface C {}` of the same name lands, heritage included.
    let mut stack: Vec<String> = class
        .extends
        .iter()
        .map(|base| base.name.clone())
        .collect();
    if let Some(TypeDeclarationInfo::Interface(info)) = ctx.lookup_type_declaration(&class.name) {
        for member in &info.body.members {
            declared.insert(member.name.clone());
        }
        stack.extend(info.body.extends.iter().map(|base| base.name.clone()));
    }

    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();
    while let Some(name) = stack.pop() {
        if !visited.insert(name.clone()) {
            continue;
        }
        let Some(TypeDeclarationInfo::Interface(info)) = ctx.lookup_type_declaration(&name) else {
            return None;
        };
        if info.file_name.ends_with(".d.ts") {
            return None;
        }
        for member in &info.body.members {
            declared.insert(member.name.clone());
        }
        stack.extend(info.body.extends.iter().map(|base| base.name.clone()));
    }

    Some(declared)
}

/// The required members of `interface_name` (its own and its bases') that
/// `declared` does not cover. `None` when the interface, or any interface it
/// extends, is not a source-declared object shape.
fn unimplemented_interface_members(
    interface_name: &str,
    declared: &std::collections::HashSet<String>,
    ctx: &CheckerContext,
) -> Option<Vec<String>> {
    let mut required = Vec::new();
    let mut stack = vec![interface_name.to_string()];
    let mut visited: std::collections::HashSet<String> = std::collections::HashSet::new();

    while let Some(name) = stack.pop() {
        if !visited.insert(name.clone()) {
            continue;
        }
        match ctx.lookup_type_declaration(&name) {
            Some(TypeDeclarationInfo::Interface(info)) => {
                if info.file_name.ends_with(".d.ts") {
                    return None;
                }
                for member in &info.body.members {
                    if !member.optional && !declared.contains(&member.name) {
                        required.push(member.name.clone());
                    }
                }
                stack.extend(info.body.extends.iter().map(|base| base.name.clone()));
            }
            // `implements SomeAlias` where the alias is written as an object
            // type is the same constraint; any other alias body (a union, a
            // conditional, a generic instantiation) is left alone.
            Some(TypeDeclarationInfo::Alias(alias))
                if alias.body.type_parameters.is_empty()
                    && !alias.file_name.ends_with(".d.ts") =>
            {
                let surge_ts_syntax::ParsedType::Object(object) = &alias.body.ty else {
                    return None;
                };
                for property in &object.properties {
                    if !property.optional && !declared.contains(&property.name) {
                        required.push(property.name.clone());
                    }
                }
            }
            _ => return None,
        }
    }

    Some(required)
}

/// Every instance member name a class body declares, `abstract` ones included:
/// an abstract member is a declaration, which is what an `implements` clause
/// asks for.
fn declared_member_name(member: &ParsedClassMember) -> Option<String> {
    match member {
        ParsedClassMember::Property(property) if !property.is_static => Some(property.name.clone()),
        ParsedClassMember::Method(method) if !method.is_static => Some(method.name.clone()),
        ParsedClassMember::Accessor(accessor) if !accessor.is_static => {
            Some(accessor.name.clone())
        }
        _ => None,
    }
}

pub(crate) fn check_class_declaration(class: &ParsedClassDeclaration, ctx: &mut CheckerContext) {
    check_inherited_abstract_members(class, ctx);
    check_extended_base_class(class, ctx);
    check_implemented_interfaces(class, ctx);
    super::property_initialization::check_property_initialization(class, ctx);
    super::forward_references::check_class_property_initializers(class, ctx);

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
                // A bodyless method is `abstract`, an overload signature, or a
                // member of an ambient class: there is no body to check, and
                // checking the absent one reported TS2355 for a non-void return
                // the implementation below actually satisfies. The same guard
                // has always been on the function-declaration path
                // (`check_function_declaration`); class members never had it.
                // The signature above is still mapped, so its annotations are
                // resolved and reported either way.
                if !method.has_body {
                    continue;
                }
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
                    method.return_type_span.or(method.name_span),
                    Some(this_type),
                    false,
                    method.has_body.then(|| method.body_reads.as_slice()),
                    method.is_generator,
                    false,
                    ctx,
                );
            }
            ParsedClassMember::Property(property) => {
                let this_type = if property.is_static {
                    static_type.clone()
                } else {
                    instance_type.clone()
                };
                check_class_property_initializer(property, this_type, ctx);
            }
            ParsedClassMember::Accessor(_) => {}
        }
    }
}

/// tsc checks a property initializer as it checks a variable's
/// (`checkVariableLikeDeclaration`): against the annotation when there is one,
/// with `this` bound to the instance — or the constructor, for a static.
fn check_class_property_initializer(
    property: &ParsedClassProperty,
    this_type: Type,
    ctx: &mut CheckerContext,
) {
    let Some(initializer) = &property.initializer else {
        return;
    };
    let mut symbols = ctx
        .symbols
        .clone_with_reason(surge_ts_types::TypeCopyReason::ScopeOrContext);
    let _ = symbols.insert(
        "this".to_string(),
        SymbolInfo {
            ty: this_type,
            kind: SymbolKind::Const,
            function_signature: None,
        },
    );
    let Some(declared_type) = property.declared_type.clone() else {
        crate::checks::expr::evaluate_expression(
            initializer,
            property.initializer_span,
            &symbols,
            ctx,
        );
        return;
    };
    let declared_type = map_parsed_type(declared_type, ctx);
    let inferred = crate::checks::expected::evaluate_expression_with_expected_type_anchored(
        initializer,
        property.initializer_span,
        property.name_span,
        Some(&declared_type),
        crate::checks::expected::ExpectedTypeDiagnostic::TypeNotAssignable,
        &symbols,
        ctx,
    );
    if let crate::infer::InferredExpression::Known(inferred_type) = inferred {
        crate::checks::var::report_initializer_mismatch(
            &inferred_type,
            &declared_type,
            property.name_span.or(property.initializer_span),
            ctx,
        );
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
