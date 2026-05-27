use std::collections::{BTreeMap, HashSet};
use std::path::Path;
use std::sync::Arc;

use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{
    ParsedFunctionType, ParsedFunctionTypeParameter, ParsedInterfaceMember, ParsedMappedType,
    ParsedNamedType, ParsedObjectType, ParsedType, ParsedTypeParameter, TextSpan,
};
use typescript_rust_types::{
    NumberLiteralType, ObjectProperty, Type, TypeCopyReason, union_type, with_type_copy_reason,
};

use crate::arena::{alloc_function_type, alloc_object_type};
use crate::context::{
    CheckerContext, DeclarationNamespace, DeclarationResolutionKey, DeclarationResolutionState,
    convert_span,
};
use crate::default_lib::is_generated_default_lib_file_name;
use crate::paths::canonicalize_if_exists_string;
use crate::program::{
    record_generic_indexed_access_attempt, record_generic_indexed_access_invalid_key,
    record_generic_indexed_access_substituted_key,
    record_generic_indexed_access_substituted_receiver, record_generic_indexed_access_success,
    record_generic_indexed_access_unknown_fallback,
};
use crate::symbols::{InterfaceInfo, TypeAliasInfo, TypeDeclarationInfo};

#[derive(Debug, Clone, Default)]
pub(crate) struct TypeParameterSubstitution {
    values: BTreeMap<String, Type>,
    placeholders: HashSet<String>,
}

impl TypeParameterSubstitution {
    pub(crate) fn clone_with_reason(&self, reason: TypeCopyReason) -> Self {
        with_type_copy_reason(reason, || self.clone())
    }
}

impl TypeParameterSubstitution {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn set(&mut self, name: String, ty: Type, placeholder: bool) {
        self.values.insert(name.clone(), ty);
        if placeholder {
            self.placeholders.insert(name);
        } else {
            self.placeholders.remove(&name);
        }
    }

    pub(crate) fn insert(&mut self, name: String, ty: Type) {
        self.set(name, ty, false);
    }

    pub(crate) fn insert_placeholder(&mut self, name: String, ty: Type) {
        self.set(name, ty, true);
    }

    pub(crate) fn get(&self, name: &str) -> Option<&Type> {
        self.values.get(name)
    }

    pub(crate) fn is_placeholder(&self, name: &str) -> bool {
        self.placeholders.contains(name)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (&String, &Type)> {
        self.values.iter()
    }

    pub(crate) fn extend(&mut self, other: Self) {
        let Self {
            values,
            placeholders,
        } = other;

        for (name, ty) in values {
            if placeholders.contains(&name) {
                self.insert_placeholder(name, ty);
            } else {
                self.insert(name, ty);
            }
        }
    }
}

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
    with_type_copy_reason(TypeCopyReason::SubstitutionChanged, || {
        map_parsed_type_with_substitution(parsed_type, ctx, &TypeParameterSubstitution::new())
    })
}

pub(crate) fn map_parsed_type_with_substitution(
    parsed_type: ParsedType,
    ctx: &mut CheckerContext,
    substitution: &TypeParameterSubstitution,
) -> Type {
    let mut resolving = Vec::new();
    with_type_copy_reason(TypeCopyReason::SubstitutionChanged, || {
        resolve_parsed_type(
            parsed_type,
            ctx,
            &mut resolving,
            &merged_type_parameter_substitution(ctx, substitution),
        )
        .ty
    })
}

fn merged_type_parameter_substitution(
    ctx: &CheckerContext,
    substitution: &TypeParameterSubstitution,
) -> TypeParameterSubstitution {
    let mut merged = TypeParameterSubstitution::new();

    for scope in &ctx.type_parameter_scopes {
        for (name, ty) in scope {
            merged.insert_placeholder(name.clone(), ty.clone());
        }
    }

    merged.extend(substitution.clone_with_reason(TypeCopyReason::SubstitutionUnchanged));

    merged
}

pub(crate) fn validate_local_type_declaration(
    declaration: &TypeDeclarationInfo,
    ctx: &mut CheckerContext,
) {
    match declaration {
        TypeDeclarationInfo::Alias(alias) => {
            let mut substitution = TypeParameterSubstitution::new();
            for type_parameter in &alias.type_parameters {
                substitution.insert_placeholder(type_parameter.name.clone(), Type::Unknown);
            }

            let mut resolving = Vec::new();
            with_type_declaration_scope(&alias.resolution_scope, ctx, |ctx| {
                with_file_name(ctx, &alias.file_name, |ctx| {
                    resolve_parsed_type_with_substitution(
                        alias.ty.clone(),
                        ctx,
                        &mut resolving,
                        &substitution,
                    )
                })
            });
        }
        TypeDeclarationInfo::Interface(interface) => {
            let mut substitution = TypeParameterSubstitution::new();
            for type_parameter in &interface.type_parameters {
                substitution.insert_placeholder(type_parameter.name.clone(), Type::Unknown);
            }

            let mut resolving = Vec::new();
            with_type_declaration_scope(&interface.resolution_scope, ctx, |ctx| {
                with_file_name(ctx, &interface.file_name, |ctx| {
                    resolve_interface_declaration(
                        &interface.extends,
                        &interface.members,
                        ctx,
                        &mut resolving,
                        &substitution,
                    )
                })
            });
        }
    }
}

fn resolve_parsed_type(
    parsed_type: ParsedType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
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
            ResolvedType {
                ty: Type::Array(Box::new(resolved_element.ty)),
                had_error: resolved_element.had_error,
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
            record_generic_indexed_access_attempt();
            let object_type_for_placeholder = indexed_access.object_type.clone();
            let object_placeholder_name =
                parsed_type_placeholder_name(object_type_for_placeholder.as_ref(), substitution);
            let index_placeholder_name =
                parsed_type_placeholder_name(indexed_access.index_type.as_ref(), substitution);
            let object_is_concrete_substitution = is_concrete_substituted_named_reference(
                object_type_for_placeholder.as_ref(),
                substitution,
            );
            let index_is_concrete_substitution = is_concrete_substituted_index_reference(
                indexed_access.index_type.as_ref(),
                substitution,
            );
            let generic_indexed_access = object_placeholder_name.is_some()
                || index_placeholder_name.is_some()
                || object_is_concrete_substitution
                || index_is_concrete_substitution;
            let index_is_keyof_same_placeholder = matches!(
                (
                    object_placeholder_name.as_deref(),
                    indexed_access.index_type.as_ref()
                ),
                (
                    Some(object_name),
                    ParsedType::KeyOf(inner)
                ) if matches!(
                    inner.as_ref(),
                    ParsedType::Named(named_type) if named_type.name == object_name
                )
            );

            if object_is_concrete_substitution {
                record_generic_indexed_access_substituted_receiver();
            }
            if index_is_concrete_substitution {
                record_generic_indexed_access_substituted_key();
            }

            let resolved_object =
                resolve_parsed_type(*indexed_access.object_type, ctx, resolving, substitution);
            if resolved_object.had_error {
                if generic_indexed_access {
                    record_generic_indexed_access_unknown_fallback();
                }
                return resolved_object;
            }

            let resolved_index = resolve_parsed_type(
                *indexed_access.index_type.clone(),
                ctx,
                resolving,
                substitution,
            );

            if object_placeholder_name.is_some() && index_is_keyof_same_placeholder {
                if generic_indexed_access {
                    record_generic_indexed_access_unknown_fallback();
                }
                return ResolvedType {
                    ty: Type::Unknown,
                    had_error: false,
                };
            }

            if index_placeholder_name.is_some()
                || object_placeholder_name.is_some() && !index_is_keyof_same_placeholder
            {
                let index_name = index_placeholder_name
                    .map(str::to_string)
                    .unwrap_or_else(|| resolved_index.ty.name());
                let object_name = object_placeholder_name
                    .map(str::to_string)
                    .unwrap_or_else(|| resolved_object.ty.name());
                let mut diagnostic =
                    Diagnostic::ts2536(&index_name, &object_name, ctx.file_name.clone());
                if let Some(span) = indexed_access
                    .index_type
                    .as_ref()
                    .span()
                    .or(indexed_access.span)
                {
                    diagnostic = diagnostic.with_span(convert_span(span));
                }
                ctx.push(diagnostic);
                if generic_indexed_access {
                    record_generic_indexed_access_invalid_key();
                }
                return ResolvedType {
                    ty: Type::Unknown,
                    had_error: false,
                };
            }

            match (&resolved_object.ty, &resolved_index.ty) {
                (Type::Object(object_type), Type::StringLiteral(key)) => {
                    if let Some(property_ty) = object_type.get_property_access_type(&key) {
                        if generic_indexed_access {
                            record_generic_indexed_access_success();
                        }
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
                    for key_ty in union_ty.types() {
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
                        if generic_indexed_access {
                            record_generic_indexed_access_success();
                        }
                        ResolvedType {
                            ty: union_type(types),
                            had_error: false,
                        }
                    }
                }
                (Type::Tuple(elements), Type::NumberLiteral(num)) => {
                    if let Ok(index) = num.value.parse::<usize>() {
                        if let Some(element_ty) = elements.get(index) {
                            if generic_indexed_access {
                                record_generic_indexed_access_success();
                            }
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
                    ty: {
                        if generic_indexed_access {
                            record_generic_indexed_access_success();
                        }
                        *element_type.clone()
                    },
                    had_error: false,
                },
                (Type::Tuple(elements), Type::Number) => {
                    if generic_indexed_access {
                        record_generic_indexed_access_success();
                    }
                    ResolvedType {
                        ty: union_type(elements.clone()),
                        had_error: false,
                    }
                }
                (Type::Any, _) | (_, Type::Any) => {
                    if generic_indexed_access {
                        record_generic_indexed_access_success();
                    }
                    ResolvedType {
                        ty: Type::Any,
                        had_error: false,
                    }
                }
                (_, Type::StringLiteral(key)) => {
                    let mut diagnostic =
                        Diagnostic::ts2339(key, &resolved_object.ty.name(), ctx.file_name.clone());
                    if let Some(span) = indexed_access.span {
                        diagnostic = diagnostic.with_span(convert_span(span));
                    }
                    ctx.push(diagnostic);
                    if generic_indexed_access {
                        record_generic_indexed_access_unknown_fallback();
                    }
                    ResolvedType {
                        ty: Type::Unknown,
                        had_error: true,
                    }
                }
                (_, invalid_index) => {
                    if let Type::Unknown = invalid_index {
                        if ctx.options.diagnostic_profile
                            != crate::context::DiagnosticProfile::Native
                        {
                            let mut diagnostic =
                                Diagnostic::ts2538(&invalid_index.name(), ctx.file_name.clone());
                            if let Some(span) = indexed_access.span {
                                diagnostic = diagnostic.with_span(convert_span(span));
                            }
                            ctx.push(diagnostic);
                        }
                        if generic_indexed_access {
                            record_generic_indexed_access_unknown_fallback();
                        }
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
                    if generic_indexed_access {
                        record_generic_indexed_access_unknown_fallback();
                    }
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
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let mut resolved_elements = Vec::new();
    let mut had_error = false;

    for element in elements {
        let resolved_element = resolve_parsed_type(element, ctx, resolving, substitution);
        had_error |= resolved_element.had_error;
        resolved_elements.push(resolved_element.ty);
    }

    ResolvedType {
        ty: Type::Tuple(resolved_elements),
        had_error,
    }
}

fn resolve_function_type(
    function_type: ParsedFunctionType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let local_substitution = extend_substitution_with_type_parameters(
        substitution,
        &function_type.type_parameters,
        ctx,
        resolving,
    );

    let required_parameter_count = required_parameter_count(&function_type.parameters);
    let mut parameters = Vec::new();
    let mut had_error = false;

    for parameter in function_type.parameters.iter().cloned() {
        let resolved_parameter =
            resolve_function_type_parameter(parameter, ctx, resolving, &local_substitution);
        had_error |= resolved_parameter.had_error;
        parameters.push(resolved_parameter.ty);
    }

    let return_type = resolve_parsed_type(
        *function_type.return_type,
        ctx,
        resolving,
        &local_substitution,
    );
    had_error |= return_type.had_error;
    ResolvedType {
        ty: Type::Function(alloc_function_type(
            parameters,
            return_type.ty,
            false,
            required_parameter_count,
        )),
        had_error,
    }
}

fn required_parameter_count(
    parameters: &[typescript_rust_syntax::ParsedFunctionTypeParameter],
) -> usize {
    let mut required = parameters.len();

    while required > 0 {
        let parameter = &parameters[required - 1];
        if parameter.optional {
            required -= 1;
        } else {
            break;
        }
    }

    required
}

fn resolve_function_type_parameter(
    parameter: ParsedFunctionTypeParameter,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let ParsedFunctionTypeParameter { ty, .. } = parameter;
    let resolved = resolve_parsed_type(ty, ctx, resolving, substitution);
    ResolvedType {
        ty: resolved.ty,
        had_error: resolved.had_error,
    }
}

fn resolve_object_type(
    object_type: ParsedObjectType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let mut properties = BTreeMap::new();
    let mut had_error = false;

    for property in object_type.properties {
        let property_type = resolve_parsed_type(property.ty, ctx, resolving, substitution);
        had_error |= property_type.had_error;
        let object_property = if property.optional {
            ObjectProperty::optional(property_type.ty)
        } else {
            ObjectProperty::required(property_type.ty)
        };

        properties.insert(property.name, object_property);
    }

    ResolvedType {
        ty: Type::Object(alloc_object_type(properties, None)),
        had_error,
    }
}

fn resolve_union_type(
    types: Vec<ParsedType>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let mut resolved_types = Vec::new();
    let mut had_error = false;

    for ty in types {
        let resolved = resolve_parsed_type(ty, ctx, resolving, substitution);
        had_error |= resolved.had_error;
        resolved_types.push(resolved.ty);
    }

    if resolved_types.is_empty() {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    }

    ResolvedType {
        ty: union_type(resolved_types),
        had_error,
    }
}

fn resolve_named_type(
    named_type: ParsedNamedType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    if let Some(ty) = substitution.get(&named_type.name) {
        return ResolvedType {
            ty: ty.clone(),
            had_error: false,
        };
    }

    let declaration = ctx.lookup_type_declaration(&named_type.name).cloned();

    let Some(declaration) = declaration else {
        emit_unknown_type_name(&named_type, ctx);
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    };

    let has_type_arguments = !named_type.type_arguments.is_empty();
    let is_generic_declaration = match &declaration {
        TypeDeclarationInfo::Alias(alias) => !alias.type_parameters.is_empty(),
        TypeDeclarationInfo::Interface(interface) => !interface.type_parameters.is_empty(),
    };

    if has_type_arguments && !is_generic_declaration {
        match declaration {
            TypeDeclarationInfo::Alias(alias) => {
                emit_type_is_not_generic(&alias.name, alias.name_span, ctx);
            }
            TypeDeclarationInfo::Interface(interface) => {
                emit_type_is_not_generic(&interface.name, interface.name_span, ctx);
            }
        }
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    }

    if !has_type_arguments && !is_generic_declaration {
        let cache_key = type_declaration_resolution_key(&declaration);
        if let Some(cached) = get_cached_named_type_resolution(ctx, &cache_key, resolving) {
            return cached;
        }

        mark_named_type_resolution_in_progress(ctx, &cache_key);
        let resolved = match declaration {
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
        };
        cache_named_type_resolution(ctx, &cache_key, &resolved);
        return resolved;
    }

    let resolved = match declaration {
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
    };
    resolved
}

fn type_declaration_resolution_key(declaration: &TypeDeclarationInfo) -> DeclarationResolutionKey {
    match declaration {
        TypeDeclarationInfo::Alias(alias) => DeclarationResolutionKey {
            file_name: canonical_declaration_file_name(&alias.file_name),
            name: alias.name.clone(),
            namespace: DeclarationNamespace::Type,
        },
        TypeDeclarationInfo::Interface(interface) => DeclarationResolutionKey {
            file_name: canonical_declaration_file_name(&interface.file_name),
            name: interface.name.clone(),
            namespace: DeclarationNamespace::Type,
        },
    }
}

fn declaration_resolution_key(file_name: &str, name: &str) -> DeclarationResolutionKey {
    DeclarationResolutionKey {
        file_name: canonical_declaration_file_name(file_name),
        name: name.to_string(),
        namespace: DeclarationNamespace::Type,
    }
}

fn get_cached_named_type_resolution(
    ctx: &CheckerContext,
    key: &DeclarationResolutionKey,
    resolving: &[DeclarationResolutionKey],
) -> Option<ResolvedType> {
    let cache = ctx.resolved_named_types.lock().ok()?;

    match cache.get(key) {
        Some(DeclarationResolutionState::Resolved { ty, had_error }) => Some(ResolvedType {
            ty: ty.clone(),
            had_error: *had_error,
        }),
        Some(DeclarationResolutionState::Resolving) => {
            if resolving.iter().any(|current| current == key) {
                None
            } else {
                Some(ResolvedType {
                    ty: Type::Unknown,
                    had_error: true,
                })
            }
        }
        None => None,
    }
}

fn mark_named_type_resolution_in_progress(ctx: &CheckerContext, key: &DeclarationResolutionKey) {
    if let Ok(mut cache) = ctx.resolved_named_types.lock() {
        cache.insert(key.clone(), DeclarationResolutionState::Resolving);
    }
}

fn cache_named_type_resolution(
    ctx: &CheckerContext,
    key: &DeclarationResolutionKey,
    resolved: &ResolvedType,
) {
    if let Ok(mut cache) = ctx.resolved_named_types.lock() {
        cache.insert(
            key.clone(),
            DeclarationResolutionState::Resolved {
                ty: resolved.ty.clone(),
                had_error: resolved.had_error,
            },
        );
    }
}

fn canonical_declaration_file_name(file_name: &str) -> String {
    canonicalize_if_exists_string(Path::new(file_name))
}

fn resolve_type_alias(
    alias: TypeAliasInfo,
    type_arguments: Vec<ParsedType>,
    reference_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let declaration_key = declaration_resolution_key(&alias.file_name, &alias.name);
    if resolving.iter().any(|name| name == &declaration_key) {
        emit_type_alias_cycle(&alias.name, alias.name_span, ctx);
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    }

    resolving.push(declaration_key.clone());
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

    if alias.file_name == "<built-in>" || is_generated_default_lib_file_name(&alias.file_name) {
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

    let resolved = with_type_declaration_scope(&alias.resolution_scope, ctx, |ctx| {
        with_file_name(ctx, &alias.file_name, |ctx| {
            resolve_parsed_type_with_substitution(alias.ty, ctx, resolving, &local_substitution)
        })
    });
    resolving.pop();

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
        "Parameters" => Some(resolve_parameters_utility_type(substitution)),
        "ReturnType" => Some(resolve_return_type_utility_type(substitution)),
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
    for (name, property) in object_type.properties.iter() {
        properties.insert(name.clone(), ObjectProperty::optional(property.ty.clone()));
    }

    ResolvedType {
        ty: Type::Object(alloc_object_type(properties, None)),
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

    if key_type == Type::String {
        return ResolvedType {
            ty: Type::Object(alloc_object_type(
                BTreeMap::new(),
                Some(substitution.get("T").cloned().unwrap_or(Type::Unknown)),
            )),
            had_error: false,
        };
    }

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
        ty: Type::Object(alloc_object_type(properties, None)),
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
        ty: Type::Object(alloc_object_type(properties, None)),
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
    for (key, property) in object_type.properties.iter() {
        if keys.iter().any(|candidate| candidate == key) {
            continue;
        }

        properties.insert(key.clone(), property.clone());
    }

    ResolvedType {
        ty: Type::Object(alloc_object_type(properties, None)),
        had_error: false,
    }
}

fn resolve_parameters_utility_type(substitution: &TypeParameterSubstitution) -> ResolvedType {
    let Some(source_type) = substitution.get("T").cloned() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let Type::Function(function_type) = source_type else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    ResolvedType {
        ty: Type::Tuple(function_type.parameters().to_vec()),
        had_error: false,
    }
}

fn resolve_return_type_utility_type(substitution: &TypeParameterSubstitution) -> ResolvedType {
    let Some(source_type) = substitution.get("T").cloned() else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    let Type::Function(function_type) = source_type else {
        return ResolvedType {
            ty: Type::Unknown,
            had_error: false,
        };
    };

    ResolvedType {
        ty: function_type.return_type().clone(),
        had_error: false,
    }
}

fn string_literal_union_keys(ty: &Type) -> Option<Vec<String>> {
    match ty {
        Type::StringLiteral(value) => Some(vec![value.clone()]),
        Type::Union(union) => {
            let mut keys = Vec::new();
            for variant in union.types() {
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
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let declaration_key = declaration_resolution_key(&interface.file_name, &interface.name);
    if resolving.iter().any(|name| name == &declaration_key) {
        emit_type_declaration_cycle(&interface.name, interface.name_span, ctx);
        return ResolvedType {
            ty: Type::Unknown,
            had_error: true,
        };
    }

    resolving.push(declaration_key.clone());
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

    if is_generated_default_lib_file_name(&interface.file_name) {
        match interface.name.as_str() {
            "Array" | "ReadonlyArray" => {
                let element_type = local_substitution.get("T").cloned().unwrap_or(Type::Any);
                resolving.pop();
                return ResolvedType {
                    ty: Type::Array(Box::new(element_type)),
                    had_error: false,
                };
            }
            "Uint8Array" => {
                resolving.pop();
                return ResolvedType {
                    ty: Type::Array(Box::new(Type::Number)),
                    had_error: false,
                };
            }
            "Map" => {
                resolving.pop();
                return ResolvedType {
                    ty: generated_default_lib_map_instance_type(),
                    had_error: false,
                };
            }
            "Promise" | "PromiseLike" => {
                let ty = local_substitution
                    .get("T")
                    .cloned()
                    .unwrap_or(Type::Unknown);
                resolving.pop();
                return ResolvedType {
                    ty,
                    had_error: false,
                };
            }
            _ => {}
        }
    }

    let resolved = with_type_declaration_scope(&interface.resolution_scope, ctx, |ctx| {
        with_file_name(ctx, &interface.file_name, |ctx| {
            resolve_interface_declaration(
                &interface.extends,
                &interface.members,
                ctx,
                resolving,
                &local_substitution,
            )
        })
    });
    resolving.pop();

    resolved
}

fn resolve_interface_declaration(
    extends: &[ParsedNamedType],
    members: &[ParsedInterfaceMember],
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    let mut properties = BTreeMap::new();
    let mut had_error = false;

    for base in extends {
        let resolved_base = resolve_named_type(base.clone(), ctx, resolving, substitution);
        if resolved_base.ty == Type::Unknown {
            had_error |= resolved_base.had_error;
            continue;
        }

        had_error |= resolved_base.had_error;

        match resolved_base.ty {
            Type::Object(object_type) => {
                for (name, property) in object_type.properties.iter() {
                    properties.entry(name.clone()).or_insert(property.clone());
                }
            }
            Type::Any => {}
            _ => {}
        }
    }

    for member in members {
        let property_type = resolve_parsed_type(member.ty.clone(), ctx, resolving, substitution);
        had_error |= property_type.had_error;
        let object_property = if member.optional {
            ObjectProperty::optional(property_type.ty)
        } else {
            ObjectProperty::required(property_type.ty)
        };

        properties.insert(member.name.clone(), object_property);
    }

    ResolvedType {
        ty: Type::Object(alloc_object_type(properties, None)),
        had_error,
    }
}

fn generated_default_lib_map_instance_type() -> Type {
    let mut properties = BTreeMap::new();
    properties.insert(
        "get".to_string(),
        ObjectProperty::required(Type::Function(alloc_function_type(
            vec![Type::Any],
            Type::Any,
            false,
            1,
        ))),
    );
    properties.insert(
        "set".to_string(),
        ObjectProperty::required(Type::Function(alloc_function_type(
            vec![Type::Any, Type::Any],
            Type::Any,
            false,
            2,
        ))),
    );
    properties.insert(
        "has".to_string(),
        ObjectProperty::required(Type::Function(alloc_function_type(
            vec![Type::Any],
            Type::Boolean,
            false,
            1,
        ))),
    );
    properties.insert(
        "delete".to_string(),
        ObjectProperty::required(Type::Function(alloc_function_type(
            vec![Type::Any],
            Type::Boolean,
            false,
            1,
        ))),
    );
    properties.insert(
        "clear".to_string(),
        ObjectProperty::required(Type::Function(alloc_function_type(
            vec![],
            Type::Void,
            false,
            0,
        ))),
    );
    properties.insert("size".to_string(), ObjectProperty::required(Type::Number));

    Type::Object(alloc_object_type(properties, None))
}

fn bind_type_arguments(
    type_parameters: &[ParsedTypeParameter],
    type_arguments: Vec<ParsedType>,
    name: &str,
    name_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
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

            if parsed_type_is_placeholder_reference(argument, parent_substitution) {
                substitution.insert_placeholder(parameter.name.clone(), resolved_argument.ty);
            } else {
                substitution.insert(parameter.name.clone(), resolved_argument.ty);
            }
            continue;
        }

        let Some(default_type) = parameter.default_type.clone() else {
            emit_generic_arity(name, type_parameters.len(), name_span, ctx);
            return None;
        };

        let mut effective_substitution =
            parent_substitution.clone_with_reason(TypeCopyReason::SubstitutionChanged);
        effective_substitution
            .extend(substitution.clone_with_reason(TypeCopyReason::SubstitutionChanged));

        let default_type_is_placeholder =
            parsed_type_is_placeholder_reference(&default_type, &effective_substitution);
        let resolved_default =
            resolve_parsed_type(default_type, ctx, resolving, &effective_substitution);
        if resolved_default.had_error {
            return None;
        }

        if default_type_is_placeholder {
            substitution.insert_placeholder(parameter.name.clone(), resolved_default.ty);
        } else {
            substitution.insert(parameter.name.clone(), resolved_default.ty);
        }
    }

    Some(substitution)
}

fn extend_substitution_with_type_parameters(
    parent_substitution: &TypeParameterSubstitution,
    type_parameters: &[ParsedTypeParameter],
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
) -> TypeParameterSubstitution {
    let mut substitution =
        parent_substitution.clone_with_reason(TypeCopyReason::SubstitutionChanged);

    for parameter in type_parameters {
        let mut effective_substitution =
            parent_substitution.clone_with_reason(TypeCopyReason::SubstitutionChanged);
        effective_substitution
            .extend(substitution.clone_with_reason(TypeCopyReason::SubstitutionChanged));

        let resolved = parameter.default_type.clone().map(|default_type| {
            resolve_parsed_type(default_type, ctx, resolving, &effective_substitution)
        });

        let ty = match resolved {
            Some(resolved) if !resolved.had_error => resolved.ty,
            Some(_) => Type::Unknown,
            None => Type::Unknown,
        };

        if let Some(default_type) = parameter.default_type.as_ref() {
            if parsed_type_is_placeholder_reference(default_type, &effective_substitution) {
                substitution.insert_placeholder(parameter.name.clone(), ty);
                continue;
            }
        }

        substitution.insert(parameter.name.clone(), ty);
    }

    substitution
}

fn resolve_mapped_type(
    mapped: ParsedMappedType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
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
            for variant in union.types() {
                match variant {
                    Type::StringLiteral(s) => keys.push(s.clone()),
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
        let mut new_substitution =
            substitution.clone_with_reason(TypeCopyReason::SubstitutionChanged);
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
        ty: Type::Object(alloc_object_type(properties, None)),
        had_error,
    }
}

fn resolve_parsed_type_with_substitution(
    parsed_type: ParsedType,
    ctx: &mut CheckerContext,
    resolving: &mut Vec<DeclarationResolutionKey>,
    substitution: &TypeParameterSubstitution,
) -> ResolvedType {
    with_type_copy_reason(TypeCopyReason::SubstitutionChanged, || {
        resolve_parsed_type(parsed_type, ctx, resolving, substitution)
    })
}

fn parsed_type_is_placeholder_reference(
    parsed_type: &ParsedType,
    substitution: &TypeParameterSubstitution,
) -> bool {
    matches!(
        parsed_type,
        ParsedType::Named(named_type) if substitution.is_placeholder(&named_type.name)
    )
}

fn parsed_type_placeholder_name<'a>(
    parsed_type: &'a ParsedType,
    substitution: &TypeParameterSubstitution,
) -> Option<&'a str> {
    match parsed_type {
        ParsedType::Named(named_type) if substitution.is_placeholder(&named_type.name) => {
            Some(named_type.name.as_str())
        }
        _ => None,
    }
}

trait ParsedTypeSpan {
    fn span(&self) -> Option<TextSpan>;
}

impl ParsedTypeSpan for ParsedType {
    fn span(&self) -> Option<TextSpan> {
        match self {
            ParsedType::Named(named_type) => named_type.span,
            ParsedType::TypeOf(type_of) => type_of.name_span,
            ParsedType::IndexedAccess(indexed_access) => indexed_access.span,
            ParsedType::Mapped(mapped) => mapped.key_span.or(mapped.span),
            _ => None,
        }
    }
}

fn is_concrete_substituted_named_reference(
    parsed_type: &ParsedType,
    substitution: &TypeParameterSubstitution,
) -> bool {
    matches!(
        parsed_type,
        ParsedType::Named(named_type)
            if substitution
                .get(&named_type.name)
                .is_some()
                && !substitution.is_placeholder(&named_type.name)
    )
}

fn is_concrete_substituted_index_reference(
    parsed_type: &ParsedType,
    substitution: &TypeParameterSubstitution,
) -> bool {
    match parsed_type {
        ParsedType::Named(named_type) => {
            substitution.get(&named_type.name).is_some()
                && !substitution.is_placeholder(&named_type.name)
        }
        ParsedType::KeyOf(inner) => {
            is_concrete_substituted_named_reference(inner.as_ref(), substitution)
        }
        _ => false,
    }
}

fn emit_unknown_type_name(named_type: &ParsedNamedType, ctx: &mut CheckerContext) {
    let diagnostic =
        if named_type.name == "Buffer" && !ctx.options.types.iter().any(|ty| ty == "node") {
            Diagnostic::ts2591(&named_type.name, ctx.file_name.clone())
        } else {
            Diagnostic::ts2304(&named_type.name, ctx.file_name.clone())
        };
    let mut diagnostic = diagnostic;
    if let Some(span) = named_type.span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }
    ctx.push_utility_diagnostic_once(diagnostic);
}

fn emit_type_is_not_generic(name: &str, name_span: Option<TextSpan>, ctx: &mut CheckerContext) {
    let mut diagnostic = Diagnostic::ts2315(name, ctx.file_name.clone());
    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }
    ctx.push_utility_diagnostic_once(diagnostic);
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
    ctx.push_utility_diagnostic_once(diagnostic);
}

fn emit_type_alias_cycle(name: &str, name_span: Option<TextSpan>, ctx: &mut CheckerContext) {
    let mut diagnostic = Diagnostic::typescript_rust_type_alias_cycle(name, ctx.file_name.clone());

    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push_utility_diagnostic_once(diagnostic);
}

fn emit_type_declaration_cycle(name: &str, name_span: Option<TextSpan>, ctx: &mut CheckerContext) {
    let mut diagnostic =
        Diagnostic::typescript_rust_type_declaration_cycle(name, ctx.file_name.clone());

    if let Some(span) = name_span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push_utility_diagnostic_once(diagnostic);
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

fn with_type_declaration_scope<R>(
    type_declaration_scope: &Option<Arc<crate::symbols::TypeDeclarationScope>>,
    ctx: &mut CheckerContext,
    f: impl FnOnce(&mut CheckerContext) -> R,
) -> R {
    let saved_type_declaration_scope = ctx.type_declaration_scope.clone();

    if let Some(type_declaration_scope) = type_declaration_scope {
        ctx.type_declaration_scope = Some(type_declaration_scope.clone());
    }

    let result = f(ctx);
    ctx.type_declaration_scope = saved_type_declaration_scope;
    result
}
