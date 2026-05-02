use std::collections::BTreeMap;

use typescript_rust_diagnostics::{Diagnostic, DiagnosticCode};
use typescript_rust_syntax::{
    ParsedFunctionType, ParsedFunctionTypeParameter, ParsedInterfaceMember, ParsedNamedType,
    ParsedObjectType, ParsedType, TextSpan,
};
use typescript_rust_types::{
    FunctionType, NumberLiteralType, ObjectProperty, ObjectType, Type, union_type,
};

use crate::context::{CheckerContext, convert_span};
use crate::symbols::{InterfaceInfo, TypeAliasInfo, TypeDeclarationInfo};

#[derive(Debug, Clone)]
struct ResolvedType {
    ty: Type,
    had_error: bool,
}

pub(crate) fn map_parsed_type(parsed_type: ParsedType, ctx: &mut CheckerContext) -> Type {
    let mut resolving = Vec::new();
    resolve_parsed_type(parsed_type, ctx, &mut resolving).ty
}

fn resolve_parsed_type(
    parsed_type: ParsedType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<String>,
) -> ResolvedType {
    match parsed_type {
        ParsedType::String => ResolvedType {
            ty: Type::String,
            had_error: false,
        },
        ParsedType::Number => ResolvedType {
            ty: Type::Number,
            had_error: false,
        },
        ParsedType::Boolean => ResolvedType {
            ty: Type::Boolean,
            had_error: false,
        },
        ParsedType::Undefined => ResolvedType {
            ty: Type::Undefined,
            had_error: false,
        },
        ParsedType::Any => ResolvedType {
            ty: Type::Any,
            had_error: false,
        },
        ParsedType::Unknown => ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        },
        ParsedType::StringLiteral(value) => ResolvedType {
            ty: Type::StringLiteral(value),
            had_error: false,
        },
        ParsedType::NumberLiteral(value) => ResolvedType {
            ty: Type::NumberLiteral(NumberLiteralType { value }),
            had_error: false,
        },
        ParsedType::BooleanLiteral(value) => ResolvedType {
            ty: Type::BooleanLiteral(value),
            had_error: false,
        },
        ParsedType::Void => ResolvedType {
            ty: Type::Void,
            had_error: false,
        },
        ParsedType::Object(object_type) => resolve_object_type(object_type, ctx, resolving),
        ParsedType::Array(element_type) => {
            let resolved_element = resolve_parsed_type(*element_type, ctx, resolving);
            if resolved_element.had_error {
                return ResolvedType {
                    ty: Type::Unknown,
                    had_error: true,
                };
            }

            ResolvedType {
                ty: Type::Array(Box::new(resolved_element.ty)),
                had_error: false,
            }
        }
        ParsedType::Union(types) => resolve_union_type(types, ctx, resolving),
        ParsedType::Function(function_type) => resolve_function_type(function_type, ctx, resolving),
        ParsedType::Named(named_type) => resolve_named_type(named_type, ctx, resolving),
    }
}

fn resolve_function_type(
    function_type: ParsedFunctionType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<String>,
) -> ResolvedType {
    let mut parameters = Vec::new();

    for parameter in function_type.parameters {
        let resolved_parameter = resolve_function_type_parameter(parameter, ctx, resolving);
        if resolved_parameter.had_error {
            return ResolvedType {
                ty: Type::Unknown,
                had_error: true,
            };
        }

        parameters.push(resolved_parameter.ty);
    }

    let return_type = resolve_parsed_type(*function_type.return_type, ctx, resolving);
    if return_type.had_error {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    }

    ResolvedType {
        ty: Type::Function(FunctionType {
            parameters,
            return_type: Box::new(return_type.ty),
        }),
        had_error: false,
    }
}

fn resolve_function_type_parameter(
    parameter: ParsedFunctionTypeParameter,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<String>,
) -> ResolvedType {
    let ParsedFunctionTypeParameter { ty, .. } = parameter;
    let resolved = resolve_parsed_type(ty, ctx, resolving);
    if resolved.had_error {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    }

    ResolvedType {
        ty: resolved.ty,
        had_error: false,
    }
}

fn resolve_object_type(
    object_type: ParsedObjectType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<String>,
) -> ResolvedType {
    let mut properties = BTreeMap::new();

    for property in object_type.properties {
        let property_type = resolve_parsed_type(property.ty, ctx, resolving);
        if property_type.had_error {
            return ResolvedType {
                ty: Type::Unknown,
                had_error: true,
            };
        }

        let object_property = if property.optional {
            ObjectProperty::optional(property_type.ty)
        } else {
            ObjectProperty::required(property_type.ty)
        };

        properties.insert(property.name, object_property);
    }

    ResolvedType {
        ty: Type::Object(ObjectType { properties }),
        had_error: false,
    }
}

fn resolve_union_type(
    types: Vec<ParsedType>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<String>,
) -> ResolvedType {
    let mut resolved_types = Vec::new();

    for ty in types {
        let resolved = resolve_parsed_type(ty, ctx, resolving);
        if resolved.had_error {
            return ResolvedType {
                ty: Type::Unknown,
                had_error: true,
            };
        }

        resolved_types.push(resolved.ty);
    }

    ResolvedType {
        ty: union_type(resolved_types),
        had_error: false,
    }
}

fn resolve_named_type(
    named_type: ParsedNamedType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<String>,
) -> ResolvedType {
    let Some(declaration) = ctx.type_declarations.get(&named_type.name).cloned() else {
        emit_unknown_type_name(&named_type, ctx);
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    };

    match declaration {
        TypeDeclarationInfo::Alias(alias) => resolve_type_alias(alias, ctx, resolving),
        TypeDeclarationInfo::Interface(interface) => resolve_interface(interface, ctx, resolving),
    }
}

fn resolve_type_alias(
    alias: TypeAliasInfo,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<String>,
) -> ResolvedType {
    if resolving.iter().any(|name| name == &alias.name) {
        emit_type_alias_cycle(&alias.name, alias.name_span, ctx);
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    }

    resolving.push(alias.name.clone());
    let resolved = resolve_parsed_type(alias.ty, ctx, resolving);
    resolving.pop();

    if resolved.had_error {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    }

    resolved
}

fn resolve_interface(
    interface: InterfaceInfo,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<String>,
) -> ResolvedType {
    if resolving.iter().any(|name| name == &interface.name) {
        emit_type_declaration_cycle(&interface.name, interface.name_span, ctx);
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    }

    resolving.push(interface.name.clone());
    let resolved = resolve_interface_members(&interface.members, ctx, resolving);
    resolving.pop();

    if resolved.had_error {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    }

    resolved
}

fn resolve_interface_members(
    members: &[ParsedInterfaceMember],
    ctx: &mut CheckerContext,
    resolving: &mut Vec<String>,
) -> ResolvedType {
    let mut properties = BTreeMap::new();

    for member in members {
        let property_type = resolve_parsed_type(member.ty.clone(), ctx, resolving);
        if property_type.had_error {
            return ResolvedType {
                ty: Type::Unknown,
                had_error: true,
            };
        }

        let object_property = if member.optional {
            ObjectProperty::optional(property_type.ty)
        } else {
            ObjectProperty::required(property_type.ty)
        };

        properties.insert(member.name.clone(), object_property);
    }

    ResolvedType {
        ty: Type::Object(ObjectType { properties }),
        had_error: false,
    }
}

fn emit_unknown_type_name(named_type: &ParsedNamedType, ctx: &mut CheckerContext) {
    let mut diagnostic = Diagnostic::ts2304(&named_type.name, ctx.file_name.clone());
    if let Some(span) = named_type.span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }
    ctx.push(diagnostic);
}

fn emit_type_alias_cycle(name: &str, name_span: Option<TextSpan>, ctx: &mut CheckerContext) {
    let mut diagnostic = Diagnostic::new(
        DiagnosticCode::Custom("typescript-rust::type-alias-cycle"),
        format!("Type alias '{name}' circularly references itself."),
        ctx.file_name.clone(),
    );

    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
}

fn emit_type_declaration_cycle(name: &str, name_span: Option<TextSpan>, ctx: &mut CheckerContext) {
    let mut diagnostic = Diagnostic::new(
        DiagnosticCode::Custom("typescript-rust::type-declaration-cycle"),
        format!("Type declaration '{name}' circularly references itself."),
        ctx.file_name.clone(),
    );

    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
}
