use surge_ts_syntax::{ParsedExpression, ParsedLogicalOperator, ParsedUnaryOperator};
use surge_ts_types::{Type, TypeCopyReason, with_type_copy_reason};

use crate::context::CheckerContext;
use crate::symbols::{ScopeStack, SymbolInfo, SymbolTable};
use super::guards::*;
use super::{
    ReferenceGuard, narrow_instanceof_heritage_symbol_table, narrow_reference_in_scope,
    narrowed_reference_type, reference_path,
};

/// Binds a generic predicate's type parameters for the guard site. A
/// non-generic predicate needs none.
pub(super) fn predicate_type_argument_substitution(
    guard: &PredicateGuardInfo,
    subject_ty: Option<&Type>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<crate::infer::TypeParameterSubstitution> {
    let mut substitution = crate::infer::TypeParameterSubstitution::new();
    if guard.signature.type_parameters.is_empty() {
        return Some(substitution);
    }
    // A property path means the tested value is not the whole argument, so the
    // parameter annotation cannot be matched against the subject's type.
    if !guard.path.is_empty() {
        return None;
    }
    let subject_ty = subject_ty?;
    let parameter_type = guard
        .signature
        .parameter_types
        .get(guard.parameter_index)?
        .as_ref()?;
    for type_parameter in &guard.signature.type_parameters {
        substitution.insert_placeholder(
            type_parameter.name.clone(),
            Type::type_parameter(&type_parameter.name),
        );
    }
    crate::checks::call::collect_inferred_type_argument(
        parameter_type,
        subject_ty,
        &mut substitution,
        false,
        ctx,
        0,
    );

    // Only the tested argument used to contribute, so a predicate that states
    // its target in terms of *another* argument (`isMatching(pattern, value)` is
    // `value is T & …narrow<T, P>…`, and `P` is the pattern) left that parameter
    // unbound and fell back to `Any` below — which then proves nothing and the
    // guard gives up. Each argument is inferred against its own declared
    // parameter, and only while something is still unbound.
    for (position, argument) in &guard.other_arguments {
        if !guard
            .signature
            .type_parameters
            .iter()
            .any(|type_parameter| substitution.is_placeholder(&type_parameter.name))
        {
            break;
        }
        let Some(Some(parameter_type)) = guard.signature.parameter_types.get(*position) else {
            continue;
        };
        let diagnostics_before = ctx.diagnostics().len();
        let inferred = crate::infer::infer_expression(argument, symbols, ctx);
        ctx.truncate_diagnostics(diagnostics_before);
        let crate::infer::InferredExpression::Known(argument_ty) = inferred else {
            continue;
        };
        if argument_ty.is_unknown() {
            continue;
        }
        crate::checks::call::collect_inferred_type_argument(
            parameter_type,
            &argument_ty,
            &mut substitution,
            false,
            ctx,
            0,
        );
    }

    // A parameter written as an *alias* whose expansion is a union
    // (`x: SyncParseReturnType<T>` = `OK<T> | DIRTY<T> | INVALID`) leaves nothing
    // to align the alias's argument against, so `T` stays unbound. The predicate
    // is still usable: narrowing only ever *filters* the subject's own union
    // members, so binding the leftovers to `Any` selects the right member and
    // the members that survive keep their own precision.
    let unresolved: Vec<&surge_ts_syntax::ParsedTypeParameter> = guard
        .signature
        .type_parameters
        .iter()
        .filter(|type_parameter| substitution.is_placeholder(&type_parameter.name))
        .collect();
    if !unresolved.is_empty() {
        let mut filled = substitution.clone_with_reason(TypeCopyReason::ScopeOrContext);
        for type_parameter in unresolved {
            filled.insert(type_parameter.name.clone(), Type::Any);
        }
        return predicate_filters_subject_union(guard, &filled, subject_ty, ctx)
            .then_some(filled);
    }
    Some(substitution)
}

/// Whether an `Any`-filled predicate would narrow `subject_ty` by *selecting*
/// among its union members rather than by replacing it wholesale. Only the
/// selecting outcome is sound when the predicate's type arguments are unknown:
/// [`super::guards::narrow_by_predicate`] substitutes the predicate itself when
/// no member matches or the subject is not a union, which would install the
/// `Any` fillers as real types.
pub(super) fn predicate_filters_subject_union(
    guard: &PredicateGuardInfo,
    substitution: &crate::infer::TypeParameterSubstitution,
    subject_ty: &Type,
    ctx: &mut CheckerContext,
) -> bool {
    let Type::Union(union) = subject_ty.peeled() else {
        return false;
    };
    let Some(predicate_ty) = resolve_predicate_type_in_declaring_scope(guard, substitution, ctx)
    else {
        return false;
    };
    let members = union.types();
    let matching = members
        .iter()
        .filter(|member| surge_ts_types::is_assignable_to(member, &predicate_ty))
        .count();
    matching > 0 && matching < members.len()
}

/// The target type of a value that *is* a type predicate (`isFoo` passed to
/// `filter`), resolved in the signature's declaring scope. `None` when the value
/// carries no collected signature, its return is not a predicate, or the
/// predicate is generic — a `T` there needs a guard site to infer against.
pub(crate) fn predicate_target_of_value(
    name: &str,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let signature = symbols.get(name)?.function_signature.clone()?;
    if !signature.type_parameters.is_empty() {
        return None;
    }
    let surge_ts_syntax::ParsedType::Predicate(predicate) = signature.return_type.as_ref()? else {
        return None;
    };
    if predicate.asserts {
        return None;
    }
    let guard = PredicateGuardInfo {
        subject: String::new(),
        path: Vec::new(),
        other_arguments: Vec::new(),
        predicate_type: predicate.ty.clone()?,
        declaring_file: signature.declaring_file.clone(),
        namespace_prefix: signature.namespace_prefix.clone(),
        parameter_index: 0,
        signature,
    };
    resolve_predicate_type_in_declaring_scope(&guard, &crate::infer::TypeParameterSubstitution::new(), ctx)
}

/// Resolves a predicate guard's target type under the predicate's declaring
/// file (see [`crate::symbols::FunctionSignatureInfo::declaring_file`]). A
/// resolution that degrades (`had_error` or the `Unknown` sentinel) proves
/// nothing — narrowing on it would manufacture facts from a modeling gap — so
/// it yields `None`. A generic predicate needs its `T` bound first, inferred
/// from `subject_ty`; without a subject type, or when a type parameter stays
/// unbound and the filled predicate would not merely select among the subject's
/// union members, the guard is dropped.
pub(super) fn resolve_predicate_guard_type(
    guard: &PredicateGuardInfo,
    subject_ty: Option<&Type>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let substitution = predicate_type_argument_substitution(guard, subject_ty, symbols, ctx)?;
    resolve_predicate_type_in_declaring_scope(guard, &substitution, ctx)
}

/// Resolves `guard.predicate_type` under the signature's declaring file and
/// namespace prefix, with `substitution` bound for its type parameters.
pub(super) fn resolve_predicate_type_in_declaring_scope(
    guard: &PredicateGuardInfo,
    substitution: &crate::infer::TypeParameterSubstitution,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    resolve_predicate_type_under(
        guard.predicate_type.clone(),
        guard.declaring_file.as_deref(),
        guard.namespace_prefix.as_deref(),
        substitution,
        ctx,
    )
}

/// Resolves a predicate's written target type under the file (and namespace)
/// that declared it. A resolution that degrades proves nothing, so it is
/// dropped rather than narrowed to.
pub(super) fn resolve_predicate_type_under(
    predicate_type: surge_ts_syntax::ParsedType,
    declaring_file: Option<&str>,
    namespace_prefix: Option<&str>,
    substitution: &crate::infer::TypeParameterSubstitution,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let declaring_file = declaring_file
        .filter(|file| *file != ctx.file_name)
        .map(str::to_string);
    let saved_file_name = declaring_file.map(|file| {
        let saved = ctx.file_name.clone();
        ctx.set_file_name(file);
        saved
    });
    if let Some(prefix) = namespace_prefix {
        ctx.namespace_member_resolution_depth += 1;
        ctx.namespace_member_prefix_stack.push(prefix.to_string());
    }
    // The predicate's target names whatever is in scope where the predicate was
    // *declared*, which includes an enclosing generic's type parameters
    // (`function isTarget(n: any): n is TFunc` inside `outer<TFunc>`). Those live
    // on the scope stack, not in the guard's own substitution, so resolving
    // against the substitution alone reported the enclosing parameter as an
    // unresolved name — at the declaration's span, long after the declaration
    // itself checked clean.
    let substitution = crate::infer::types::merged_type_parameter_substitution(ctx, substitution);
    let resolved = with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
        crate::infer::types::resolve_parsed_type(predicate_type, ctx, &mut Vec::new(), &substitution)
    });
    if namespace_prefix.is_some() {
        ctx.namespace_member_prefix_stack.pop();
        ctx.namespace_member_resolution_depth -= 1;
    }
    if let Some(saved) = saved_file_name {
        ctx.set_file_name(saved);
    }
    if resolved.had_error() {
        return None;
    }
    let ty = resolved.into_ty();
    (!matches!(ty, Type::Unknown | Type::TypeParameter(_))).then_some(ty)
}

/// A zero-argument method call (`type.isUnion()`) plus the reference it is
/// called on, the shape a `this is T` predicate guards.
pub(super) fn parse_this_predicate_call(
    condition: &ParsedExpression,
) -> Option<(String, Vec<String>, &str)> {
    let ParsedExpression::PropertyCall {
        object,
        property_name,
        type_arguments,
        arguments,
        ..
    } = condition
    else {
        return None;
    };
    if !type_arguments.is_empty() || !arguments.is_empty() {
        return None;
    }
    let (subject, path) = reference_path(object)?;
    Some((subject, path, property_name.as_str()))
}

/// The resolved target of a `this is T` method predicate declared on the
/// receiver's own interface (`interface Type { isUnion(): this is UnionType }`).
///
/// Predicates are dropped when a signature resolves to a type — `x is T` and
/// `this is T` both resolve to `boolean` — so the declaration has to be read
/// back. Only the receiver's own declaration is consulted: an inherited
/// predicate would need the heritage chain, which is not what the shapes this
/// models (`ts.Type`'s `isUnion`/`isIntersection`) declare.
pub(super) fn this_predicate_target(
    receiver_ty: &Type,
    method: &str,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let Type::Reference(reference) = receiver_ty else {
        return None;
    };
    let id = reference.id.as_ref();
    let separator = id.rfind('\u{0}')?;
    let (declaring_file, declaration_name) = (&id[..separator], &id[separator + 1..]);
    let declaration_name = declaration_name.to_string();

    let found = {
        let crate::symbols::TypeDeclarationInfo::Interface(info) =
            ctx.lookup_type_declaration(&declaration_name)?
        else {
            return None;
        };
        if info.file_name.as_ref() != declaring_file {
            return None;
        }
        let member = info.body.members.iter().find(|member| member.name == method)?;
        let surge_ts_syntax::ParsedType::Function(function) = &member.ty else {
            return None;
        };
        let surge_ts_syntax::ParsedType::Predicate(predicate) = function.return_type.as_ref()
        else {
            return None;
        };
        if predicate.asserts || predicate.parameter_name != "this" {
            return None;
        }
        (predicate.ty.clone()?, info.file_name.to_string())
    };
    let (predicate_type, declaring_file) = found;
    // A predicate written inside `declare namespace ts` names `ts.UnionType`,
    // which only resolves under that prefix.
    let namespace_prefix = declaration_name
        .rfind('.')
        .map(|dot| declaration_name[..dot].to_string());
    resolve_predicate_type_under(
        predicate_type,
        Some(declaring_file.as_str()),
        namespace_prefix.as_deref(),
        &crate::infer::TypeParameterSubstitution::new(),
        ctx,
    )
}

/// Narrows `symbols` by a `this is T` method predicate, or `None` when the
/// condition is not one (or it proves nothing new).
pub(super) fn narrow_this_predicate_symbol_table(
    condition: &ParsedExpression,
    symbols: &SymbolTable,
    branch_is_true: bool,
    ctx: &mut CheckerContext,
) -> Option<SymbolTable> {
    let (subject, path, method) = parse_this_predicate_call(condition)?;
    if !path.is_empty() {
        return None;
    }
    let symbol = symbols.get(&subject)?;
    let subject_ty = symbol.ty.clone();
    let kind = symbol.kind;
    let function_signature = symbol.function_signature.clone();
    let target = this_predicate_target(&subject_ty, method, ctx)?;
    let narrowed = with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
        narrow_by_predicate(&subject_ty, &target, branch_is_true)
    })?;
    let mut narrowed_symbols = symbols.clone_with_reason(TypeCopyReason::ScopeOrContext);
    narrowed_symbols.insert_narrowed(
        subject,
        SymbolInfo {
            ty: narrowed,
            kind,
            function_signature,
        },
        subject_ty,
    );
    Some(narrowed_symbols)
}

/// The `ScopeStack` counterpart of [`narrow_this_predicate_symbol_table`].
pub(super) fn narrow_this_predicate_call_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
    ctx: &mut CheckerContext,
) -> bool {
    let Some((subject, path, method)) = parse_this_predicate_call(condition) else {
        return false;
    };
    if !path.is_empty() {
        return false;
    }
    let Some(symbol) = scopes.resolve(&subject) else {
        return false;
    };
    let subject_ty = symbol.ty.clone();
    let kind = symbol.kind;
    let function_signature = symbol.function_signature.clone();
    let Some(target) = this_predicate_target(&subject_ty, method, ctx) else {
        return false;
    };
    let Some(narrowed) = with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
        narrow_by_predicate(&subject_ty, &target, branch_is_true)
    }) else {
        return true;
    };
    let _ = scopes.insert_current_narrowed(
        subject,
        SymbolInfo {
            ty: narrowed,
            kind,
            function_signature,
        },
        subject_ty,
    );
    true
}

/// Applies user-defined type-predicate narrowing (`isFoo(x)`) in place to a
/// `ScopeStack`. Returns whether the condition was such a predicate call over a
/// bare-identifier argument.
pub(super) fn narrow_predicate_call_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
    ctx: &mut CheckerContext,
) -> bool {
    let Some(guard) = parse_type_predicate_condition(condition, &mut |name| {
        scopes
            .resolve(name)
            .and_then(|symbol| symbol.function_signature.clone())
    }) else {
        return false;
    };
    let Some(symbol) = scopes.resolve(&guard.subject) else {
        return true;
    };
    let subject_ty = symbol.ty.clone();
    let kind = symbol.kind;
    let function_signature = symbol.function_signature.clone();
    let Some(predicate_ty) =
        resolve_predicate_guard_type(&guard, Some(&subject_ty), scopes.visible_symbols(), ctx)
    else {
        return true;
    };
    let Some(narrowed) = with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
        narrowed_predicate_subject(&subject_ty, &guard.path, &predicate_ty, branch_is_true)
    }) else {
        return true;
    };
    let _ = scopes.insert_current_narrowed(
        guard.subject,
        SymbolInfo {
            ty: narrowed,
            kind,
            function_signature,
        },
        subject_ty,
    );
    true
}

/// Applies an assertion call's narrowing (`assertIsObject(obj);`) to the
/// enclosing scope. Unlike a `x is T` predicate, an `asserts x is T` signature
/// narrows from the *statement* onward rather than inside a branch, so it is
/// applied where the expression statement is checked.
pub(crate) fn narrow_assertion_call_in_scope(
    expression: &ParsedExpression,
    scopes: &mut ScopeStack,
    ctx: &mut CheckerContext,
) {
    let ParsedExpression::Call {
        callee_name,
        type_arguments,
        arguments,
        ..
    } = expression
    else {
        return;
    };
    if !type_arguments.is_empty() {
        return;
    }
    let Some(signature) = scopes
        .resolve(callee_name)
        .and_then(|symbol| symbol.function_signature.clone())
    else {
        return;
    };
    // A generic assertion needs its `T` bound at the call site, which this path
    // has no inference for; leaving it alone keeps the declared type.
    if !signature.type_parameters.is_empty() {
        return;
    }
    let Some(surge_ts_syntax::ParsedType::Predicate(predicate)) = signature.return_type.as_ref()
    else {
        return;
    };
    if !predicate.asserts || predicate.parameter_name == "this" {
        return;
    }
    let Some(index) = signature
        .parameter_names
        .iter()
        .position(|name| name.as_deref() == Some(predicate.parameter_name.as_str()))
    else {
        return;
    };
    let Some(argument) = arguments.get(index) else {
        return;
    };
    let Some((subject, path)) = reference_path(&argument.expression) else {
        return;
    };
    let Some(symbol) = scopes.resolve(&subject) else {
        return;
    };
    let subject_ty = symbol.ty.clone();
    let kind = symbol.kind;
    let function_signature = symbol.function_signature.clone();

    // `asserts x` with no target proves only that `x` is truthy.
    let Some(predicate_type) = predicate.ty.clone() else {
        if path.is_empty() {
            let narrowed = surge_ts_types::remove_nullish(&subject_ty);
            if narrowed != subject_ty {
                let _ = scopes.insert_current_narrowed(
                    subject,
                    SymbolInfo {
                        ty: narrowed,
                        kind,
                        function_signature,
                    },
                    subject_ty,
                );
            }
        }
        return;
    };
    let Some(target) = resolve_predicate_type_under(
        predicate_type,
        signature.declaring_file.as_deref(),
        signature.namespace_prefix.as_deref(),
        &crate::infer::TypeParameterSubstitution::new(),
        ctx,
    ) else {
        return;
    };
    let Some(narrowed) = with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
        narrowed_predicate_subject(&subject_ty, &path, &target, true)
    }) else {
        return;
    };
    let _ = scopes.insert_current_narrowed(
        subject,
        SymbolInfo {
            ty: narrowed,
            kind,
            function_signature,
        },
        subject_ty,
    );
}

/// Collects the subjects of user-defined predicate guard calls (`isFoo(x)` → `x`)
/// within a (possibly `||`/`&&`/`!`-composed) condition. Kept separate from
/// [`collect_guard_operand_identifiers`]: recognizing a predicate call needs the
/// callee's collected signature, and a non-predicate call must not count as a
/// guard (tsc does not narrow `if (foo(x))`).
pub(super) fn collect_predicate_guard_subjects(
    condition: &ParsedExpression,
    scopes: &ScopeStack,
    names: &mut Vec<String>,
) {
    match condition {
        ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Not,
            operand,
            ..
        } => collect_predicate_guard_subjects(operand, scopes, names),
        ParsedExpression::Logical {
            left,
            operator: ParsedLogicalOperator::Or | ParsedLogicalOperator::And,
            right,
            ..
        } => {
            collect_predicate_guard_subjects(left, scopes, names);
            collect_predicate_guard_subjects(right, scopes, names);
        }
        _ => {
            if let Some(guard) = parse_type_predicate_condition(condition, &mut |name| {
                scopes
                    .resolve(name)
                    .and_then(|symbol| symbol.function_signature.clone())
            }) && !names.iter().any(|existing| existing == &guard.subject)
            {
                names.push(guard.subject);
            }
        }
    }
}

/// Narrows `symbols` by everything the truthy operand of an `&&` proves: a
/// structured guard (`x.kind === "k" && …`) plus each identifier/property in the
/// `&&` chain narrowed to non-nullish (`a.b && a.b > c`). Returns `None` when
/// nothing narrows. Used to type the right operand of `&&`.
/// Narrows the guarded reference: the value itself for a bare identifier, or the
/// property the guard tested (`ts.isStringLiteral(node.moduleSpecifier)`).
pub(super) fn narrowed_predicate_subject(
    subject_ty: &Type,
    path: &[String],
    predicate_ty: &Type,
    branch_is_true: bool,
) -> Option<Type> {
    if path.is_empty() {
        return narrow_by_predicate(subject_ty, predicate_ty, branch_is_true);
    }
    narrowed_reference_type(
        subject_ty,
        path,
        ReferenceGuard::Predicate {
            target: predicate_ty,
            keep_matching: branch_is_true,
        },
    )
}

/// Applies user-defined type-predicate narrowing (`isFoo(x)`,
/// `ts.isImportDeclaration(node)`) to a plain symbol table, for the expression
/// paths that have no `ScopeStack` — the right operand of `&&` and a ternary's
/// branches. Kept apart from [`narrow_condition_symbol_table`] because
/// resolving the predicate's target type needs the checker context, which those
/// syntactic guards do not take.
pub(crate) fn narrow_predicate_guards_symbol_table(
    condition: &ParsedExpression,
    symbols: &SymbolTable,
    branch_is_true: bool,
    ctx: &mut CheckerContext,
) -> Option<SymbolTable> {
    if let ParsedExpression::Unary {
        operator: ParsedUnaryOperator::Not,
        operand,
        ..
    } = condition
    {
        return narrow_predicate_guards_symbol_table(operand, symbols, !branch_is_true, ctx);
    }

    // Every operand of an `&&` holds in its true branch, and — by De Morgan —
    // every operand of an `||` fails in its false branch, so either chain
    // narrows by all of its operands. The other branch proves nothing about any
    // single operand.
    if let ParsedExpression::Logical {
        left,
        operator,
        right,
        ..
    } = condition
        && matches!(
            (operator, branch_is_true),
            (ParsedLogicalOperator::And, true) | (ParsedLogicalOperator::Or, false)
        )
    {
        let left_narrowed =
            narrow_predicate_guards_symbol_table(left, symbols, branch_is_true, ctx);
        let right_narrowed = narrow_predicate_guards_symbol_table(
            right,
            left_narrowed.as_ref().unwrap_or(symbols),
            branch_is_true,
            ctx,
        );
        return right_narrowed.or(left_narrowed);
    }

    if let Some(narrowed) =
        narrow_this_predicate_symbol_table(condition, symbols, branch_is_true, ctx)
    {
        return Some(narrowed);
    }

    if let Some(narrowed) =
        narrow_instanceof_heritage_symbol_table(condition, symbols, branch_is_true, ctx)
    {
        return Some(narrowed);
    }

    let guard = parse_type_predicate_condition(condition, &mut |name| {
        symbols
            .get(name)
            .and_then(|symbol| symbol.function_signature.clone())
    })?;
    let symbol = symbols.get(&guard.subject)?;
    let subject_ty = symbol.ty.clone();
    let kind = symbol.kind;
    let function_signature = symbol.function_signature.clone();
    let predicate_ty = resolve_predicate_guard_type(&guard, Some(&subject_ty), symbols, ctx)?;
    let narrowed = with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
        narrowed_predicate_subject(&subject_ty, &guard.path, &predicate_ty, branch_is_true)
    })?;
    if narrowed == subject_ty {
        return None;
    }
    let mut narrowed_symbols = symbols.clone_with_reason(TypeCopyReason::ScopeOrContext);
    narrowed_symbols.insert_narrowed(
        guard.subject,
        SymbolInfo {
            ty: narrowed,
            kind,
            function_signature,
        },
        subject_ty,
    );
    Some(narrowed_symbols)
}

/// Applies every property-path type-predicate guard the condition proves. Walks
/// `!` and the operands of a true-branch `&&`, the shapes that compose guards.
pub(super) fn narrow_predicate_reference_guards_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
    ctx: &mut CheckerContext,
) {
    if let ParsedExpression::Unary {
        operator: ParsedUnaryOperator::Not,
        operand,
        ..
    } = condition
    {
        narrow_predicate_reference_guards_in_scope(operand, scopes, !branch_is_true, ctx);
        return;
    }
    if let Some((inner, flip)) = strip_boolean_literal_comparison(condition) {
        narrow_predicate_reference_guards_in_scope(inner, scopes, branch_is_true != flip, ctx);
        return;
    }

    if let ParsedExpression::Logical {
        left,
        operator,
        right,
        ..
    } = condition
        && matches!(
            (operator, branch_is_true),
            (ParsedLogicalOperator::And, true) | (ParsedLogicalOperator::Or, false)
        )
    {
        narrow_predicate_reference_guards_in_scope(left, scopes, branch_is_true, ctx);
        narrow_predicate_reference_guards_in_scope(right, scopes, branch_is_true, ctx);
        return;
    }

    let Some(guard) = parse_type_predicate_condition(condition, &mut |name| {
        scopes
            .resolve(name)
            .and_then(|symbol| symbol.function_signature.clone())
    }) else {
        return;
    };
    if guard.path.is_empty() {
        return;
    }
    let Some(predicate_ty) =
        resolve_predicate_guard_type(&guard, None, scopes.visible_symbols(), ctx)
    else {
        return;
    };
    narrow_reference_in_scope(
        &guard.subject,
        &guard.path,
        ReferenceGuard::Predicate {
            target: &predicate_ty,
            keep_matching: branch_is_true,
        },
        scopes,
    );
}
