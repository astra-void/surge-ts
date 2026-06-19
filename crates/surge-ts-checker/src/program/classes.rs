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

use crate::checks::function::{
    check_function_body_with_signature_and_this, map_function_signature,
};
use crate::context::CheckerContext;
use crate::infer::map_parsed_type;
use crate::symbols::{InterfaceInfo, SymbolInfo, SymbolKind, TypeDeclarationInfo};

/// Builds the instance-side interface (fields + instance methods) for a class.
/// Static members and the constructor are excluded; they live on the value side.
pub(crate) fn class_instance_interface_info(
    class: &ParsedClassDeclaration,
    file_name: String,
) -> InterfaceInfo {
    let members = class
        .members
        .iter()
        .filter_map(class_member_to_interface_member)
        .collect();

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
                ty: property.declared_type.clone().unwrap_or(ParsedType::Any),
            })
        }
        ParsedClassMember::Method(method) if !method.is_static => Some(ParsedInterfaceMember {
            name: method.name.clone(),
            name_span: method.name_span,
            optional: false,
            ty: method_function_type(method),
        }),
        ParsedClassMember::Accessor(accessor) if !accessor.is_static => {
            Some(ParsedInterfaceMember {
                name: accessor.name.clone(),
                name_span: accessor.name_span,
                optional: false,
                ty: accessor_property_type(accessor),
            })
        }
        _ => None,
    }
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
    ParsedType::Function(ParsedFunctionType {
        parameters: method
            .parameters
            .iter()
            .map(parameter_to_type_parameter)
            .collect(),
        return_type: Box::new(method.return_type.clone().unwrap_or(ParsedType::Any)),
        type_parameters: method.type_parameters.clone(),
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

/// Builds the constructor/static-side value symbol: a `Type::Object` whose
/// properties are the static members and whose construct signature yields the
/// instance type.
pub(crate) fn build_class_value_symbol(
    class: &ParsedClassDeclaration,
    ctx: &mut CheckerContext,
) -> SymbolInfo {
    // Generic classes are out of scope for this slice. Model their value side as
    // `any` so `new C<T>(...)` and `C.member` stay non-cascading rather than
    // resolving the self type without type arguments (which would mis-report).
    if !class.type_parameters.is_empty() {
        return SymbolInfo {
            ty: Type::Any,
            kind: SymbolKind::Const,
            function_signature: None,
        };
    }

    let instance_type = class_instance_type(class, ctx);
    let construct_signature = class_construct_signature(class, instance_type, ctx);

    let mut properties = PropertyMap::new();
    for member in &class.members {
        match member {
            ParsedClassMember::Property(property) if property.is_static => {
                let property_type = static_property_type(property, ctx);
                let object_property = if property.optional {
                    ObjectProperty::optional(property_type)
                } else {
                    ObjectProperty::required(property_type)
                };
                properties.insert(property.name.clone(), object_property);
            }
            ParsedClassMember::Method(method) if method.is_static => {
                let function_type = map_function_signature(
                    &method.parameters,
                    method.return_type.as_ref(),
                    &method.type_parameters,
                    None,
                    ctx,
                );
                properties.insert(
                    method.name.clone(),
                    ObjectProperty::required(Type::Function(function_type)),
                );
            }
            ParsedClassMember::Accessor(accessor) if accessor.is_static => {
                let property_type = map_parsed_type(accessor_property_type(accessor), ctx);
                properties.insert(
                    accessor.name.clone(),
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

fn static_property_type(property: &ParsedClassProperty, ctx: &mut CheckerContext) -> Type {
    match property.declared_type.clone() {
        Some(declared_type) => map_parsed_type(declared_type, ctx),
        None => Type::Any,
    }
}

fn class_instance_type(class: &ParsedClassDeclaration, ctx: &mut CheckerContext) -> Type {
    map_parsed_type(
        ParsedType::Named(ParsedNamedType {
            name: class.name.clone(),
            span: class.name_span,
            type_arguments: Vec::new(),
        }),
        ctx,
    )
}

fn class_construct_signature(
    class: &ParsedClassDeclaration,
    instance_type: Type,
    ctx: &mut CheckerContext,
) -> FunctionType {
    let named_instance = ParsedType::Named(ParsedNamedType {
        name: class.name.clone(),
        span: class.name_span,
        type_arguments: Vec::new(),
    });

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

    for member in &class.members {
        match member {
            ParsedClassMember::Constructor(constructor) => {
                let function_type =
                    map_function_signature(&constructor.parameters, None, &[], None, ctx);
                check_function_body_with_signature_and_this(
                    "constructor".to_string(),
                    constructor.parameters.clone(),
                    constructor.body.clone(),
                    &function_type,
                    &[],
                    None,
                    false,
                    None,
                    Some(instance_type.clone()),
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
                    method.name.clone(),
                    method.parameters.clone(),
                    method.body.clone(),
                    &function_type,
                    &method.type_parameters,
                    None,
                    method.return_type.is_some(),
                    method.name_span,
                    Some(this_type),
                    ctx,
                );
            }
            ParsedClassMember::Property(_) | ParsedClassMember::Accessor(_) => {}
        }
    }
}

/// Inserts a class's instance-side interface into the current type-declaration
/// table. Mirrors `collect_interface` for first-wins / duplicate behaviour.
pub(crate) fn collect_class(class: &ParsedClassDeclaration, ctx: &mut CheckerContext) {
    let info = class_instance_interface_info(class, ctx.file_name.clone());
    let _ = ctx
        .type_declarations
        .insert(class.name.clone(), TypeDeclarationInfo::Interface(info));
}
