//! Generic call type-argument inference and function-type instantiation.

use super::*;

use std::borrow::Cow;
use std::collections::HashMap;
use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedCallArgument, ParsedNamedType, ParsedObjectType, ParsedType, TextSpan,
};
use surge_ts_types::{FunctionType, Type, TypeCopyReason, with_type_copy_reason};

use crate::arena::{alloc_function_type, alloc_object_type};
use crate::context::{CheckerContext, convert_span};
use crate::infer::string_literal_union_keys;
use crate::infer::{InferredExpression, infer_expression};
use crate::infer::{TypeParameterSubstitution, map_parsed_type_with_substitution};
use crate::program::{
    record_generic_call_inference_attempt, record_generic_call_inference_candidate,
    record_generic_call_inference_explicit_type_args_skip, record_generic_call_inference_failed,
    record_generic_call_inference_success, record_generic_call_inference_tuple_return_suppressed,
    record_generic_call_inference_unresolved_argument_skip,
};
use crate::symbols::{FunctionSignatureInfo, SymbolTable, TypeDeclarationInfo};

pub(crate) fn instantiate_function_type<'a>(
    function_type: &'a FunctionType,
    function_signature: Option<&FunctionSignatureInfo>,
    type_arguments: &[ParsedType],
    type_argument_span: Option<TextSpan>,
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Cow<'a, FunctionType> {
    let Some(function_signature) = function_signature else {
        return Cow::Borrowed(function_type);
    };

    if function_signature.type_parameters.is_empty() {
        return Cow::Borrowed(function_type);
    }

    record_generic_call_inference_attempt();

    if !type_arguments.is_empty() {
        record_generic_call_inference_explicit_type_args_skip();
        let substitution =
            explicit_type_argument_substitution(function_signature, type_arguments, ctx);

        // A type argument that failed to resolve, or one that violates a
        // `K extends keyof T` constraint, must not cascade into the `T[K]`
        // return type; fall back to the declared (generic) return type instead.
        let has_unresolved_argument = substitution
            .iter()
            .any(|(_, candidate)| type_contains_unknown(candidate));
        let constraint_violation = enforce_explicit_keyof_constraints(
            function_signature,
            type_arguments,
            &substitution,
            type_argument_span,
            ctx,
        );
        if has_unresolved_argument || constraint_violation {
            return Cow::Borrowed(function_type);
        }

        return instantiate_function_type_with_substitution(
            function_type,
            function_signature,
            &substitution,
            false,
            ctx,
        );
    }

    let substitution =
        infer_type_argument_substitution(function_signature, arguments, symbols, ctx);

    if substitution
        .iter()
        .all(|(_, candidate)| candidate.is_unknown())
    {
        record_generic_call_inference_failed();
        return Cow::Borrowed(function_type);
    }

    record_generic_call_inference_success();
    instantiate_function_type_with_substitution(
        function_type,
        function_signature,
        &substitution,
        true,
        ctx,
    )
}

pub(crate) fn instantiate_function_type_with_substitution<'a>(
    function_type: &'a FunctionType,
    function_signature: &FunctionSignatureInfo,
    substitution: &TypeParameterSubstitution,
    suppress_tuple_return_type: bool,
    ctx: &mut CheckerContext,
) -> Cow<'a, FunctionType> {
    with_type_copy_reason(TypeCopyReason::CallResolution, || {
        let mut instantiated_parameters = Vec::with_capacity(function_type.parameters().len());
        for (index, parameter) in function_type.parameters().iter().enumerate() {
            let Some(parsed_parameter) = function_signature
                .parameter_types
                .get(index)
                .and_then(|ty| ty.clone())
            else {
                instantiated_parameters.push(parameter.clone());
                continue;
            };

            instantiated_parameters.push(map_parsed_type_with_substitution(
                parsed_parameter,
                ctx,
                substitution,
            ));
        }

        let mut instantiated_return_type = function_signature
            .return_type
            .as_ref()
            .map(|return_type| {
                map_parsed_type_with_substitution(return_type.clone(), ctx, substitution)
            })
            .unwrap_or_else(|| function_type.return_type().clone());

        if suppress_tuple_return_type && matches!(instantiated_return_type, Type::Tuple(_)) {
            record_generic_call_inference_tuple_return_suppressed();
            instantiated_return_type = Type::Unknown;
        }

        Cow::Owned(alloc_function_type(
            instantiated_parameters,
            instantiated_return_type,
            function_type.is_variadic(),
            function_type.required_parameter_count(),
        ))
    })
}

pub(crate) fn instantiate_function_return_type_for_call(
    function_type: &FunctionType,
    function_signature: Option<&FunctionSignatureInfo>,
    type_arguments: &[ParsedType],
    type_argument_span: Option<TextSpan>,
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Type {
    with_type_copy_reason(TypeCopyReason::CallResolution, || {
        instantiate_function_type(
            function_type,
            function_signature,
            type_arguments,
            type_argument_span,
            arguments,
            symbols,
            ctx,
        )
        .return_type()
        .clone()
    })
}

pub(crate) fn explicit_type_argument_substitution(
    function_signature: &FunctionSignatureInfo,
    type_arguments: &[ParsedType],
    ctx: &mut CheckerContext,
) -> TypeParameterSubstitution {
    let mut substitution = TypeParameterSubstitution::new();
    for (type_parameter, type_argument) in function_signature
        .type_parameters
        .iter()
        .zip(type_arguments.iter())
    {
        substitution.insert(
            type_parameter.name.clone(),
            map_parsed_type_with_substitution(
                type_argument.clone(),
                ctx,
                &TypeParameterSubstitution::new(),
            ),
        );
    }
    substitution
}

/// Validate explicit type arguments against `K extends keyof T` constraints when
/// both `T` (a concrete object) and `K` (concrete string-literal keys) are
/// resolved. Emits TS2344 for keys that are not members of `T` and reports
/// whether any constraint was violated so the caller can avoid a cascading
/// `T[K]` diagnostic. Other constraint forms are intentionally left untouched.
pub(crate) fn enforce_explicit_keyof_constraints(
    function_signature: &FunctionSignatureInfo,
    type_arguments: &[ParsedType],
    substitution: &TypeParameterSubstitution,
    type_argument_span: Option<TextSpan>,
    ctx: &mut CheckerContext,
) -> bool {
    let mut violated = false;

    for type_parameter in &function_signature.type_parameters {
        let Some(ParsedType::KeyOf(inner)) = &type_parameter.constraint else {
            continue;
        };
        let ParsedType::Named(constraint_target) = inner.as_ref() else {
            continue;
        };

        let Some(constraint_value) = substitution.get(&constraint_target.name).cloned() else {
            continue;
        };
        // `T` may be bound to a nominal reference (`get<User, …>`); peel it to read
        // the constrained object's keys.
        let Type::Object(object_type) = constraint_value.peeled() else {
            continue;
        };
        let Some(key_type) = substitution.get(&type_parameter.name).cloned() else {
            continue;
        };
        let Some(keys) = string_literal_union_keys(&key_type) else {
            continue;
        };

        let all_keys_present = keys
            .iter()
            .all(|key| object_type.get_property_access_type(key).is_some());
        if all_keys_present {
            continue;
        }

        violated = true;
        let constraint_name = format!(
            "keyof {}",
            constraint_target_display(
                function_signature,
                type_arguments,
                &constraint_target.name,
                &object_type
            )
        );
        let mut diagnostic =
            Diagnostic::ts2344(&key_type.name(), &constraint_name, ctx.file_name.clone());
        if let Some(span) = type_argument_span {
            diagnostic = diagnostic.with_span(convert_span(span));
        }
        ctx.push_utility_diagnostic_once(diagnostic);
    }

    violated
}

fn constraint_target_display(
    function_signature: &FunctionSignatureInfo,
    type_arguments: &[ParsedType],
    target_name: &str,
    resolved_object: &surge_ts_types::ObjectType,
) -> String {
    let target_argument = function_signature
        .type_parameters
        .iter()
        .position(|type_parameter| type_parameter.name == target_name)
        .and_then(|index| type_arguments.get(index));

    if let Some(ParsedType::Named(named)) = target_argument {
        return named.name.clone();
    }

    Type::Object(resolved_object.clone()).name()
}

pub(crate) fn infer_type_argument_substitution(
    function_signature: &FunctionSignatureInfo,
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> TypeParameterSubstitution {
    let mut substitution = TypeParameterSubstitution::new();
    for type_parameter in &function_signature.type_parameters {
        substitution.insert_placeholder(type_parameter.name.clone(), Type::Unknown);
    }

    for (index, argument) in arguments.iter().enumerate() {
        let Some(parameter_type) = function_signature
            .parameter_types
            .get(index)
            .and_then(|ty| ty.as_ref())
        else {
            continue;
        };

        // This is an inference *probe*: the argument is evaluated only to infer the
        // call's type parameters, without the contextual parameter type the
        // authoritative `check_function_type_call` pass supplies afterwards. An
        // object-literal argument with a method (`{ run(ctx, input) {} }`) would
        // here type those parameters as implicit `any` and emit a spurious TS7006,
        // even though the instantiated parameter type gives them real contextual
        // types in the authoritative pass. Discard any diagnostics this probe
        // emits; the authoritative pass re-evaluates every argument and reports the
        // genuine ones.
        let diagnostics_before = ctx.diagnostics().len();
        let inferred_argument = infer_expression(&argument.expression, symbols, ctx);
        ctx.truncate_diagnostics(diagnostics_before);
        let InferredExpression::Known(argument_type) = inferred_argument else {
            record_generic_call_inference_unresolved_argument_skip();
            continue;
        };

        if argument_type.is_unknown() || type_contains_unknown(&argument_type) {
            record_generic_call_inference_unresolved_argument_skip();
            continue;
        }

        collect_inferred_type_argument(
            parameter_type,
            &argument_type,
            &mut substitution,
            false,
            ctx,
            0,
        );
    }

    substitution
}

pub(crate) fn collect_inferred_type_argument(
    parameter_type: &ParsedType,
    argument_type: &Type,
    substitution: &mut TypeParameterSubstitution,
    widen_literals: bool,
    ctx: &mut CheckerContext,
    depth: usize,
) {
    if type_contains_unknown(argument_type) {
        return;
    }

    match parameter_type {
        ParsedType::Named(named_type) => {
            record_type_argument_candidate(
                substitution,
                &named_type.name,
                argument_type,
                widen_literals,
            );
            // A generic-instantiation parameter (`Wrapper<T>`,
            // `SignalDefinition<TPayload>`) carries its type parameters inside the
            // declaration's members, not at the surface — match the argument
            // against that shape so `T` is inferred from the nested field.
            if !named_type.type_arguments.is_empty() {
                infer_through_generic_reference(
                    named_type,
                    argument_type,
                    substitution,
                    ctx,
                    depth,
                );
            }
        }
        ParsedType::Array(element_type) => match argument_type {
            Type::Array(actual_element_type) => {
                collect_inferred_type_argument(
                    element_type.as_ref(),
                    actual_element_type.as_ref(),
                    substitution,
                    true,
                    ctx,
                    depth,
                );
            }
            Type::Tuple(elements) => {
                for element in elements {
                    collect_inferred_type_argument(
                        element_type.as_ref(),
                        element,
                        substitution,
                        true,
                        ctx,
                        depth,
                    );
                }
            }
            _ => {}
        },
        ParsedType::Tuple(expected_elements) => {
            if let Type::Tuple(actual_elements) = argument_type
                && expected_elements.len() == actual_elements.len()
            {
                for (expected_element, actual_element) in
                    expected_elements.iter().zip(actual_elements.iter())
                {
                    collect_inferred_type_argument(
                        expected_element,
                        actual_element,
                        substitution,
                        true,
                        ctx,
                        depth,
                    );
                }
            }
        }
        ParsedType::Object(expected_object_type) => {
            if let Type::Object(actual_object_type) = argument_type {
                collect_object_type_candidates(
                    expected_object_type,
                    actual_object_type,
                    substitution,
                    ctx,
                    depth,
                );
            }
        }
        ParsedType::Union(expected_types) => {
            if let Type::Union(actual_union) = argument_type
                && expected_types.len() == actual_union.types().len()
            {
                for (expected_element, actual_element) in
                    expected_types.iter().zip(actual_union.types().iter())
                {
                    collect_inferred_type_argument(
                        expected_element,
                        actual_element,
                        substitution,
                        widen_literals,
                        ctx,
                        depth,
                    );
                }
            }
        }
        _ => {}
    }
}

/// Infers type parameters that appear *inside* a generic parameter
/// (`def: SignalDefinition<TPayload>`, `w: Wrapper<T>`). When the argument is an
/// instantiation of the same declaration we match type arguments positionally;
/// when it is an object literal we resolve the declaration's members, substitute
/// the declaration's own type parameters with this position's type arguments, and
/// match member-by-member. Bounded by `depth` so mutually-generic declarations
/// cannot loop.
fn infer_through_generic_reference(
    named_type: &ParsedNamedType,
    argument_type: &Type,
    substitution: &mut TypeParameterSubstitution,
    ctx: &mut CheckerContext,
    depth: usize,
) {
    const MAX_DEPTH: usize = 6;
    if depth >= MAX_DEPTH {
        return;
    }

    if let Type::Reference(reference) = argument_type {
        for (pattern_argument, actual_argument) in named_type
            .type_arguments
            .iter()
            .zip(reference.arguments.iter())
        {
            collect_inferred_type_argument(
                pattern_argument,
                actual_argument,
                substitution,
                true,
                ctx,
                depth + 1,
            );
        }
        return;
    }

    let Type::Object(actual_object) = argument_type else {
        return;
    };

    let body = {
        let Some(handle) = ctx.lookup_type_declaration_handle(&named_type.name) else {
            return;
        };
        match handle.get() {
            TypeDeclarationInfo::Interface(info) => info.body.clone(),
            TypeDeclarationInfo::Alias(info) => {
                infer_through_generic_alias(info, named_type, argument_type, substitution, ctx, depth);
                return;
            }
        }
    };
    if body.type_parameters.len() != named_type.type_arguments.len() {
        return;
    }

    let parameter_map: HashMap<String, ParsedType> = body
        .type_parameters
        .iter()
        .zip(named_type.type_arguments.iter())
        .map(|(parameter, argument)| (parameter.name.clone(), argument.clone()))
        .collect();

    for member in &body.members {
        let Some(actual_member_type) = actual_object.get_property_access_type(&member.name) else {
            continue;
        };
        let expected_member_type = substitute_parsed_type_parameters(&member.ty, &parameter_map);
        collect_inferred_type_argument(
            &expected_member_type,
            &actual_member_type,
            substitution,
            true,
            ctx,
            depth + 1,
        );
    }
}

/// Infers through a generic *alias* parameter (`config?: Config<T>`). A plain
/// object body matches like an interface's members. A conditional body whose
/// check type is one of the alias's own parameters (`T extends ConfigSchema ?
/// { variants?: T; … } : never` — the class-variance-authority shape) matches
/// the argument against the substituted TRUE branch: selecting that branch is
/// exactly what a successful inference implies, and its members are where the
/// parameter occurs (tsc infers `T` from `variants` the same way).
fn infer_through_generic_alias(
    alias: &crate::symbols::TypeAliasInfo,
    named_type: &ParsedNamedType,
    argument_type: &Type,
    substitution: &mut TypeParameterSubstitution,
    ctx: &mut CheckerContext,
    depth: usize,
) {
    if alias.body.type_parameters.len() != named_type.type_arguments.len() {
        return;
    }
    let parameter_map: HashMap<String, ParsedType> = alias
        .body
        .type_parameters
        .iter()
        .zip(named_type.type_arguments.iter())
        .map(|(parameter, argument)| (parameter.name.clone(), argument.clone()))
        .collect();

    let mut body = &alias.body.ty;
    if let ParsedType::Conditional(conditional) = body {
        let check_is_own_parameter = matches!(
            conditional.check_type.as_ref(),
            ParsedType::Named(check) if parameter_map.contains_key(&check.name)
        );
        if !check_is_own_parameter {
            return;
        }
        body = conditional.true_type.as_ref();
    }

    let substituted =
        crate::infer::substitute_parsed_type_parameters_deep(body, &parameter_map);
    collect_inferred_type_argument(&substituted, argument_type, substitution, true, ctx, depth + 1);
}

/// Substitutes bare named references in a parsed type using `map`, recursing into
/// generic arguments and array elements. Used to rewrite a declaration's member
/// types from the declaration's own type parameters into the enclosing call's
/// type parameters before inference.
fn substitute_parsed_type_parameters(
    parsed_type: &ParsedType,
    map: &HashMap<String, ParsedType>,
) -> ParsedType {
    match parsed_type {
        ParsedType::Named(named) => {
            if named.type_arguments.is_empty() {
                if let Some(replacement) = map.get(&named.name) {
                    return replacement.clone();
                }
                ParsedType::Named(named.clone())
            } else {
                let mut substituted = named.clone();
                substituted.type_arguments = named
                    .type_arguments
                    .iter()
                    .map(|argument| substitute_parsed_type_parameters(argument, map))
                    .collect();
                ParsedType::Named(substituted)
            }
        }
        ParsedType::Array(element) => {
            ParsedType::Array(Box::new(substitute_parsed_type_parameters(element, map)))
        }
        other => other.clone(),
    }
}

pub(crate) fn collect_object_type_candidates(
    expected_object_type: &ParsedObjectType,
    actual_object_type: &surge_ts_types::ObjectType,
    substitution: &mut TypeParameterSubstitution,
    ctx: &mut CheckerContext,
    depth: usize,
) {
    for property in &expected_object_type.properties {
        let Some(actual_property_type) =
            actual_object_type.get_property_access_type(&property.name)
        else {
            continue;
        };

        collect_inferred_type_argument(
            &property.ty,
            &actual_property_type,
            substitution,
            true,
            ctx,
            depth,
        );
    }
}

pub(crate) fn record_type_argument_candidate(
    substitution: &mut TypeParameterSubstitution,
    type_parameter_name: &str,
    argument_type: &Type,
    widen_literals: bool,
) {
    let Some(existing) = substitution.get(type_parameter_name).cloned() else {
        return;
    };

    record_generic_call_inference_candidate();
    let candidate = if widen_literals {
        widen_candidate_type(argument_type)
    } else {
        with_type_copy_reason(TypeCopyReason::CallResolution, || argument_type.clone())
    };

    if existing.is_unknown() {
        substitution.set(type_parameter_name.to_string(), candidate, false);
        return;
    }

    if existing == candidate {
        return;
    }

    if let Some(common_primitive) = common_primitive_candidate(&existing, &candidate) {
        substitution.set(type_parameter_name.to_string(), common_primitive, false);
    }
}

pub(crate) fn common_primitive_candidate(existing: &Type, candidate: &Type) -> Option<Type> {
    match (existing.base_primitive(), candidate.base_primitive()) {
        (Some(Type::String), Some(Type::String)) => Some(Type::String),
        (Some(Type::Number), Some(Type::Number)) => Some(Type::Number),
        (Some(Type::Boolean), Some(Type::Boolean)) => Some(Type::Boolean),
        _ => None,
    }
}

pub(crate) fn widen_candidate_type(ty: &Type) -> Type {
    match ty {
        Type::StringLiteral(_) => Type::String,
        Type::NumberLiteral(_) => Type::Number,
        Type::BooleanLiteral(_) => Type::Boolean,
        Type::Array(element) => Type::Array(Box::new(widen_candidate_type(element.as_ref()))),
        Type::Tuple(elements) => Type::Tuple(elements.iter().map(widen_candidate_type).collect()),
        Type::Object(object) => {
            let properties = object
                .properties
                .iter()
                .map(|(name, property)| {
                    (
                        name.clone(),
                        surge_ts_types::ObjectProperty {
                            ty: widen_candidate_type(&property.ty),
                            optional: property.optional,
                        },
                    )
                })
                .collect::<surge_ts_types::PropertyMap>();

            Type::Object(alloc_object_type(properties, None))
        }
        Type::Union(union_payload) => surge_ts_types::union_type(
            union_payload
                .types()
                .iter()
                .map(widen_candidate_type)
                .collect(),
        ),
        _ => with_type_copy_reason(TypeCopyReason::CallResolution, || ty.clone()),
    }
}
