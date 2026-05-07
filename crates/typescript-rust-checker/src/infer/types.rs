use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{
    ParsedFunctionType, ParsedFunctionTypeParameter, ParsedInterfaceMember, ParsedMappedType,
    ParsedNamedType, ParsedObjectType, ParsedType, ParsedTypeParameter, TextSpan,
};
use typescript_rust_types::{
    FunctionType, NumberLiteralType, ObjectProperty, ObjectType, Type, union_type,
};

use crate::context::{CheckerContext, convert_span};
use crate::symbols::{InterfaceInfo, TypeAliasInfo, TypeDeclarationInfo};

pub(crate) type TypeParameterSubstitution = BTreeMap<String, Type>;

pub(crate) fn report_duplicate_type_parameters(
    type_parameters: &[ParsedTypeParameter],
    ctx: &mut CheckerContext,
) {
    let mut seen = HashSet::new();

    for type_parameter in type_parameters {
        if !seen.insert(type_parameter.name.clone()) {
            let mut diagnostic = Diagnostic::typescript_rust_duplicate_type_parameter(
                type_parameter.name.clone(),
                ctx.file_name.clone(),
            );

            if let Some(span) = type_parameter.name_span.or(type_parameter.span) {
                diagnostic = diagnostic.with_span(convert_span(span));
            }

            ctx.push(diagnostic);
        }
    }
}

#[derive(Debug, Clone)]
struct ResolvedType {
    ty: Type,
    had_error: bool,
}

pub(crate) fn map_parsed_type(parsed_type: ParsedType, ctx: &mut CheckerContext) -> Type {
    map_parsed_type_with_substitution(parsed_type, ctx, &TypeParameterSubstitution::new())
}

pub(crate) fn map_parsed_type_with_substitution(
    parsed_type: ParsedType,
    ctx: &mut CheckerContext,
    substitution: &TypeParameterSubstitution,
) -> Type {
    let mut resolving = Vec::new();
    resolve_parsed_type(parsed_type, ctx, &mut resolving, substitution).ty
}

fn get_parsed_type_name(parsed: &ParsedType) -> String {
    match parsed {
        ParsedType::Named(named) => named.name.clone(),
        ParsedType::TypeOf(type_of) => type_of.name.clone(),
        ParsedType::String => "string".to_string(),
        ParsedType::Number => "number".to_string(),
        ParsedType::Boolean => "boolean".to_string(),
        ParsedType::Any => "any".to_string(),
        ParsedType::Unknown => "unknown".to_string(),
        ParsedType::Void => "void".to_string(),
        ParsedType::Undefined => "undefined".to_string(),
        _ => "unknown".to_string(),
    }
}

fn resolve_parsed_type(
    parsed_type: ParsedType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<String>,
    substitution: &TypeParameterSubstitution,
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
        ParsedType::Object(object_type) => {
            resolve_object_type(object_type, ctx, resolving, substitution)
        }
        ParsedType::Array(element_type) => {
            let resolved_element = resolve_parsed_type(*element_type, ctx, resolving, substitution);
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
        ParsedType::Tuple(elements) => resolve_tuple_type(elements, ctx, resolving, substitution),
        ParsedType::Union(types) => resolve_union_type(types, ctx, resolving, substitution),
        ParsedType::Function(function_type) => {
            resolve_function_type(function_type, ctx, resolving, substitution)
        }
        ParsedType::Named(named_type) => {
            resolve_named_type(named_type, ctx, resolving, substitution)
        }
        ParsedType::TypeOf(type_of) => {
            let symbol = ctx
                .symbols
                .get(&type_of.name)
                .cloned()
                .or_else(|| ctx.ambient_global_symbols.get(&type_of.name).cloned());

            let Some(symbol) = symbol else {
                let mut diagnostic = Diagnostic::ts2304(&type_of.name, ctx.file_name.clone());
                if let Some(span) = type_of.name_span {
                    diagnostic = diagnostic.with_span(convert_span(span));
                }
                ctx.push(diagnostic);

                return ResolvedType {
                    ty: Type::Unknown,
                    had_error: true,
                };
            };

            ResolvedType {
                ty: symbol.ty,
                had_error: false,
            }
        }
        ParsedType::KeyOf(inner) => {
            let resolved_inner = resolve_parsed_type(*inner, ctx, resolving, substitution);
            if resolved_inner.had_error {
                return ResolvedType {
                    ty: Type::Unknown,
                    had_error: true,
                };
            }

            let mut keys = Vec::new();
            match &resolved_inner.ty {
                Type::Object(object_type) => {
                    for key in object_type.properties.keys() {
                        keys.push(Type::StringLiteral(key.clone()));
                    }
                }
                _ => {
                    return ResolvedType {
                        ty: Type::Unknown,
                        had_error: false,
                    };
                }
            }

            ResolvedType {
                ty: if keys.is_empty() {
                    Type::Unknown
                } else if keys.len() == 1 {
                    keys.into_iter().next().unwrap()
                } else {
                    union_type(keys)
                },
                had_error: false,
            }
        }
        ParsedType::Mapped(mapped) => resolve_mapped_type(mapped, ctx, resolving, substitution),
        ParsedType::IndexedAccess(indexed_access) => {
            let resolved_object =
                resolve_parsed_type(*indexed_access.object_type, ctx, resolving, substitution);
            let resolved_index = resolve_parsed_type(
                *indexed_access.index_type.clone(),
                ctx,
                resolving,
                substitution,
            );

            if resolved_object.had_error || resolved_index.had_error {
                if resolved_index.had_error
                    && ctx.options.diagnostic_profile == crate::context::DiagnosticProfile::Tsc
                {
                    let mut diagnostic = Diagnostic::ts2538(
                        &get_parsed_type_name(&indexed_access.index_type),
                        ctx.file_name.clone(),
                    );
                    if let Some(span) = indexed_access.span {
                        diagnostic = diagnostic.with_span(convert_span(span));
                    }
                    ctx.push(diagnostic);
                }
                return ResolvedType {
                    ty: Type::Unknown,
                    had_error: true,
                };
            }

            match (&resolved_object.ty, &resolved_index.ty) {
                (Type::Object(object_type), Type::StringLiteral(key)) => {
                    if let Some(property_ty) = object_type.get_property_access_type(&key) {
                        ResolvedType {
                            ty: property_ty,
                            had_error: false,
                        }
                    } else {
                        let mut diagnostic = Diagnostic::ts2339(
                            key,
                            &resolved_object.ty.name(),
                            ctx.file_name.clone(),
                        );
                        if let Some(span) = indexed_access.span {
                            diagnostic = diagnostic.with_span(convert_span(span));
                        }
                        ctx.push(diagnostic);
                        ResolvedType {
                            ty: Type::Unknown,
                            had_error: true,
                        }
                    }
                }
                (Type::Object(object_type), Type::Union(union_ty)) => {
                    let mut types = Vec::new();
                    let mut had_error = false;
                    for key_ty in &union_ty.types {
                        if let Type::StringLiteral(key) = key_ty {
                            if let Some(property_ty) = object_type.get_property_access_type(key) {
                                types.push(property_ty);
                            } else {
                                let mut diagnostic = Diagnostic::ts2339(
                                    key,
                                    &resolved_object.ty.name(),
                                    ctx.file_name.clone(),
                                );
                                if let Some(span) = indexed_access.span {
                                    diagnostic = diagnostic.with_span(convert_span(span));
                                }
                                ctx.push(diagnostic);
                                had_error = true;
                            }
                        } else {
                            let mut diagnostic =
                                Diagnostic::ts2538(&key_ty.name(), ctx.file_name.clone());
                            if let Some(span) = indexed_access.span {
                                diagnostic = diagnostic.with_span(convert_span(span));
                            }
                            ctx.push(diagnostic);
                            had_error = true;
                        }
                    }

                    if had_error {
                        ResolvedType {
                            ty: Type::Unknown,
                            had_error: true,
                        }
                    } else {
                        ResolvedType {
                            ty: union_type(types),
                            had_error: false,
                        }
                    }
                }
                (Type::Tuple(elements), Type::NumberLiteral(num)) => {
                    if let Ok(index) = num.value.parse::<usize>() {
                        if let Some(element_ty) = elements.get(index) {
                            ResolvedType {
                                ty: element_ty.clone(),
                                had_error: false,
                            }
                        } else {
                            let mut diagnostic = Diagnostic::ts2493(
                                &resolved_object.ty.name(),
                                elements.len(),
                                index,
                                ctx.file_name.clone(),
                            );
                            if let Some(span) = indexed_access.span {
                                diagnostic = diagnostic.with_span(convert_span(span));
                            }
                            ctx.push(diagnostic);
                            ResolvedType {
                                ty: Type::Unknown,
                                had_error: true,
                            }
                        }
                    } else {
                        ResolvedType {
                            ty: Type::Unknown,
                            had_error: true,
                        }
                    }
                }
                (Type::Array(element_type), Type::Number) => ResolvedType {
                    ty: *element_type.clone(),
                    had_error: false,
                },
                (Type::Tuple(elements), Type::Number) => ResolvedType {
                    ty: union_type(elements.clone()),
                    had_error: false,
                },
                (Type::Any, _) | (_, Type::Any) => ResolvedType {
                    ty: Type::Any,
                    had_error: false,
                },
                (_, Type::StringLiteral(key)) => {
                    let mut diagnostic =
                        Diagnostic::ts2339(key, &resolved_object.ty.name(), ctx.file_name.clone());
                    if let Some(span) = indexed_access.span {
                        diagnostic = diagnostic.with_span(convert_span(span));
                    }
                    ctx.push(diagnostic);
                    ResolvedType {
                        ty: Type::Unknown,
                        had_error: true,
                    }
                }
                (_, invalid_index) => {
                    // For Type::Unknown index (like UnresolvedKey), avoid cascading TS2538 if possible
                    if let Type::Unknown = invalid_index {
                        return ResolvedType {
                            ty: Type::Unknown,
                            had_error: true,
                        };
                    }
                    let mut diagnostic =
                        Diagnostic::ts2538(&invalid_index.name(), ctx.file_name.clone());
                    if let Some(span) = indexed_access.span {
                        diagnostic = diagnostic.with_span(convert_span(span));
                    }
                    ctx.push(diagnostic);
                    ResolvedType {
                        ty: Type::Unknown,
                        had_error: true,
                    }
                }
            }
        }
    }
}

fn resolve_tuple_type(
    elements: Vec<ParsedType>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<String>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let mut resolved_elements = Vec::new();

    for element in elements {
        let resolved_element = resolve_parsed_type(element, ctx, resolving, substitution);
        if resolved_element.had_error {
            return ResolvedType {
                ty: Type::Unknown,
                had_error: true,
            };
        }

        resolved_elements.push(resolved_element.ty);
    }

    ResolvedType {
        ty: Type::Tuple(resolved_elements),
        had_error: false,
    }
}

fn resolve_function_type(
    function_type: ParsedFunctionType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<String>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let local_substitution = extend_substitution_with_type_parameters(
        substitution,
        &function_type.type_parameters,
        ctx,
        resolving,
    );

    let mut parameters = Vec::new();

    for parameter in function_type.parameters {
        let resolved_parameter =
            resolve_function_type_parameter(parameter, ctx, resolving, &local_substitution);
        if resolved_parameter.had_error {
            return ResolvedType {
                ty: Type::Unknown,
                had_error: true,
            };
        }

        parameters.push(resolved_parameter.ty);
    }

    let return_type = resolve_parsed_type(
        *function_type.return_type,
        ctx,
        resolving,
        &local_substitution,
    );
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
            is_variadic: false,
        }),
        had_error: false,
    }
}

fn resolve_function_type_parameter(
    parameter: ParsedFunctionTypeParameter,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<String>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let ParsedFunctionTypeParameter { ty, .. } = parameter;
    let resolved = resolve_parsed_type(ty, ctx, resolving, substitution);
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
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let mut properties = BTreeMap::new();

    for property in object_type.properties {
        let property_type = resolve_parsed_type(property.ty, ctx, resolving, substitution);
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
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let mut resolved_types = Vec::new();

    for ty in types {
        let resolved = resolve_parsed_type(ty, ctx, resolving, substitution);
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
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    if let Some(ty) = substitution.get(&named_type.name) {
        return ResolvedType {
            ty: ty.clone(),
            had_error: false,
        };
    }

    let declaration = ctx
        .type_declarations
        .get(&named_type.name)
        .cloned()
        .or_else(|| {
            ctx.ambient_global_type_declarations
                .get(&named_type.name)
                .cloned()
        });

    let Some(declaration) = declaration else {
        emit_unknown_type_name(&named_type, ctx);
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    };

    match declaration {
        TypeDeclarationInfo::Alias(alias) => resolve_type_alias(
            alias,
            named_type.type_arguments,
            named_type.span,
            ctx,
            resolving,
            substitution,
        ),
        TypeDeclarationInfo::Interface(interface) => resolve_interface(
            interface,
            named_type.type_arguments,
            ctx,
            resolving,
            substitution,
        ),
    }
}

fn resolve_type_alias(
    alias: TypeAliasInfo,
    type_arguments: Vec<ParsedType>,
    reference_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<String>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    if resolving.iter().any(|name| name == &alias.name) {
        emit_type_alias_cycle(&alias.name, alias.name_span, ctx);
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    }

    resolving.push(alias.name.clone());
    let Some(local_substitution) = bind_type_arguments(
        &alias.type_parameters,
        type_arguments,
        &alias.name,
        alias.name_span,
        ctx,
        resolving,
        substitution,
    ) else {
        resolving.pop();
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    };

    if alias.file_name == "<built-in>" {
        if let Some(resolved) = resolve_builtin_utility_alias(
            &alias.name,
            &local_substitution,
            reference_span.or(alias.name_span),
            ctx,
        ) {
            resolving.pop();
            return resolved;
        }
    }

    if alias.file_name == "<built-in>" && (alias.name == "Array" || alias.name == "ReadonlyArray") {
        resolving.pop();
        let element_type = local_substitution.get("T").cloned().unwrap_or(Type::Any);
        return ResolvedType {
            ty: Type::Array(Box::new(element_type)),
            had_error: false,
        };
    }

    let resolved = with_type_declarations(&alias.resolution_scope, ctx, |ctx| {
        with_file_name(ctx, &alias.file_name, |ctx| {
            resolve_parsed_type_with_substitution(alias.ty, ctx, resolving, &local_substitution)
        })
    });
    resolving.pop();

    if resolved.had_error {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    }

    resolved
}

fn resolve_builtin_utility_alias(
    alias_name: &str,
    substitution: &TypeParameterSubstitution,
    name_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
) -> Option<ResolvedType> {
    match alias_name {
        "Partial" => Some(resolve_partial_utility_type(substitution)),
        "Record" => Some(resolve_record_utility_type(substitution)),
        "Pick" => Some(resolve_pick_utility_type(substitution, name_span, ctx)),
        "Omit" => Some(resolve_omit_utility_type(substitution)),
        _ => None,
    }
}

fn resolve_partial_utility_type(substitution: &TypeParameterSubstitution) -> ResolvedType {
    let Some(source_type) = substitution.get("T").cloned() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let Type::Object(object_type) = source_type else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let mut properties = BTreeMap::new();
    for (name, property) in object_type.properties {
        properties.insert(name, ObjectProperty::optional(property.ty));
    }

    ResolvedType {
        ty: Type::Object(ObjectType { properties }),
        had_error: false,
    }
}

fn resolve_record_utility_type(substitution: &TypeParameterSubstitution) -> ResolvedType {
    let Some(key_type) = substitution.get("K").cloned() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let Some(keys) = string_literal_union_keys(&key_type) else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let value_type = substitution.get("T").cloned().unwrap_or(Type::Unknown);
    let mut properties = BTreeMap::new();

    for key in keys {
        properties.insert(key, ObjectProperty::required(value_type.clone()));
    }

    ResolvedType {
        ty: Type::Object(ObjectType { properties }),
        had_error: false,
    }
}

fn resolve_pick_utility_type(
    substitution: &TypeParameterSubstitution,
    name_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
) -> ResolvedType {
    let Some(source_type) = substitution.get("T").cloned() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let Type::Object(object_type) = source_type else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let Some(key_type) = substitution.get("K").cloned() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let Some(keys) = string_literal_union_keys(&key_type) else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let mut properties = BTreeMap::new();
    for key in keys {
        let Some(property) = object_type.properties.get(&key) else {
            let key_type_name = key_type.name();
            let constraint_name = format!("keyof {}", Type::Object(object_type.clone()).name());
            let mut diagnostic =
                Diagnostic::ts2344(&key_type_name, &constraint_name, ctx.file_name.clone());
            if let Some(span) = name_span {
                diagnostic = diagnostic.with_span(convert_span(span));
            }
            ctx.push_utility_diagnostic_once(diagnostic);
            return ResolvedType {
                ty: Type::Unknown,
                had_error: true,
            };
        };

        properties.insert(key, property.clone());
    }

    ResolvedType {
        ty: Type::Object(ObjectType { properties }),
        had_error: false,
    }
}

fn resolve_omit_utility_type(substitution: &TypeParameterSubstitution) -> ResolvedType {
    let Some(source_type) = substitution.get("T").cloned() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let Type::Object(object_type) = source_type else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let Some(key_type) = substitution.get("K").cloned() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let Some(keys) = string_literal_union_keys(&key_type) else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let mut properties = BTreeMap::new();
    for (key, property) in object_type.properties {
        if keys.iter().any(|candidate| candidate == &key) {
            continue;
        }

        properties.insert(key, property);
    }

    ResolvedType {
        ty: Type::Object(ObjectType { properties }),
        had_error: false,
    }
}

fn string_literal_union_keys(ty: &Type) -> Option<Vec<String>> {
    match ty {
        Type::StringLiteral(value) => Some(vec![value.clone()]),
        Type::Union(union) => {
            let mut keys = Vec::new();
            for variant in &union.types {
                match variant {
                    Type::StringLiteral(value) => keys.push(value.clone()),
                    _ => return None,
                }
            }
            Some(keys)
        }
        _ => None,
    }
}

fn resolve_interface(
    interface: InterfaceInfo,
    type_arguments: Vec<ParsedType>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<String>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    if resolving.iter().any(|name| name == &interface.name) {
        emit_type_declaration_cycle(&interface.name, interface.name_span, ctx);
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    }

    resolving.push(interface.name.clone());
    let Some(local_substitution) = bind_type_arguments(
        &interface.type_parameters,
        type_arguments,
        &interface.name,
        interface.name_span,
        ctx,
        resolving,
        substitution,
    ) else {
        resolving.pop();
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    };

    let resolved = with_type_declarations(&interface.resolution_scope, ctx, |ctx| {
        with_file_name(ctx, &interface.file_name, |ctx| {
            resolve_interface_members(&interface.members, ctx, resolving, &local_substitution)
        })
    });
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
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let mut properties = BTreeMap::new();

    for member in members {
        let property_type = resolve_parsed_type(member.ty.clone(), ctx, resolving, substitution);
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

fn bind_type_arguments(
    type_parameters: &[ParsedTypeParameter],
    type_arguments: Vec<ParsedType>,
    name: &str,
    name_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<String>,
    parent_substitution: &TypeParameterSubstitution,
) -> Option<TypeParameterSubstitution> {
    if type_parameters.is_empty() {
        if !type_arguments.is_empty() {
            emit_type_is_not_generic(name, name_span, ctx);
            return None;
        }

        return Some(TypeParameterSubstitution::new());
    }

    if type_arguments.len() > type_parameters.len() {
        emit_generic_arity(name, type_parameters.len(), name_span, ctx);
        return None;
    }

    let mut substitution = TypeParameterSubstitution::new();

    for (index, parameter) in type_parameters.iter().enumerate() {
        if let Some(argument) = type_arguments.get(index) {
            let resolved_argument =
                resolve_parsed_type(argument.clone(), ctx, resolving, parent_substitution);
            if resolved_argument.had_error {
                return None;
            }

            substitution
                .entry(parameter.name.clone())
                .or_insert(resolved_argument.ty);
            continue;
        }

        let Some(default_type) = parameter.default_type.clone() else {
            emit_generic_arity(name, type_parameters.len(), name_span, ctx);
            return None;
        };

        let mut effective_substitution = parent_substitution.clone();
        effective_substitution.extend(substitution.clone());

        let resolved_default =
            resolve_parsed_type(default_type, ctx, resolving, &effective_substitution);
        if resolved_default.had_error {
            return None;
        }

        substitution
            .entry(parameter.name.clone())
            .or_insert(resolved_default.ty);
    }

    Some(substitution)
}

fn extend_substitution_with_type_parameters(
    parent_substitution: &TypeParameterSubstitution,
    type_parameters: &[ParsedTypeParameter],
    ctx: &mut CheckerContext,
    resolving: &mut Vec<String>,
) -> TypeParameterSubstitution {
    let mut substitution = parent_substitution.clone();

    for parameter in type_parameters {
        let mut effective_substitution = parent_substitution.clone();
        effective_substitution.extend(substitution.clone());

        let resolved = parameter.default_type.clone().map(|default_type| {
            resolve_parsed_type(default_type, ctx, resolving, &effective_substitution)
        });

        let ty = match resolved {
            Some(resolved) if !resolved.had_error => resolved.ty,
            Some(_) => Type::Unknown,
            None => Type::Unknown,
        };

        substitution.entry(parameter.name.clone()).or_insert(ty);
    }

    substitution
}

fn resolve_mapped_type(
    mapped: ParsedMappedType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<String>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let resolved_constraint = resolve_parsed_type(*mapped.constraint, ctx, resolving, substitution);

    if resolved_constraint.had_error {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    }

    let keys = match resolved_constraint.ty {
        Type::StringLiteral(s) => vec![s],
        Type::Union(union) => {
            let mut keys = Vec::new();
            for variant in union.types {
                match variant {
                    Type::StringLiteral(s) => keys.push(s),
                    _ => {
                        return ResolvedType {
                            ty: Type::Unknown,
                            had_error: false,
                        };
                    }
                }
            }
            keys
        }
        _ => {
            return ResolvedType {
                ty: Type::Unknown,
                had_error: false,
            };
        }
    };

    let mut properties = std::collections::BTreeMap::new();
    let mut had_error = false;

    for key in keys {
        let mut new_substitution = substitution.clone();
        new_substitution.insert(mapped.key_name.clone(), Type::StringLiteral(key.clone()));

        let resolved_value = resolve_parsed_type(
            *mapped.value_type.clone(),
            ctx,
            resolving,
            &new_substitution,
        );

        if resolved_value.had_error {
            had_error = true;
        }

        properties.insert(
            key,
            ObjectProperty {
                ty: resolved_value.ty,
                optional: mapped.optional,
            },
        );
    }

    ResolvedType {
        ty: Type::Object(ObjectType { properties }),
        had_error,
    }
}

fn resolve_parsed_type_with_substitution(
    parsed_type: ParsedType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<String>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    resolve_parsed_type(parsed_type, ctx, resolving, substitution)
}

fn emit_unknown_type_name(named_type: &ParsedNamedType, ctx: &mut CheckerContext) {
    let mut diagnostic = Diagnostic::ts2304(&named_type.name, ctx.file_name.clone());
    if let Some(span) = named_type.span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }
    ctx.push(diagnostic);
}

fn emit_type_is_not_generic(name: &str, name_span: Option<TextSpan>, ctx: &mut CheckerContext) {
    let mut diagnostic = Diagnostic::ts2315(name, ctx.file_name.clone());
    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }
    ctx.push(diagnostic);
}

fn emit_generic_arity(
    name: &str,
    arity: usize,
    name_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
) {
    let mut diagnostic = Diagnostic::ts2314(name, arity, ctx.file_name.clone());
    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }
    ctx.push(diagnostic);
}

fn emit_type_alias_cycle(name: &str, name_span: Option<TextSpan>, ctx: &mut CheckerContext) {
    let mut diagnostic = Diagnostic::typescript_rust_type_alias_cycle(name, ctx.file_name.clone());

    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
}

fn emit_type_declaration_cycle(name: &str, name_span: Option<TextSpan>, ctx: &mut CheckerContext) {
    let mut diagnostic =
        Diagnostic::typescript_rust_type_declaration_cycle(name, ctx.file_name.clone());

    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
}

fn with_file_name<R>(
    ctx: &mut CheckerContext,
    file_name: &str,
    f: impl FnOnce(&mut CheckerContext) -> R,
) -> R {
    let current_file_name = ctx.file_name.clone();
    ctx.set_file_name(file_name.to_string());
    let result = f(ctx);
    ctx.set_file_name(current_file_name);
    result
}

fn with_type_declarations<R>(
    type_declarations: &Option<Arc<crate::symbols::TypeDeclarationTable>>,
    ctx: &mut CheckerContext,
    f: impl FnOnce(&mut CheckerContext) -> R,
) -> R {
    let saved_type_declarations = ctx.type_declarations.clone();

    if let Some(type_declarations) = type_declarations {
        ctx.type_declarations = (**type_declarations).clone();
    }

    let result = f(ctx);
    ctx.type_declarations = saved_type_declarations;
    result
}
