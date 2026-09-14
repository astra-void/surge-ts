//! Generic call type-argument inference and function-type instantiation.

use super::*;

use std::borrow::Cow;
use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedCallArgument, ParsedNamedType, ParsedObjectType, ParsedType, TextSpan,
};
use surge_ts_types::{FunctionType, Type, TypeCopyReason, with_type_copy_reason};

use crate::context::{CheckerContext, convert_span};
use crate::infer::string_literal_union_keys;
use crate::infer::{InferredExpression, infer_expression};
use crate::infer::{
    TypeParameterSubstitution, map_parsed_type_with_substitution,
    try_map_parsed_type_with_substitution,
};
use crate::metrics::{alloc_function_type, alloc_object_type};
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
    outer_type_arguments: &[(String, Type)],
    type_arguments: &[ParsedType],
    type_argument_span: Option<TextSpan>,
    arguments: &[ParsedCallArgument],
    expected_return_type: Option<&Type>,
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
        // Explicit type arguments name types visible at the CALL, which may be
        // function locals; `ctx.symbols` is the file-level table and does not
        // hold them. The caller already passed the right one.
        let substitution = {
            let saved_symbols = std::mem::replace(&mut ctx.symbols, symbols.clone());
            let substitution =
                explicit_type_argument_substitution(function_signature, type_arguments, ctx);
            ctx.symbols = saved_symbols;
            substitution
        };

        // The enclosing bindings (`T` of the interface a `<K extends keyof T>`
        // member was read off) are seeded first: the constraint checks below
        // resolve `keyof T` against them.
        let mut substitution = substitution;
        seed_outer_type_arguments(&mut substitution, outer_type_arguments);
        // A type argument that failed to resolve, or one that violates a
        // `K extends keyof T` constraint, must not cascade into the `T[K]`
        // return type; fall back to the declared (generic) return type instead.
        let has_unresolved_argument = substitution
            .iter()
            .filter(|(name, _)| {
                !outer_type_arguments
                    .iter()
                    .any(|(outer, _)| outer == name.as_ref())
            })
            .any(|(_, candidate)| type_argument_is_unresolved(candidate));
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

        let instantiated = instantiate_function_type_with_substitution(
            function_type,
            function_signature,
            &substitution,
            false,
            ctx,
        );
        return fold_overload_alternative_parameters(
            instantiated,
            function_type,
            function_signature,
            outer_type_arguments,
            type_arguments,
            arguments,
            symbols,
            ctx,
        );
    }

    let mut substitution = infer_type_argument_substitution(
        function_signature,
        arguments,
        outer_type_arguments,
        expected_return_type,
        symbols,
        ctx,
    );
    seed_outer_type_arguments(&mut substitution, outer_type_arguments);
    enforce_inferred_constraints(function_signature, &mut substitution, ctx);
    apply_uninferred_type_parameter_defaults(
        function_signature,
        arguments.len(),
        expected_return_type.is_some(),
        &mut substitution,
        ctx,
    );

    let inferred_nothing = substitution
        .iter()
        .filter(|(name, _)| {
            !outer_type_arguments
                .iter()
                .any(|(outer, _)| outer == name.as_ref())
        })
        .all(|(_, candidate)| candidate.is_unknown());
    if inferred_nothing {
        record_generic_call_inference_failed();
        if is_declaration_backed_lazy_signature(function_type) || !outer_type_arguments.is_empty() {
            return instantiate_function_type_with_substitution(
                function_type,
                function_signature,
                &substitution,
                false,
                ctx,
            );
        }
        return Cow::Borrowed(function_type);
    }

    record_generic_call_inference_success();
    let instantiated = instantiate_function_type_with_substitution(
        function_type,
        function_signature,
        &substitution,
        true,
        ctx,
    );
    fold_overload_alternative_parameters(
        instantiated,
        function_type,
        function_signature,
        outer_type_arguments,
        type_arguments,
        arguments,
        symbols,
        ctx,
    )
}

/// Restores an overload group's parameter fold after instantiation.
///
/// The group's value type already holds the union of every overload's parameter
/// at a position, but a generic call rebuilds each parameter from the *kept*
/// signature's written annotation, which leaves only the first overload's shape
/// behind: an argument written for a later overload
/// (`useQuery({ queryKey, queryFn })`, whose first overload demands
/// `initialData`) was then reported against the first. Each later overload is
/// instantiated against the same call and its parameter unioned back in.
///
/// Return type, arity and variadic flag are untouched — which overload's return
/// applies needs real overload resolution, and widening it here would degrade
/// every generic group's result.
fn fold_overload_alternative_parameters<'a>(
    instantiated: Cow<'a, FunctionType>,
    function_type: &FunctionType,
    function_signature: &FunctionSignatureInfo,
    outer_type_arguments: &[(String, Type)],
    type_arguments: &[ParsedType],
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Cow<'a, FunctionType> {
    if function_signature.overload_alternatives.is_empty() {
        return instantiated;
    }

    let mut parameters = instantiated.parameters().to_vec();
    let mut folded = false;
    // The kept signature first, then each alternative in declaration order:
    // the same list a non-generic group carries, instantiated for this call.
    let mut members = Vec::with_capacity(1 + function_signature.overload_alternatives.len());
    members.push(instantiated.clone().into_owned());
    // An alternative's own annotations are resolved here only to read their
    // shape; a constraint violation or an unresolved name in an overload this
    // call did not pick is not the call's error.
    let diagnostics_before = ctx.diagnostics.len();

    for alternative in &function_signature.overload_alternatives {
        let mut substitution = if type_arguments.is_empty() {
            let mut substitution =
                infer_type_argument_substitution(alternative, arguments, &[], None, symbols, ctx);
            apply_uninferred_type_parameter_defaults(
                alternative,
                arguments.len(),
                false,
                &mut substitution,
                ctx,
            );
            substitution
        } else {
            let saved_symbols = std::mem::replace(&mut ctx.symbols, symbols.clone());
            let substitution =
                explicit_type_argument_substitution(alternative, type_arguments, ctx);
            ctx.symbols = saved_symbols;
            substitution
        };
        seed_outer_type_arguments(&mut substitution, outer_type_arguments);

        let alternative_type = instantiate_function_type_with_substitution(
            function_type,
            alternative,
            &substitution,
            false,
            ctx,
        );

        members.push(alternative_type.clone().into_owned());

        for (index, parameter) in parameters.iter_mut().enumerate() {
            let Some(candidate) = alternative_type.parameters().get(index) else {
                continue;
            };
            // A candidate standing at the degradation sentinel says nothing about
            // what this position accepts, and folding it in would make the
            // position permissive enough to swallow a real mismatch.
            if candidate == parameter || candidate.is_unknown() {
                continue;
            }
            *parameter = surge_ts_types::union_type(vec![parameter.clone(), candidate.clone()]);
            folded = true;
        }
    }

    ctx.diagnostics.truncate(diagnostics_before);

    if !folded {
        return Cow::Owned(instantiated.into_owned().with_overloads(members));
    }

    Cow::Owned(
        alloc_function_type(
            parameters,
            instantiated.return_type().clone(),
            instantiated.is_variadic(),
            instantiated.required_parameter_count(),
        )
        .with_overloads(members),
    )
}

/// The declaring interface's own type parameters, bound by the reference that
/// named it. They are not inferred from the call and must not take part in the
/// "inference found nothing" test, but the written annotation mentions them, so
/// re-resolution needs them present. A name the signature already bound wins:
/// an inner type parameter shadows an outer one of the same name.
/// The written element a call argument lands in when the signature's last
/// parameter is a rest tuple (`...params: [TClient] | [TClient, Config<T>]`):
/// the tuple arm with the call's arity is selected and its element at the
/// argument's offset is the inference target. `None` when the last parameter
/// is not such a tuple, or no arm has that arity.
fn rest_tuple_parameter_element(
    function_signature: &FunctionSignatureInfo,
    arity: usize,
    index: usize,
) -> Option<&ParsedType> {
    let last = function_signature.parameter_types.len().checked_sub(1)?;
    if index < last {
        return None;
    }
    let rest_type = function_signature.parameter_types.get(last)?.as_ref()?;
    let offset = index - last;
    let tuple_arity = arity - last;
    let arms: Vec<&std::sync::Arc<Vec<ParsedType>>> = match rest_type {
        ParsedType::Tuple(elements) => vec![elements],
        ParsedType::Union(members) => members
            .iter()
            .filter_map(|member| match member {
                ParsedType::Tuple(elements) => Some(elements),
                _ => None,
            })
            .collect(),
        _ => return None,
    };
    arms.iter()
        .find(|elements| elements.len() == tuple_arity)
        .and_then(|elements| elements.get(offset))
}

enum RestInferenceTarget<'a> {
    Whole(&'a ParsedType),
    Element(&'a ParsedType),
}

/// What a rest argument infers against. A rest written as one of the
/// signature's own type parameters (`<E extends any[]>(...args: E)`) is bound
/// to the tuple of all rest arguments; `f(1, [1, 2])` binds `E` to
/// `[number[]]`, not `number[]` as matching the first argument alone did (which
/// then rejected that very argument against the element `number`). A rest
/// written as an array (`...args: T[]`) infers its element from each argument.
/// A tuple-shaped rest keeps the positional path.
fn rest_parameter_inference_target<'a>(
    written: &'a ParsedType,
    function_signature: &FunctionSignatureInfo,
) -> Option<RestInferenceTarget<'a>> {
    match written {
        ParsedType::Array(element) => Some(RestInferenceTarget::Element(element)),
        ParsedType::Named(named)
            if named.type_arguments.is_empty()
                && function_signature
                    .type_parameters
                    .iter()
                    .any(|type_parameter| type_parameter.name == named.name) =>
        {
            Some(RestInferenceTarget::Whole(written))
        }
        _ => None,
    }
}

/// The tuple of the rest arguments' (widened) types, or `None` when any of
/// them is unresolved — a partial tuple would bind the wrong arity.
fn rest_arguments_tuple(
    arguments: &[ParsedCallArgument],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let mut elements = Vec::with_capacity(arguments.len());
    for argument in arguments {
        let diagnostics_before = ctx.diagnostics().len();
        let inferred = infer_expression(&argument.expression, symbols, ctx);
        ctx.truncate_diagnostics(diagnostics_before);
        let InferredExpression::Known(argument_type) = inferred else {
            return None;
        };
        if argument_type.is_unknown()
            || crate::checks::expr::carries_leaked_type_parameter(&argument_type, ctx)
        {
            return None;
        }
        elements.push(crate::checks::expr::widen_type(&argument_type));
    }
    Some(Type::Tuple(elements))
}

fn seed_outer_type_arguments(
    substitution: &mut TypeParameterSubstitution,
    outer_type_arguments: &[(String, Type)],
) {
    for (name, ty) in outer_type_arguments {
        if substitution.get(name).is_some() {
            continue;
        }
        substitution.insert(name.clone(), ty.clone());
    }
}

fn is_declaration_backed_lazy_signature(function_type: &FunctionType) -> bool {
    function_type
        .parameters()
        .iter()
        .chain(std::iter::once(function_type.return_type()))
        .any(|ty| {
            matches!(
                ty,
                Type::Reference(reference)
                    if reference.id.contains("\0signature-annotation\0")
            )
        })
}

/// Runs `resolve` with the checker positioned in the signature's declaring
/// file and namespace, so annotations written there (a module-local or imported
/// name the caller cannot see) resolve against that scope rather than the call
/// site's.
fn with_declaring_scope<R>(
    function_signature: &FunctionSignatureInfo,
    ctx: &mut CheckerContext,
    resolve: impl FnOnce(&mut CheckerContext) -> R,
) -> R {
    let declaring_file = function_signature
        .declaring_file
        .as_deref()
        .filter(|file| *file != ctx.file_name);
    let saved_file_name = declaring_file.map(|file| {
        let saved = ctx.file_name.clone();
        ctx.set_file_name(file.to_string());
        saved
    });
    let namespace_prefix = function_signature.namespace_prefix.clone();
    if let Some(prefix) = namespace_prefix.as_deref() {
        ctx.namespace_member_resolution_depth += 1;
        ctx.namespace_member_prefix_stack.push(prefix.to_string());
    }

    let resolved = resolve(ctx);

    if namespace_prefix.is_some() {
        ctx.namespace_member_prefix_stack.pop();
        ctx.namespace_member_resolution_depth -= 1;
    }
    if let Some(saved) = saved_file_name {
        ctx.set_file_name(saved);
    }
    resolved
}

pub(crate) fn instantiate_function_type_with_substitution<'a>(
    function_type: &'a FunctionType,
    function_signature: &FunctionSignatureInfo,
    substitution: &TypeParameterSubstitution,
    suppress_tuple_return_type: bool,
    ctx: &mut CheckerContext,
) -> Cow<'a, FunctionType> {
    // The declared parameter/return annotations may reference the declaring
    // module's local types (an imported generic like react-hook-form's
    // `useForm(props?: UseFormProps<…>)`), which the caller's file scope cannot
    // see; resolving under the declaring file keys the `module_scope_by_file`
    // fallback to the right per-file scope. Substituted type arguments are
    // already-resolved `Type`s, so they are unaffected by the swap.
    let declaring_file = function_signature
        .declaring_file
        .as_deref()
        .filter(|file| *file != ctx.file_name);
    let saved_file_name = declaring_file.map(|file| {
        let saved = ctx.file_name.clone();
        ctx.set_file_name(file.to_string());
        saved
    });
    // A namespace member's annotations name its siblings unqualified; they live
    // under qualified keys, so re-resolution needs the declaring namespace's
    // scope (the same one the type-member bodies expand in).
    let namespace_prefix = function_signature.namespace_prefix.clone();
    if let Some(prefix) = namespace_prefix.as_deref() {
        ctx.namespace_member_resolution_depth += 1;
        ctx.namespace_member_prefix_stack.push(prefix.to_string());
    }

    // Re-resolving a written annotation under a substitution is not a fresh
    // declaration check: the declaration was already checked where it was
    // written, against the type parameter's *constraint*. Anything raised here
    // is raised at the declaration's span from a call site that cannot see it —
    // `(o: K[1])` under `K = [""]` reported a false TS2493 on the parameter
    // list of a generic the call never looked at.
    let diagnostics_before = ctx.diagnostics.len();

    let instantiated = with_type_copy_reason(TypeCopyReason::CallResolution, || {
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
    });

    ctx.diagnostics.truncate(diagnostics_before);

    if namespace_prefix.is_some() {
        ctx.namespace_member_prefix_stack.pop();
        ctx.namespace_member_resolution_depth -= 1;
    }
    if let Some(saved) = saved_file_name {
        ctx.set_file_name(saved);
    }
    instantiated
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
            &[],
            type_arguments,
            type_argument_span,
            arguments,
            None,
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
    for (index, type_parameter) in function_signature.type_parameters.iter().enumerate() {
        // A call may supply fewer type arguments than the signature declares;
        // the rest take their declared default. Leaving them out of the
        // substitution let the *name* survive into the instantiated
        // parameter/return types, where it read as an unknown type
        // (`registry<GlobalMeta>()` against `registry<T, S>(): Registry<T, S>`
        // reported a false TS2304 for `S`). Defaults resolve under the
        // substitution built so far, so one that names an earlier parameter
        // (`S extends T = T`) sees it.
        let resolved = match type_arguments.get(index) {
            Some(type_argument) => map_parsed_type_with_substitution(
                type_argument.clone(),
                ctx,
                &TypeParameterSubstitution::new(),
            ),
            None => match type_parameter.default_type.clone() {
                // A default annotation is written in the DECLARING file and may
                // name that module's imports (`TSSRContext = NextPageContext`,
                // which `@trpc/next` imports and the app calling it cannot see),
                // so it resolves under the declaring scope, not the call site's.
                Some(default_type) => with_declaring_scope(function_signature, ctx, |ctx| {
                    map_parsed_type_with_substitution(default_type, ctx, &substitution)
                }),
                // No default: the parameter is genuinely unconstrained here, so
                // it degrades rather than leaking its name.
                None => Type::Unknown,
            },
        };
        substitution.insert(type_parameter.name.clone(), resolved);
    }
    substitution
}

/// Binds the type parameters inference left untouched to their declared
/// defaults — but only when that completes the binding, i.e. every parameter
/// of the signature is then either inferred or defaulted. `vi.fn()` is the
/// shape that needs it: `fn<T extends Procedure = Procedure>(impl?: T):
/// Mock<T>` called with no argument infers nothing, and without the default
/// the result kept the bare parameter, so `Mock<T>`'s conditional operand
/// (which carries the call signature) could never resolve.
///
/// The completeness rule is what makes this safe. Defaulting *some* parameters
/// while another stays a placeholder moved the call off the all-unknown bail
/// and into an instantiation with a placeholder argument, which picked a wrong
/// conditional branch (ofetch's `$fetch<T = any, R = "json">(url, options)`,
/// where surge could not infer `R` from the options object). Inference seeds
/// every parameter as a placeholder and inserts any candidate as a real
/// binding, so the placeholder flag separates "nothing inferred" from
/// "`unknown` inferred". Defaults resolve in the declaring scope, under the
/// bindings made so far, so `S extends T = T` sees `T`.
fn apply_uninferred_type_parameter_defaults(
    function_signature: &FunctionSignatureInfo,
    argument_count: usize,
    has_expected_return_type: bool,
    substitution: &mut TypeParameterSubstitution,
    ctx: &mut CheckerContext,
) {
    // A parameter the call had an inference *source* for — a supplied argument
    // whose annotation mentions it, or a contextual return type over a return
    // annotation that does — is one surge failed to infer, not one tsc would
    // default: zod's `hash(alg, { enc: "base64" })` against `Enc = "hex"`
    // reported every call once the default stood in for the failed inference.
    // An unannotated parameter counts as mentioning everything.
    if argument_count > function_signature.parameter_types.len() {
        return;
    }
    let has_inference_source = |name: &str| {
        function_signature.parameter_types[..argument_count]
            .iter()
            .any(|parameter_type| match parameter_type {
                Some(parameter_type) => parsed_type_mentions_name(parameter_type, name),
                None => true,
            })
            || (has_expected_return_type
                && function_signature
                    .return_type
                    .as_ref()
                    .is_none_or(|return_type| parsed_type_mentions_name(return_type, name)))
    };
    let uninferred: Vec<&surge_ts_syntax::ParsedTypeParameter> = function_signature
        .type_parameters
        .iter()
        .filter(|type_parameter| substitution.is_placeholder(&type_parameter.name))
        .collect();
    if uninferred.is_empty()
        || uninferred.iter().any(|type_parameter| {
            type_parameter.default_type.is_none() || has_inference_source(&type_parameter.name)
        })
    {
        return;
    }

    let mut bound = substitution.clone_with_reason(TypeCopyReason::SubstitutionChanged);
    for type_parameter in uninferred {
        let default_type = type_parameter.default_type.clone().expect("checked above");
        let snapshot = bound.clone_with_reason(TypeCopyReason::SubstitutionChanged);
        let resolved = with_declaring_scope(function_signature, ctx, |ctx| {
            map_parsed_type_with_substitution(default_type, ctx, &snapshot)
        });
        if type_contains_unknown(&resolved) {
            return;
        }
        bound.insert(type_parameter.name.clone(), resolved);
    }
    *substitution = bound;
}

/// Whether a parsed annotation names `name` anywhere within it. Positions this
/// walker does not descend into (`typeof`, indexed access, mapped, conditional,
/// template, predicate) report `true`: the caller uses a mention as a reason
/// *not* to act, so over-reporting is the safe direction.
fn parsed_type_mentions_name(ty: &ParsedType, name: &str) -> bool {
    match ty {
        ParsedType::Named(named) => {
            named.name == name
                || named
                    .type_arguments
                    .iter()
                    .any(|argument| parsed_type_mentions_name(argument, name))
        }
        ParsedType::Object(object) => {
            object
                .properties
                .iter()
                .any(|property| parsed_type_mentions_name(&property.ty, name))
                || object
                    .construct_signature
                    .as_deref()
                    .into_iter()
                    .chain(object.call_signature.as_deref())
                    .any(|signature| {
                        signature
                            .parameters
                            .iter()
                            .any(|parameter| parsed_type_mentions_name(&parameter.ty, name))
                            || parsed_type_mentions_name(&signature.return_type, name)
                    })
        }
        ParsedType::Array(inner) | ParsedType::KeyOf(inner) => {
            parsed_type_mentions_name(inner, name)
        }
        ParsedType::Union(members)
        | ParsedType::Intersection(members)
        | ParsedType::Tuple(members) => members
            .iter()
            .any(|member| parsed_type_mentions_name(member, name)),
        ParsedType::Function(function) => {
            function
                .parameters
                .iter()
                .any(|parameter| parsed_type_mentions_name(&parameter.ty, name))
                || parsed_type_mentions_name(&function.return_type, name)
        }
        ParsedType::TypeOf(_)
        | ParsedType::IndexedAccess(_)
        | ParsedType::Mapped(_)
        | ParsedType::Conditional(_)
        | ParsedType::TemplateLiteral(_)
        | ParsedType::Predicate(_)
        | ParsedType::Infer(_) => true,
        _ => false,
    }
}

/// An inferred candidate that does not satisfy its parameter's constraint is
/// replaced by the constraint, as tsc does (`transform({ value: 1 })` against
/// `T extends string` instantiates `{ value: string }` and reports the
/// argument).
///
/// tsc substitutes unconditionally because its inference is complete. Here a
/// failed assignability check is at least as often surge's own inference miss,
/// and the substituted constraint re-types every parameter of the signature —
/// so one miss becomes a TS2345 at every argument of the call. Both operands
/// must therefore be settled before the check counts; an unsettled one leaves
/// the candidate alone.
fn enforce_inferred_constraints(
    function_signature: &FunctionSignatureInfo,
    substitution: &mut TypeParameterSubstitution,
    ctx: &mut CheckerContext,
) {
    for type_parameter in &function_signature.type_parameters {
        let Some(constraint) = type_parameter.constraint.clone() else {
            continue;
        };
        let Some(candidate) = substitution.get(&type_parameter.name).cloned() else {
            continue;
        };
        if !constraint_operand_is_settled(&candidate) {
            continue;
        }
        let resolved = with_declaring_scope(function_signature, ctx, |ctx| {
            try_map_parsed_type_with_substitution(constraint, ctx, substitution)
        });
        // A constraint that failed to resolve — an alias surge rejected as a
        // cycle, a name it could not find, a recursion it cut — carries no
        // verdict about the candidate. Enforcing the degraded shape anyway
        // installs it as the type argument and rejects every argument against
        // it.
        if resolved.had_error() {
            continue;
        }
        let resolved = resolved.into_ty();
        if !constraint_operand_is_settled(&resolved)
            || matches!(resolved, Type::Any)
            || surge_ts_types::is_assignable_to(&candidate, &resolved)
        {
            continue;
        }
        substitution.set(type_parameter.name.clone(), resolved, false);
    }
}

/// Whether a type is settled enough to stand on either side of the constraint
/// check above.
///
/// `Unknown` is surge's degradation sentinel and `GenuineUnknown` a written
/// `unknown`; neither settles assignability here, so a constraint carrying one
/// (`QueryKey = ReadonlyArray<unknown>`) must not displace an inferred
/// candidate. A lazy reference is declined rather than peeled: forcing an
/// expansion to settle a constraint would expand declarations the call never
/// reads. The walk reaches the places a degraded member hides one level down —
/// index and call/construct signatures included — which a property-only walk
/// misses.
fn constraint_operand_is_settled(ty: &Type) -> bool {
    match ty {
        Type::Unknown | Type::GenuineUnknown | Type::TypeParameter(_) | Type::Reference(_) => false,
        Type::Array(element) => constraint_operand_is_settled(element),
        Type::Tuple(elements) => elements.iter().all(constraint_operand_is_settled),
        Type::Union(union) => union.types().iter().all(constraint_operand_is_settled),
        Type::Function(function) => constraint_signature_is_settled(function),
        Type::Object(object) => {
            object
                .properties
                .values()
                .all(|property| constraint_operand_is_settled(&property.ty))
                && object
                    .string_index_type
                    .as_deref()
                    .is_none_or(constraint_operand_is_settled)
                && object
                    .call_signature
                    .as_deref()
                    .is_none_or(constraint_signature_is_settled)
                && object
                    .construct_signature
                    .as_deref()
                    .is_none_or(constraint_signature_is_settled)
        }
        _ => true,
    }
}

fn constraint_signature_is_settled(function: &FunctionType) -> bool {
    function
        .parameters()
        .iter()
        .all(constraint_operand_is_settled)
        && constraint_operand_is_settled(function.return_type())
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
    outer_type_arguments: &[(String, Type)],
    expected_return_type: Option<&Type>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> TypeParameterSubstitution {
    let mut substitution = TypeParameterSubstitution::new();
    for type_parameter in &function_signature.type_parameters {
        substitution.insert_placeholder(
            type_parameter.name.clone(),
            Type::type_parameter(&type_parameter.name),
        );
        if type_parameter.constraint.is_some() {
            substitution.mark_keeps_literal(&type_parameter.name);
        }
    }

    let rest_index = function_signature
        .rest
        .then(|| function_signature.parameter_types.len().checked_sub(1))
        .flatten();
    let mut rest_tuple_bound = false;
    let mut deferred_callbacks: Vec<(&ParsedType, &surge_ts_syntax::ParsedArrowFunction)> =
        Vec::new();
    for (index, argument) in arguments.iter().enumerate() {
        let rest_target = rest_index
            .filter(|rest_index| index >= *rest_index)
            .and_then(|rest_index| function_signature.parameter_types[rest_index].as_ref())
            .and_then(|written| rest_parameter_inference_target(written, function_signature));
        let parameter_type = match rest_target {
            // `...args: E` binds `E` to the tuple of every rest argument, once.
            Some(RestInferenceTarget::Whole(parameter_type)) => {
                if !rest_tuple_bound {
                    rest_tuple_bound = true;
                    if let Some(tuple) = rest_arguments_tuple(&arguments[index..], symbols, ctx) {
                        with_declaring_scope(function_signature, ctx, |ctx| {
                            collect_inferred_type_argument(
                                parameter_type,
                                &tuple,
                                &mut substitution,
                                false,
                                ctx,
                                0,
                            );
                        });
                    }
                }
                continue;
            }
            Some(RestInferenceTarget::Element(element)) => element,
            None => {
                let Some(parameter_type) = function_signature
                    .parameter_types
                    .get(index)
                    .and_then(|ty| ty.as_ref())
                    .or_else(|| {
                        rest_tuple_parameter_element(function_signature, arguments.len(), index)
                    })
                    .map(|ty| {
                        rest_tuple_parameter_element(function_signature, arguments.len(), index)
                            .filter(|_| index + 1 == function_signature.parameter_types.len())
                            .unwrap_or(ty)
                    })
                else {
                    continue;
                };
                parameter_type
            }
        };

        // A callback with an un-annotated parameter is *context-sensitive*: what
        // its body — and so its return type — infers depends on the parameter
        // types this very signature gives it. Sketching it now types those
        // parameters as `any`, and a type parameter appearing only in the
        // callback's return position would be bound to `any` from the `any`
        // body. It waits for the second pass below, which types it against the
        // parameters the arguments here have already pinned.
        if let Some(callback) = context_sensitive_callback_argument(parameter_type, argument) {
            deferred_callbacks.push((parameter_type, callback));
            continue;
        }

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
        let inferred_argument = match array_literal_tuple_inference(
            parameter_type,
            &function_signature.type_parameters,
            &argument.expression,
            symbols,
            ctx,
        ) {
            Some(tuple) => InferredExpression::Known(tuple),
            None => infer_expression(&argument.expression, symbols, ctx),
        };
        ctx.truncate_diagnostics(diagnostics_before);
        let InferredExpression::Known(argument_type) = inferred_argument else {
            record_generic_call_inference_unresolved_argument_skip();
            continue;
        };

        // A leaked placeholder (`Mock<T>` off a `vi.fn()` whose `T` no scope
        // binds) infers garbage — `TData` as the mock's own call signature —
        // so the argument contributes nothing, as an unresolved one does.
        if argument_type.is_unknown()
            || crate::checks::expr::carries_leaked_type_parameter(&argument_type, ctx)
        {
            record_generic_call_inference_unresolved_argument_skip();
            continue;
        }

        // The argument's type is in hand; matching it against the written
        // parameter (`FetchOptions<R>`) resolves the declaring module's names,
        // so that side runs under the declaring file like instantiation does.
        let widen_literals = widens_a_fresh_literal_argument(
            parameter_type,
            &argument.expression,
            &function_signature.type_parameters,
        );
        with_declaring_scope(function_signature, ctx, |ctx| {
            collect_inferred_type_argument(
                parameter_type,
                &argument_type,
                &mut substitution,
                widen_literals,
                ctx,
                0,
            );
        });
    }

    infer_from_context_sensitive_callbacks(
        function_signature,
        &deferred_callbacks,
        outer_type_arguments,
        &mut substitution,
        symbols,
        ctx,
    );

    with_declaring_scope(function_signature, ctx, |ctx| {
        infer_type_arguments_from_expected_return_type(
            function_signature,
            expected_return_type,
            &mut substitution,
            ctx,
        );
    });

    substitution
}

/// The written function annotation a callback argument is matched against, with
/// the optionality a lib signature wraps it in removed: `then`'s parameter is
/// `((value: T) => TResult1 | PromiseLike<TResult1>) | undefined | null`, and
/// the callback is the union's only function member.
fn callback_parameter_annotation(
    parameter_type: &ParsedType,
) -> Option<&std::sync::Arc<surge_ts_syntax::ParsedFunctionType>> {
    match parameter_type {
        ParsedType::Function(function) => Some(function),
        ParsedType::Union(members) => {
            let mut functions = members.iter().filter_map(|member| match member {
                ParsedType::Function(function) => Some(function),
                _ => None,
            });
            let function = functions.next()?;
            functions.next().is_none().then_some(function)
        }
        _ => None,
    }
}

/// An arrow argument whose parameters this signature is the one to type: at
/// least one is written without an annotation, and the parameter it is passed
/// to is a callback the signature spells out. A generic arrow keeps its own
/// type parameters and is left to the ordinary path.
fn context_sensitive_callback_argument<'a>(
    parameter_type: &ParsedType,
    argument: &'a ParsedCallArgument,
) -> Option<&'a surge_ts_syntax::ParsedArrowFunction> {
    let callback = callback_parameter_annotation(parameter_type)?;
    let surge_ts_syntax::ParsedExpression::ArrowFunction(arrow) = &argument.expression else {
        return None;
    };
    if !arrow.type_parameters.is_empty() || arrow.parameters.is_empty() {
        return None;
    }
    if !arrow
        .parameters
        .iter()
        .any(|parameter| parameter.declared_type.is_none())
    {
        return None;
    }
    // Nothing to gain when the signature does not type the position either.
    callback
        .parameters
        .iter()
        .any(|parameter| !parameter.is_this)
        .then_some(arrow.as_ref())
}

/// The second inference pass: every deferred callback is sketched with the
/// parameter types the signature gives it — the first pass's candidates
/// substituted in, over the enclosing bindings the member was read under — and
/// the sketch then infers the type parameters the callback's return names.
fn infer_from_context_sensitive_callbacks(
    function_signature: &FunctionSignatureInfo,
    deferred_callbacks: &[(&ParsedType, &surge_ts_syntax::ParsedArrowFunction)],
    outer_type_arguments: &[(String, Type)],
    substitution: &mut TypeParameterSubstitution,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    if deferred_callbacks.is_empty() {
        return;
    }
    // The enclosing interface's own arguments (`T` of the `Box<string>` the
    // method was read off) are seeded here and here only: they type the
    // callback's parameters, but they are not this call's to infer, and the
    // caller seeds them into the real substitution after inference.
    let mut contextual_substitution =
        substitution.clone_with_reason(TypeCopyReason::CallResolution);
    seed_outer_type_arguments(&mut contextual_substitution, outer_type_arguments);

    for (parameter_type, arrow) in deferred_callbacks {
        let Some(callback) = callback_parameter_annotation(parameter_type) else {
            continue;
        };
        let diagnostics_before = ctx.diagnostics().len();
        let contextual_parameters = with_declaring_scope(function_signature, ctx, |ctx| {
            callback
                .parameters
                .iter()
                .filter(|parameter| !parameter.is_this)
                .map(|parameter| {
                    map_parsed_type_with_substitution(
                        parameter.ty.clone(),
                        ctx,
                        &contextual_substitution,
                    )
                })
                .collect::<Vec<_>>()
        });
        let sketch = Type::Function(
            crate::infer::expression::infer_arrow_function_with_contextual_parameters(
                arrow,
                &contextual_parameters,
                symbols,
                ctx,
            ),
        );
        ctx.truncate_diagnostics(diagnostics_before);
        if crate::checks::expr::carries_leaked_type_parameter(&sketch, ctx) {
            continue;
        }
        with_declaring_scope(function_signature, ctx, |ctx| {
            collect_inferred_type_argument(
                &ParsedType::Function(callback.clone()),
                &sketch,
                substitution,
                false,
                ctx,
                0,
            );
        });
    }
}

/// tsc infers the *widened* type from a literal expression — `behaviorSubject(1)`
/// is `BehaviorSubject<number>`, so a later `value.next(2)` is fine. The literal
/// survives only when it is not fresh (`{ n: 1 } as const`, an annotated
/// binding) or when the parameter's constraint asks for one, so this is limited
/// to a literal written at the call site inferring a bare type parameter.
fn widens_a_fresh_literal_argument(
    parameter_type: &ParsedType,
    argument: &surge_ts_syntax::ParsedExpression,
    type_parameters: &[surge_ts_syntax::ParsedTypeParameter],
) -> bool {
    // A fresh literal: a primitive literal, or an object/array literal whose
    // property and element literals widen the same way (`subject({ n: 1 })`
    // binds `T` to `{ n: number }`). A `const` assertion is not fresh.
    if !matches!(
        argument,
        surge_ts_syntax::ParsedExpression::StringLiteral(_)
            | surge_ts_syntax::ParsedExpression::NumberLiteral(_)
            | surge_ts_syntax::ParsedExpression::BooleanLiteral(_)
            | surge_ts_syntax::ParsedExpression::ObjectLiteral { .. }
            | surge_ts_syntax::ParsedExpression::ArrayLiteral { .. }
    ) {
        return false;
    }
    let ParsedType::Named(named) = parameter_type else {
        return false;
    };
    if !named.type_arguments.is_empty() {
        return false;
    }
    // Only an *unconstrained* parameter widens. A constraint decides on its own
    // whether the literal survives — `T extends string` and `T extends keyof O`
    // keep it, and so does a named alias for a literal union, which cannot be
    // told apart from any other alias without resolving it. Widening those
    // collapsed `Field extends EditableField` to `string` and made
    // `Student[Field]` an invalid index.
    type_parameters.iter().any(|type_parameter| {
        type_parameter.name == named.name && type_parameter.constraint.is_none()
    })
}

/// What a tuple-shaped constraint says its elements are. `[A, ...A[]] | []` —
/// zod's `tuple` and every other "one or more, or none" signature — describes
/// the elements in its non-empty member; the `[]` member only says the argument
/// may also be empty. Without looking through the union the parameter stayed
/// unsolved and landed on that empty tuple, so every well-formed argument
/// reported `Type '[…]' is not assignable to type '[]'`.
fn tuple_element_constraint(constraint: &ParsedType) -> Option<&ParsedType> {
    match constraint {
        ParsedType::Array(element) => Some(element.as_ref()),
        ParsedType::Tuple(elements) => elements.first(),
        ParsedType::Union(members) => members.iter().find_map(tuple_element_constraint),
        _ => None,
    }
}

/// tsc infers an array-literal argument as a *tuple* when the inference target
/// is a tuple-shaped type parameter, and keeps its element literal types when
/// that tuple's element type is itself a parameter constrained to a primitive.
/// Both halves are load-bearing for the `[T, ...T[]]` enum-builder idiom
/// (`arrayToEnum(["a", "b"])` must yield `{ a: "a"; b: "b" }`, not
/// `{ [k: string]: string }`); array inference alone collapses the member
/// literals and every `switch (issue.code)` narrowing built on them.
fn array_literal_tuple_inference(
    parameter_type: &ParsedType,
    type_parameters: &[surge_ts_syntax::ParsedTypeParameter],
    argument: &surge_ts_syntax::ParsedExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let surge_ts_syntax::ParsedExpression::ArrayLiteral { elements, .. } = argument else {
        return None;
    };
    if elements.is_empty() {
        return None;
    }
    let ParsedType::Named(named) = parameter_type else {
        return None;
    };
    if !named.type_arguments.is_empty() {
        return None;
    }
    let constraint = type_parameters
        .iter()
        .find(|type_parameter| type_parameter.name == named.name)?
        .constraint
        .as_ref()?;
    let element_constraint = tuple_element_constraint(constraint)?;
    let keep_literals = matches!(
        element_constraint,
        ParsedType::String | ParsedType::Number | ParsedType::Boolean
    ) || matches!(
        element_constraint,
        ParsedType::Named(element_named)
            if type_parameters.iter().any(|type_parameter| {
                type_parameter.name == element_named.name
                    && matches!(
                        type_parameter.constraint,
                        Some(ParsedType::String | ParsedType::Number | ParsedType::Boolean)
                    )
            })
    );

    let mut element_types = Vec::with_capacity(elements.len());
    for element in elements {
        let InferredExpression::Known(element_type) =
            infer_expression(&element.expression, symbols, ctx)
        else {
            return None;
        };
        if element_type.is_unknown() {
            return None;
        }
        element_types.push(if keep_literals {
            element_type
        } else {
            crate::checks::expr::widen_type(&element_type)
        });
    }
    Some(Type::Tuple(element_types))
}

/// A type parameter that occurs only in the return type (`$constructor<T>`,
/// `Ctor<T>`) is inferable solely from the call's contextual type — no argument
/// mentions it, so it would otherwise stay `Type::Unknown` and degrade every
/// callback parameter typed by it. Runs after the argument loop so an
/// argument-derived candidate always wins, and only for a generic-instantiation
/// return annotation, where the match is positional against the expected
/// reference's type arguments rather than a whole-type guess.
fn infer_type_arguments_from_expected_return_type(
    function_signature: &FunctionSignatureInfo,
    expected_return_type: Option<&Type>,
    substitution: &mut TypeParameterSubstitution,
    ctx: &mut CheckerContext,
) {
    let Some(expected_return_type) = expected_return_type else {
        return;
    };
    // The expected return type is a lower-priority source than the arguments:
    // it fills in what they left open and never overrides what they fixed.
    // A conditional return (`Promise<MappedResponseType<R, T>>`) matched
    // against a contextual type is otherwise free to re-bind `R`.
    let unresolved: Vec<String> = substitution
        .iter()
        .filter(|(_, candidate)| candidate.is_unknown())
        .map(|(name, _)| name.to_string())
        .collect();
    if unresolved.is_empty() {
        return;
    }
    let Some(ParsedType::Named(declared_return_type)) = function_signature.return_type.as_ref()
    else {
        return;
    };
    if declared_return_type.type_arguments.is_empty() {
        return;
    }

    let mut from_return = substitution.clone_with_reason(TypeCopyReason::CallResolution);
    infer_through_generic_reference(
        declared_return_type,
        expected_return_type,
        &mut from_return,
        ctx,
        0,
    );
    for name in unresolved {
        if let Some(candidate) = from_return.get(&name)
            && !candidate.is_unknown()
        {
            substitution.set(name, candidate.clone(), false);
        }
    }
}

pub(crate) fn collect_inferred_type_argument(
    parameter_type: &ParsedType,
    argument_type: &Type,
    substitution: &mut TypeParameterSubstitution,
    widen_literals: bool,
    ctx: &mut CheckerContext,
    depth: usize,
) {
    // A degraded *part* of an argument does not discard the whole: a callback
    // whose body the sketch cannot type (a block with no `return`, or a call
    // surge does not model) still proves what its parameters are, and an object
    // literal still proves its other members. Only the recording of a candidate
    // checks the shape it is about to bind, so the sentinel never reaches a
    // substitution.
    if argument_type.is_unknown() {
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
            // `Array<T>` / `ReadonlyArray<T>` spelled as a reference infers the
            // same way as the `T[]` shorthand: element-wise, against an array or
            // tuple argument. Falling through to the member walk below instead
            // inferred nothing, because an array argument is `Type::Array` and
            // has no object surface to match the interface's members against —
            // so `addToStart(items, item)` against
            // `addToStart<T>(items: Array<T>, item: T)` saw only the second
            // argument and instantiated `T` as the literal `4`, then rejected
            // `number[]` against `4[]`.
            // The same element-wise inference serves every lib interface whose
            // FIRST type argument is the element an array satisfies it with:
            // `Object.fromEntries(entries: Iterable<readonly [PropertyKey, T]>)`
            // is handed a `[string, V][]`, and without this `T` is never
            // inferred, the call falls to the `any`-returning overload, and the
            // index signature tsc gives the result — with every `TS4111` that
            // depends on it — disappears. `Type::Array` has no object surface,
            // so the member walk below cannot do it.
            if !named_type.type_arguments.is_empty()
                && matches!(
                    named_type.name.as_str(),
                    "Array" | "ReadonlyArray" | "Iterable" | "IterableIterator"
                )
            {
                let element_type = &named_type.type_arguments[0];
                match argument_type {
                    Type::Array(actual_element_type) => {
                        collect_inferred_type_argument(
                            element_type,
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
                                element_type,
                                element,
                                substitution,
                                true,
                                ctx,
                                depth,
                            );
                        }
                    }
                    _ => {}
                }
            }

            // surge models a resolved `Promise<T>` as its awaited `T`, so a
            // written `Promise<TData>` is handed the awaited value itself —
            // `fn: (v: string) => Promise<TData>` against a callback returning
            // `string` — and the generic-reference walk below, which matches
            // member for member, infers nothing from a primitive. Infer the
            // argument against the awaited actual, which is the actual itself
            // when it is already awaited.
            if named_type.type_arguments.len() == 1
                && matches!(named_type.name.as_str(), "Promise" | "PromiseLike")
            {
                let awaited = crate::checks::call::promise_like_awaited_type(argument_type);
                collect_inferred_type_argument(
                    &named_type.type_arguments[0],
                    &awaited,
                    substitution,
                    widen_literals,
                    ctx,
                    depth,
                );
            }

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
        // A callback parameter (`factory: () => T`, `initialState: () => S`):
        // match the argument's own signature so the type parameter is inferred
        // from what the callback takes and returns.
        ParsedType::Function(expected_function) => {
            // A callable object (vitest's `Mock<…>`, an interface with a call
            // signature) infers through its signature the way a function does.
            let actual_function = match argument_type.peeled() {
                Type::Function(function) => function,
                Type::Object(object) => match object.call_signature() {
                    Some(signature) => signature.clone(),
                    None => return,
                },
                _ => return,
            };
            let actual_parameters = actual_function.parameters();
            let rest_index = actual_function
                .is_variadic()
                .then(|| actual_parameters.len().checked_sub(1))
                .flatten();
            for (index, expected_parameter) in expected_function.parameters.iter().enumerate() {
                // A rest parameter stands for every position from its own on,
                // and it is its *element* the expected parameter there lines up
                // with: `(...args: any[]) => any` — vitest's `vi.fn()` — binds
                // each of `(data: TData, variables: TVariables)` to `any`, as
                // tsc does, rather than the first to `any[]` and the rest to
                // nothing (which left `TVariables` at its `void` default and
                // rejected every `mutate(1)`).
                let rest_element = match rest_index {
                    Some(rest_index) if index >= rest_index => Some(rest_parameter_element_type(
                        &actual_parameters[rest_index],
                        index - rest_index,
                    )),
                    _ => None,
                };
                let Some(actual_parameter) = rest_element
                    .as_ref()
                    .or_else(|| actual_parameters.get(index))
                else {
                    break;
                };
                // A bare `any` on a non-rest position is the sketch's placeholder
                // for an *un-annotated* callback parameter, which is what the
                // signature is about to type — inferring from it would bind the
                // type parameter to `any` and silence the callback body's own
                // errors. A rest element is written, never sketched.
                if rest_element.is_none() && matches!(actual_parameter, Type::Any) {
                    continue;
                }
                collect_inferred_type_argument(
                    &expected_parameter.ty,
                    actual_parameter,
                    substitution,
                    false,
                    ctx,
                    depth,
                );
            }
            collect_inferred_type_argument(
                &expected_function.return_type,
                actual_function.return_type(),
                substitution,
                widen_literals,
                ctx,
                depth,
            );
        }
        ParsedType::Union(expected_types) => {
            // A union carries no member order, so zipping positionally is only
            // meaningful when every member has a shape to pin it to. Once the
            // union mixes a naked type parameter with structured members
            // (`undefined | TValue | ((q: Q) => TValue)`), a same-arity argument
            // union lines up by accident: tanstack's `resolveQueryValue` bound
            // `TValue` to the callback arm and made every `!== false` on the
            // result a false TS2367. Fall through to the shape-matching path.
            let stands_for_the_rest = |member: &ParsedType, ctx: &CheckerContext| {
                is_naked_type_parameter(member, substitution)
                    || is_conditional_alias_reference(member, ctx)
            };
            let mixes_naked_and_structured = expected_types
                .iter()
                .any(|member| stands_for_the_rest(member, ctx))
                && expected_types
                    .iter()
                    .any(|member| !stands_for_the_rest(member, ctx));
            if let Type::Union(actual_union) = argument_type
                && !mixes_naked_and_structured
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
                return;
            }

            // React's `SetStateAction<S>` shape: `S | (() => S)`. Each argument
            // member goes to the union member whose *shape* it matches, and what
            // matches none is what a naked type parameter stands for. The naked
            // one is recorded first: candidates are first-wins, and a direct
            // member (`resultOf(value)` binding `T` to the value itself) is a
            // better answer than one read back out of a callback's return type.
            // A conditional-bodied alias member (`MaybePromise<DefaultValue<In,
            // $Output>>`) has no shape of its own either; what fits no shaped
            // member is what it, like a naked parameter, stands for.
            let (naked, structured): (Vec<&ParsedType>, Vec<&ParsedType>) = expected_types
                .iter()
                .partition(|member| stands_for_the_rest(member, ctx));
            if structured.is_empty() {
                return;
            }
            let argument_members: Vec<&Type> = match argument_type {
                Type::Union(union) => union.types().iter().collect(),
                other => vec![other],
            };
            let mut matches: Vec<(&ParsedType, &Type)> = Vec::new();
            let mut unmatched: Vec<Type> = Vec::new();
            for member in argument_members {
                match structured
                    .iter()
                    .find(|target| parsed_shape_matches(target, member))
                {
                    Some(target) => matches.push((target, member)),
                    None => unmatched.push(member.clone()),
                }
            }
            // Nothing shaped matched and there is no naked member to take the
            // rest: no evidence. With a naked member, an argument none of the
            // shapes fit (`A | B` against `TOut | Promise<TOut>`, or `{ a: 2 }`
            // against `T | undefined`) is what it stands for, in full.
            if matches.is_empty() && naked.is_empty() {
                return;
            }
            if !unmatched.is_empty() {
                let leftover = if unmatched.len() == 1 {
                    unmatched.remove(0)
                } else {
                    surge_ts_types::union_type(unmatched)
                };
                for target in naked {
                    collect_inferred_type_argument(
                        target,
                        &leftover,
                        substitution,
                        widen_literals,
                        ctx,
                        depth,
                    );
                }
            }
            // Several argument members fitting the same shape are one candidate
            // (tsc unions the candidates): matched one at a time, the first-wins
            // rule kept a single member of a union return.
            let mut grouped: Vec<(&ParsedType, Vec<Type>)> = Vec::new();
            for (target, member) in matches {
                match grouped
                    .iter_mut()
                    .find(|(seen, _)| std::ptr::eq(*seen, target))
                {
                    Some((_, members)) => members.push(member.clone()),
                    None => grouped.push((target, vec![member.clone()])),
                }
            }
            for (target, mut members) in grouped {
                let member = if members.len() == 1 {
                    members.remove(0)
                } else {
                    surge_ts_types::union_type(members)
                };
                collect_inferred_type_argument(
                    target,
                    &member,
                    substitution,
                    widen_literals,
                    ctx,
                    depth,
                );
            }
        }
        _ => {}
    }
}

/// A union member that names a type parameter directly (`S` in `S | (() => S)`),
/// as opposed to one that gives the argument a shape to match against.
fn is_conditional_alias_reference(member: &ParsedType, ctx: &CheckerContext) -> bool {
    let ParsedType::Named(named) = member else {
        return false;
    };
    if named.type_arguments.is_empty() {
        return false;
    }
    lookup_declaration_for_inference(&named.name, ctx).is_some_and(|handle| {
        matches!(handle.get(), TypeDeclarationInfo::Alias(info)
            if matches!(info.body.ty, ParsedType::Conditional(_)))
    })
}

fn is_naked_type_parameter(member: &ParsedType, substitution: &TypeParameterSubstitution) -> bool {
    let ParsedType::Named(named) = member else {
        return false;
    };
    named.type_arguments.is_empty() && substitution.get(&named.name).is_some()
}

/// Whether an argument type could inhabit this union member at all, by shape.
/// Deliberately conservative: only the constructs that carry inference targets.
fn parsed_shape_matches(member: &ParsedType, ty: &Type) -> bool {
    match member {
        ParsedType::Function(_) => matches!(ty, Type::Function(_)),
        ParsedType::Array(_) => matches!(ty, Type::Array(_) | Type::Tuple(_)),
        ParsedType::Tuple(_) => matches!(ty, Type::Tuple(_)),
        ParsedType::Object(_) => matches!(ty, Type::Object(_)),
        // The nullish arms of an optional-value parameter
        // (`undefined | TValue | …`) claim their own argument member, so the
        // naked parameter is left standing for the value alone rather than for
        // `value | undefined`.
        ParsedType::Undefined => matches!(ty, Type::Undefined),
        _ => false,
    }
}

/// Infers type parameters that appear *inside* a generic parameter
/// (`def: SignalDefinition<TPayload>`, `w: Wrapper<T>`). When the argument is an
/// instantiation of the same declaration we match type arguments positionally;
/// when it is an object literal we resolve the declaration's members, substitute
/// the declaration's own type parameters with this position's type arguments, and
/// match member-by-member. Bounded by `depth` so mutually-generic declarations
/// cannot loop.
/// Whether an alias's body is a function type that names the alias's own type
/// parameters in a *parameter* position, the shape a callback argument infers
/// from.
fn alias_parameters_carry_type_parameters(alias: &crate::symbols::TypeAliasInfo) -> bool {
    let ParsedType::Function(body) = &alias.body.ty else {
        return false;
    };
    alias.body.type_parameters.iter().any(|type_parameter| {
        body.parameters
            .iter()
            .any(|parameter| parsed_type_mentions_name(&parameter.ty, &type_parameter.name))
    })
}

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

    // A reference that is not an instantiation (a deferred value annotation,
    // `let VariableDeclaration: Type<VariableDeclaration>` read off a namespace
    // object) carries no arguments to zip; what it resolves to does.
    if let Type::Reference(reference) = argument_type
        && reference.arguments.is_empty()
        && !named_type.type_arguments.is_empty()
    {
        let resolved = argument_type.peeled();
        if !matches!(&resolved, Type::Reference(_)) && !resolved.is_unknown() {
            return infer_through_generic_reference(
                named_type,
                &resolved,
                substitution,
                ctx,
                depth + 1,
            );
        }
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

    // The alias is tried before the object requirement below: its body can be any
    // shape, so `MutationFunction<TData, TVariables>` matches a function argument
    // that has no object surface at all.
    let (body, declaring_file) = {
        let Some(handle) = lookup_declaration_for_inference(&named_type.name, ctx) else {
            return;
        };
        match handle.get() {
            TypeDeclarationInfo::Interface(info) => (info.body.clone(), info.file_name.clone()),
            TypeDeclarationInfo::Alias(info) => {
                // An object argument matches an alias body of any shape, as it
                // always has. A *function* argument is admitted only where the
                // inference is the one this path exists for: a callback alias
                // whose PARAMETERS carry the type parameters
                // (`MutationFunction<TData, TVariables> = (variables:
                // TVariables, …) => …`). An alias that mentions them only in its
                // return (`TRPCLink<TRouter> = (opts) => OperationLink<TRouter>`)
                // is left alone — matching one bound tRPC's `TRouter` from a
                // link argument and collapsed `inferClientTypes<TRouter>`.
                let shape_matches = matches!(argument_type, Type::Object(_))
                    || matches!(info.body.ty, ParsedType::Conditional(_))
                    || (matches!(argument_type, Type::Union(_))
                        && matches!(info.body.ty, ParsedType::Union(_)))
                    || (matches!(argument_type, Type::Function(_))
                        && alias_parameters_carry_type_parameters(info));
                if !shape_matches {
                    return;
                }
                let file_name = info.file_name.clone();
                with_inference_scope_file(&file_name, ctx, |ctx| {
                    infer_through_generic_alias(
                        info,
                        named_type,
                        argument_type,
                        substitution,
                        ctx,
                        depth,
                    );
                });
                return;
            }
        }
    };
    let Type::Object(actual_object) = argument_type else {
        return;
    };
    // A reference may write fewer arguments than the declaration has
    // parameters (`FetchOptions<R>` for `FetchOptions<R, T = any>`); the
    // trailing defaults carry no inference and are simply left unmapped.
    if body.type_parameters.len() < named_type.type_arguments.len() {
        return;
    }

    let parameter_map: surge_ts_types::fx::FxHashMap<String, ParsedType> = body
        .type_parameters
        .iter()
        .zip(named_type.type_arguments.iter())
        .map(|(parameter, argument)| (parameter.name.clone(), argument.clone()))
        .collect();

    // A member's annotation names what the *declaring* file has in scope
    // (`MutationFunction`, which the consumer never imported), so the walk
    // records that file for the declaration lookups inside it.
    with_inference_scope_file(&declaring_file, ctx, |ctx| {
        for member in &body.members {
            let Some(actual_member_type) = actual_object.get_property_access_type(&member.name)
            else {
                continue;
            };
            let expected_member_type =
                substitute_parsed_type_parameters(&member.ty, &parameter_map);
            collect_inferred_type_argument(
                &expected_member_type,
                &actual_member_type,
                substitution,
                true,
                ctx,
                depth + 1,
            );
        }

        // An options interface usually declares almost nothing itself:
        // `MutationObserverOptions<TData, TError, TVariables, TOnMutateResult>`
        // extends `MutationOptions<…>`, which is where `mutationFn` lives — the
        // member the call infers `TVariables` from. The base's arguments are
        // written in terms of this declaration's parameters, so they carry the
        // same map.
        for base in &body.extends {
            let base_arguments: Vec<ParsedType> = base
                .type_arguments
                .iter()
                .map(|argument| substitute_parsed_type_parameters(argument, &parameter_map))
                .collect();
            if base_arguments.is_empty() {
                continue;
            }
            let base = ParsedNamedType {
                name: base.name.clone(),
                span: base.span,
                type_arguments: base_arguments,
            };
            infer_through_generic_reference(&base, argument_type, substitution, ctx, depth + 1);
        }
    });
}

thread_local! {
    /// Files whose declarations the current inference walk is reading members
    /// from, innermost last. Consulted only to *find* a declaration a member's
    /// annotation names; the context keeps pointing at the file being checked,
    /// so nothing resolved during the walk is keyed on a borrowed scope — moving
    /// the context itself drifted tRPC's `inferClientTypes<T>['errorShape']` to
    /// `never`.
    static INFERENCE_SCOPE_FILES: std::cell::RefCell<Vec<std::sync::Arc<str>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

fn with_inference_scope_file<R>(
    file: &std::sync::Arc<str>,
    ctx: &mut CheckerContext,
    resolve: impl FnOnce(&mut CheckerContext) -> R,
) -> R {
    INFERENCE_SCOPE_FILES.with(|files| files.borrow_mut().push(file.clone()));
    let resolved = resolve(ctx);
    INFERENCE_SCOPE_FILES.with(|files| {
        files.borrow_mut().pop();
    });
    resolved
}

fn lookup_declaration_for_inference(
    name: &str,
    ctx: &CheckerContext,
) -> Option<crate::symbols::TypeDeclarationHandle> {
    if let Some(handle) = ctx.lookup_type_declaration_handle(name) {
        return Some(handle);
    }
    INFERENCE_SCOPE_FILES.with(|files| {
        files
            .borrow()
            .iter()
            .rev()
            .find_map(|file| ctx.lookup_type_declaration_handle_in_file(name, file))
    })
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
    if alias.body.type_parameters.len() < named_type.type_arguments.len() {
        return;
    }
    let parameter_map: surge_ts_types::fx::FxHashMap<String, ParsedType> = alias
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
        if check_is_own_parameter {
            body = conditional.true_type.as_ref();
        } else {
            // The branch is not decided by the argument, so the argument infers
            // into both, as tsc does (`DefaultValue<In, $Output> = Unset extends
            // In ? $Output : In` binds `$Output` from a resolver's return).
            for branch in [&conditional.true_type, &conditional.false_type] {
                let substituted =
                    crate::infer::substitute_parsed_type_parameters_deep(branch, &parameter_map);
                collect_inferred_type_argument(
                    &substituted,
                    argument_type,
                    substitution,
                    true,
                    ctx,
                    depth + 1,
                );
            }
            return;
        }
    }

    let substituted = crate::infer::substitute_parsed_type_parameters_deep(body, &parameter_map);
    collect_inferred_type_argument(
        &substituted,
        argument_type,
        substitution,
        true,
        ctx,
        depth + 1,
    );
}

/// Substitutes bare named references in a parsed type using `map`, recursing into
/// generic arguments and array elements. Used to rewrite a declaration's member
/// types from the declaration's own type parameters into the enclosing call's
/// type parameters before inference.
fn substitute_parsed_type_parameters(
    parsed_type: &ParsedType,
    map: &surge_ts_types::fx::FxHashMap<String, ParsedType>,
) -> ParsedType {
    match parsed_type {
        ParsedType::Named(named) => {
            if named.type_arguments.is_empty() {
                if let Some(replacement) = map.get(&named.name) {
                    return replacement.clone();
                }
                ParsedType::Named(named.clone())
            } else {
                ParsedType::Named(std::sync::Arc::new(ParsedNamedType {
                    name: named.name.clone(),
                    span: named.span,
                    type_arguments: named
                        .type_arguments
                        .iter()
                        .map(|argument| substitute_parsed_type_parameters(argument, map))
                        .collect(),
                }))
            }
        }
        ParsedType::Array(element) => ParsedType::Array(std::sync::Arc::new(
            substitute_parsed_type_parameters(element, map),
        )),
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
    // Binding a type parameter to a shape carrying the degradation sentinel
    // re-types every parameter of the signature with it, so a candidate surge
    // could not fully infer is no candidate at all. A placeholder is not that: a
    // generic function passed as an argument carries its own type parameters.
    if type_argument_is_unresolved(argument_type) {
        return;
    }
    let Some(existing) = substitution.get(type_parameter_name).cloned() else {
        return;
    };

    record_generic_call_inference_candidate();
    let candidate = if widen_literals && !substitution.keeps_literal(type_parameter_name) {
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
        return;
    }

    // Two object candidates for one parameter: tsc infers the union of them,
    // so `eq({ a: 1 }, { a: 2 })` binds `T` to both shapes and the second
    // argument is not checked against the first. Dropping the later candidate
    // — the previous behavior — fixed `T` from the first argument alone.
    if matches!(existing.peeled(), Type::Object(_)) && matches!(candidate.peeled(), Type::Object(_))
    {
        substitution.set(
            type_parameter_name.to_string(),
            surge_ts_types::union_type(vec![existing, candidate]),
            false,
        );
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
                            method: property.method,
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
