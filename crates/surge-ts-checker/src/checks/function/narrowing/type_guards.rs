use surge_ts_syntax::{ParsedExpression, ParsedLogicalOperator, ParsedUnaryOperator};
use surge_ts_types::{Type, TypeCopyReason, with_type_copy_reason};

use crate::context::CheckerContext;
use crate::symbols::{ScopeStack, SymbolInfo, SymbolTable};
use super::guards::*;
use super::{
    ReferenceGuard, narrow_predicate_reference_guards_in_scope, narrow_reference_in_scope,
    narrow_value_guards_in_scope, narrowed_reference_type, reference_path,
};

/// Applies `typeof x === "tag"` / `typeof o.p === "tag"` narrowing in place to a
/// `ScopeStack` (the if-body and early-return paths). Returns whether the
/// condition was a typeof guard over a reference.
pub(super) fn narrow_typeof_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
) -> bool {
    let Some((operand, tag, eq)) = parse_typeof_condition(condition) else {
        return false;
    };
    let Some((base, path)) = reference_path(operand) else {
        return false;
    };
    let guard = ReferenceGuard::Typeof {
        tag,
        keep_matching: branch_is_true == eq,
    };
    narrow_reference_in_scope(&base, &path, guard, scopes);
    true
}

/// Applies `x instanceof Ctor` / `o.p instanceof Ctor` narrowing in place to a
/// `ScopeStack`. Returns whether the condition was an instanceof guard over a
/// reference.
pub(super) fn narrow_instanceof_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
    ctx: &mut CheckerContext,
) -> bool {
    let Some((operand, ctor_name)) = parse_instanceof_condition(condition) else {
        return false;
    };
    let Some((base, path)) = reference_path(operand) else {
        return false;
    };
    let instance = resolve_constructor_instance_type(ctor_name, ctx);
    narrow_reference_in_scope(
        &base,
        &path,
        ReferenceGuard::Instanceof {
            ctor_name,
            instance: instance.as_ref(),
            keep_matching: branch_is_true,
        },
        scopes,
    );
    true
}

/// The instance type a constructor name denotes, for `x instanceof Ctor`. `None`
/// when the name does not resolve to a type — the guard then falls back to
/// nominal union filtering.
pub(super) fn resolve_constructor_instance_type(
    ctor_name: &str,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    // The reference is synthesized from the guard, not written in the source, so
    // it has to carry the arity the declaration expects — a bare generic name
    // would report TS2314 for arguments the guard never writes. `x instanceof
    // GSub` narrows to `GSub<any>` in tsc, which is what filling `any` gives.
    // Through the full resolution surface, not the file's own table: a lib
    // constructor (`Map`, `WeakSet`) is declared in the default lib, so reading
    // arity from the local table alone gave it zero, and `Map` then resolved
    // with no arguments and failed — leaving `x instanceof Map` narrowing
    // nothing at all in project mode.
    let arity = match ctx
        .type_declarations
        .get(ctor_name)
        .or_else(|| ctx.lookup_type_declaration(ctor_name))
    {
        Some(crate::symbols::TypeDeclarationInfo::Interface(info)) => {
            info.body.type_parameters.len()
        }
        Some(crate::symbols::TypeDeclarationInfo::Alias(info)) => info.body.type_parameters.len(),
        None => 0,
    };
    // Nothing this resolution reports belongs to the source, so drop whatever it
    // raised.
    let diagnostics_before = ctx.diagnostics().len();
    let resolved = with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
        crate::infer::types::resolve_parsed_type(
            surge_ts_syntax::ParsedType::Named(std::sync::Arc::new(
                surge_ts_syntax::ParsedNamedType {
                    name: ctor_name.to_string(),
                    span: None,
                    type_arguments: vec![surge_ts_syntax::ParsedType::Any; arity],
                },
            )),
            ctx,
            &mut Vec::new(),
            &crate::infer::TypeParameterSubstitution::new(),
        )
    });
    ctx.truncate_diagnostics(diagnostics_before);
    if resolved.had_error() {
        return None;
    }
    let instance = resolved.into_ty();
    (!instance.is_unknown()).then_some(instance)
}

/// Applies `Array.isArray(x)` / `Array.isArray(o.p)` narrowing in place to a
/// `ScopeStack`. Returns whether the condition was such a guard over a
/// reference.
pub(super) fn narrow_array_isarray_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
) -> bool {
    let Some(operand) = parse_array_isarray_condition(condition) else {
        return false;
    };
    let Some((base, path)) = reference_path(operand) else {
        return false;
    };
    narrow_reference_in_scope(
        &base,
        &path,
        ReferenceGuard::Arrayness {
            keep_arrays: branch_is_true,
        },
        scopes,
    );
    true
}

/// Applies `"prop" in x` narrowing in place to a `ScopeStack` (the if-body and
/// early-return paths). Returns whether the condition was such a test over a
/// bare identifier.
pub(super) fn narrow_property_presence_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
) -> bool {
    let Some((object, property)) = parse_in_condition(condition) else {
        return false;
    };
    let Some((base, path)) = reference_path(object) else {
        return false;
    };
    // Shadows in the current frame, not the owning frame — see the note in
    // `narrow_discriminant_in_scope`.
    narrow_reference_in_scope(
        &base,
        &path,
        ReferenceGuard::PropertyPresence {
            property,
            keep_present: branch_is_true,
        },
        scopes,
    );
    true
}

/// Applies `x === null` / `x.p === undefined` narrowing in place to a
/// `ScopeStack`, for a bare identifier or one property of one. Returns whether
/// the condition was such a test (handled either way, so the discriminant parse
/// downstream is skipped — `null` is not a discriminant literal).
pub(super) fn narrow_nullish_equality_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
) -> bool {
    let Some((subject, eq)) = parse_nullish_equality_condition(condition) else {
        return false;
    };
    let Some((base, path)) = reference_path(subject) else {
        return false;
    };
    narrow_reference_in_scope(
        &base,
        &path,
        ReferenceGuard::Nullish {
            keep_matching: branch_is_true == eq,
        },
        scopes,
    );
    true
}

/// Applies `x === "lit"` / `x !== 3` narrowing in place to a `ScopeStack`.
/// Composite conditions reach the same leaf through
/// [`narrow_single_guard_for_identifier`]; this is the bare form, which never
/// enters that path. Returns whether the subject narrowed.
pub(super) fn narrow_literal_equality_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
) -> bool {
    let Some((name, literal, eq)) = parse_identifier_literal_equality(condition) else {
        return false;
    };
    let Some(symbol) = scopes.resolve(name) else {
        return false;
    };
    let declared = symbol.ty.clone();
    let kind = symbol.kind;
    let function_signature = symbol.function_signature.clone();
    let Some(narrowed) = narrow_by_literal_equality(&declared, &literal, branch_is_true == eq)
    else {
        return false;
    };
    let _ = scopes.insert_current_narrowed(
        name.to_string(),
        SymbolInfo {
            ty: narrowed,
            kind,
            function_signature,
        },
        declared,
    );
    true
}

/// Applies `ArrayBuffer.isView(x)` / `ArrayBuffer.isView(o.p)` narrowing in place
/// to a `ScopeStack`. Returns whether the condition was such a guard over a
/// reference.
pub(super) fn narrow_arraybuffer_isview_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
) -> bool {
    let Some(operand) = parse_arraybuffer_isview_condition(condition) else {
        return false;
    };
    let Some((base, path)) = reference_path(operand) else {
        return false;
    };
    narrow_reference_in_scope(
        &base,
        &path,
        ReferenceGuard::ArrayBufferView {
            keep_views: branch_is_true,
        },
        scopes,
    );
    true
}

/// `x instanceof Ctor` / `o.p instanceof Ctor` on a plain symbol table. Split
/// from the syntactic guards in [`narrow_condition_symbol_table`] for the same
/// reason the predicates are: deciding whether a union member *derives from*
/// the constructor needs the constructor's instance type, and resolving that
/// needs the checker context. The name-only test those guards run leaves
/// `TRPCClientError | Envelope` untouched under `instanceof Error`, so the
/// false branch of `props.result instanceof Error || …` still read the error
/// arm.
pub(super) fn narrow_instanceof_heritage_symbol_table(
    condition: &ParsedExpression,
    symbols: &SymbolTable,
    branch_is_true: bool,
    ctx: &mut CheckerContext,
) -> Option<SymbolTable> {
    let (operand, ctor_name) = parse_instanceof_condition(condition)?;
    let (base, path) = reference_path(operand)?;
    let symbol = symbols.get(&base)?;
    let declared = symbol.ty.clone();
    let kind = symbol.kind;
    let function_signature = symbol.function_signature.clone();

    let instance = resolve_constructor_instance_type(ctor_name, ctx)?;
    let narrowed = with_type_copy_reason(TypeCopyReason::ScopeOrContext, || {
        if path.is_empty() {
            // A non-union subject narrows down its subtype edge instead, the
            // same fallback the scope-based single-guard path takes: without it
            // only an `if` narrowed `valueA instanceof Map`, and the same test
            // written as the condition of a `?:` left the subject alone.
            narrow_union_by_instanceof(&declared, ctor_name, Some(&instance), branch_is_true)
                .or_else(|| {
                    narrow_to_instanceof_subclass(&declared, Some(&instance), branch_is_true)
                })
        } else {
            narrowed_reference_type(
                &declared,
                &path,
                ReferenceGuard::Instanceof {
                    ctor_name,
                    instance: Some(&instance),
                    keep_matching: branch_is_true,
                },
            )
        }
    })?;
    if narrowed == declared {
        return None;
    }

    let mut narrowed_symbols = symbols.clone_with_reason(TypeCopyReason::ScopeOrContext);
    narrowed_symbols.insert_narrowed(
        base,
        SymbolInfo {
            ty: narrowed,
            kind,
            function_signature,
        },
        declared,
    );
    Some(narrowed_symbols)
}

/// Narrows `ty` for `var_name` under a single (non-composite) guard condition.
/// `x instanceof Sub` on a subject that is not a union narrows it *down* to the
/// constructor's instance type when that type is a subtype of the subject's
/// (`declare const s: Base; if (s instanceof Sub) s.onlyOnSub()`). Union subjects
/// are handled by member filtering in `narrow_union_by_instanceof`; this is the
/// single-type case, which filtering cannot express.
///
/// Only the matching branch narrows: the `else` branch of `s instanceof Sub`
/// keeps `Base`, since every non-`Sub` `Base` still inhabits it.
pub(super) fn narrow_to_instanceof_subclass(
    ty: &Type,
    instance: Option<&Type>,
    keep_matching: bool,
) -> Option<Type> {
    if !keep_matching || matches!(ty.peeled(), Type::Union(_)) {
        return None;
    }
    // A subject that is already `any` or unresolved says nothing to narrow.
    if ty.is_unknown() || matches!(ty, Type::Any) {
        return None;
    }
    let instance = instance?;
    if instance.is_unknown() || instance == ty {
        return None;
    }
    // Narrow only along a real subtype edge; an unrelated constructor leaves the
    // subject alone rather than replacing it with something it never was.
    surge_ts_types::is_assignable_to(instance, ty).then(|| instance.clone())
}

/// Narrows `symbols` by a bare `x === "lit"` / `x !== 3` test.
pub(super) fn narrow_literal_equality_symbol_table(
    condition: &ParsedExpression,
    symbols: &SymbolTable,
    branch_is_true: bool,
) -> Option<SymbolTable> {
    let (name, literal, eq) = parse_identifier_literal_equality(condition)?;
    let symbol = symbols.get(name)?;
    let narrowed = narrow_by_literal_equality(&symbol.ty, &literal, branch_is_true == eq)?;
    let mut narrowed_symbols = symbols.clone_with_reason(TypeCopyReason::ScopeOrContext);
    narrowed_symbols.insert_narrowed(
        name.to_string(),
        SymbolInfo {
            ty: narrowed,
            kind: symbol.kind,
            function_signature: symbol.function_signature.clone(),
        },
        symbol.ty.clone(),
    );
    Some(narrowed_symbols)
}

/// Collects the identifiers tested by equality guards — a discriminant test
/// (`x.kind === "a"` → `x`) or a nullish test (`x === null`) — within a
/// (possibly `||`/`&&`/`!`-composed) condition. Kept out of
/// [`collect_guard_operand_identifiers`], whose results also drive the
/// genuine-`unknown` downgrade — testing a *property* proves nothing about the
/// whole value there.
pub(super) fn collect_equality_guard_subjects(
    condition: &ParsedExpression,
    scopes: &ScopeStack,
    names: &mut Vec<String>,
) {
    match condition {
        ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Not,
            operand,
            ..
        } => collect_equality_guard_subjects(operand, scopes, names),
        ParsedExpression::Logical {
            left,
            operator: ParsedLogicalOperator::Or | ParsedLogicalOperator::And,
            right,
            ..
        } => {
            collect_equality_guard_subjects(left, scopes, names);
            collect_equality_guard_subjects(right, scopes, names);
        }
        _ => {
            // The compared value may be a `const` (`x?.version === CACHE_VERSION`);
            // it resolves through the visible table like the single-guard path.
            let subject = match parse_discriminant_condition_with(condition, &|expression| {
                const_member_literal_value(expression, scopes.visible_symbols())
            }) {
                Some((ParsedExpression::Identifier { name, .. }, _, _, _)) => Some(name.as_str()),
                _ => match parse_nullish_equality_condition(condition) {
                    Some((ParsedExpression::Identifier { name, .. }, _)) => Some(name.as_str()),
                    _ => parse_identifier_literal_equality(condition).map(|(name, _, _)| name),
                },
            };
            if let Some(name) = subject
                && !names.iter().any(|existing| existing == name)
            {
                names.push(name.to_string());
            }
        }
    }
}

/// Like [`narrow_discriminant_symbol_table`] but applies the narrowing in place
/// to a `ScopeStack`, for narrowing a discriminated union inside an `if` branch
/// (or after an early-returning `if`).
pub(crate) fn narrow_discriminant_in_scope(
    condition: &ParsedExpression,
    scopes: &mut ScopeStack,
    branch_is_true: bool,
    ctx: &mut CheckerContext,
) {
    narrow_value_guards_in_scope(condition, scopes, branch_is_true, ctx);
    // A predicate over a *property* (`ts.isStringLiteral(node.moduleSpecifier)`)
    // narrows that path, which the value-guard dispatch above does not reach —
    // and it has to run after it, since the property only exists once the base
    // itself is narrowed (`isImportDeclaration(node) && isStringLiteral(node.m)`).
    narrow_predicate_reference_guards_in_scope(condition, scopes, branch_is_true, ctx);
}
