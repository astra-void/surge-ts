use super::*;
use crate::symbols::SymbolInfo;
use surge_ts_syntax::{ParsedLogicalOperator, ParsedType, ParsedUnaryOperator};

/// Drops every binding a condition guards from [`Type::GenuineUnknown`] to
/// [`Type::Unknown`] in a copy of `symbols`, returning `None` when nothing
/// changes.
///
/// tsc narrows a guarded `unknown` value, so an access in the branch where the
/// guard holds is not `TS18046`. surge does not compute the narrowed shape for
/// `unknown`, but it must stop treating the value as a genuine-unknown
/// receiver — the same downgrade
/// [`crate::checks::function::narrow_truthy_operand_symbol_table`] performs for
/// `typeof`/`in`/`instanceof` guards, extended here to user-defined type
/// predicates so that `isObject(x) && x.p` and `guard(x) ? x.p : …` behave like
/// their `if` counterparts.
/// The guard only holds in one of the two branches, so a `!`-negated condition
/// flips which one gets the downgrade.
pub(crate) fn downgrade_guarded_genuine_unknown(
    condition: &ParsedExpression,
    symbols: &SymbolTable,
    branch_is_true: bool,
) -> Option<SymbolTable> {
    if let ParsedExpression::Unary {
        operator: ParsedUnaryOperator::Not,
        operand,
        ..
    } = condition
    {
        return downgrade_guarded_genuine_unknown(operand, symbols, !branch_is_true);
    }
    if !branch_is_true {
        return None;
    }

    let mut names = crate::checks::function::guarded_value_identifiers(condition);
    collect_predicate_guard_subjects(condition, symbols, &mut names);
    downgrade_names(names, symbols)
}

/// The predicate-only half of [`downgrade_guarded_genuine_unknown`], for callers
/// that already applied the syntactic-guard narrowing.
pub(crate) fn downgrade_predicate_guarded_genuine_unknown(
    condition: &ParsedExpression,
    symbols: &SymbolTable,
) -> Option<SymbolTable> {
    let mut names = Vec::new();
    collect_predicate_guard_subjects(condition, symbols, &mut names);
    downgrade_names(names, symbols)
}

fn downgrade_names(names: Vec<String>, symbols: &SymbolTable) -> Option<SymbolTable> {
    let downgraded: Vec<(String, SymbolInfo)> = names
        .into_iter()
        .filter_map(|name| {
            let symbol = symbols.get(&name)?;
            matches!(symbol.ty, Type::GenuineUnknown).then(|| {
                (
                    name,
                    SymbolInfo {
                        ty: Type::Unknown,
                        kind: symbol.kind,
                        function_signature: symbol.function_signature.clone(),
                    },
                )
            })
        })
        .collect();

    if downgraded.is_empty() {
        return None;
    }

    let mut narrowed = symbols.clone_with_reason(TypeCopyReason::ScopeOrContext);
    for (name, symbol) in downgraded {
        narrowed.insert(name, symbol);
    }
    Some(narrowed)
}

/// Collects the bare-identifier arguments tested by user-defined type-predicate
/// calls within a (possibly `!`/`&&`/`||`-composed) condition.
fn collect_predicate_guard_subjects(
    condition: &ParsedExpression,
    symbols: &SymbolTable,
    names: &mut Vec<String>,
) {
    match condition {
        ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Not,
            operand,
            ..
        } => collect_predicate_guard_subjects(operand, symbols, names),
        ParsedExpression::Logical {
            left,
            operator: ParsedLogicalOperator::And | ParsedLogicalOperator::Or,
            right,
            ..
        } => {
            collect_predicate_guard_subjects(left, symbols, names);
            collect_predicate_guard_subjects(right, symbols, names);
        }
        _ => {
            if let Some(subject) = predicate_guard_subject(condition, symbols)
                && !names.iter().any(|existing| existing == &subject)
            {
                names.push(subject);
            }
        }
    }
}

/// The identifier tested by a single `isFoo(x)` call whose collected signature
/// declares a `param is T` predicate over a named value parameter. Generic
/// callees are refused: their predicate target cannot be resolved at the guard
/// site.
fn predicate_guard_subject(condition: &ParsedExpression, symbols: &SymbolTable) -> Option<String> {
    let ParsedExpression::Call {
        callee_name,
        type_arguments,
        arguments,
        ..
    } = condition
    else {
        return None;
    };
    if !type_arguments.is_empty() {
        return None;
    }
    let signature = symbols.get(callee_name)?.function_signature.as_ref()?;
    if !signature.type_parameters.is_empty() {
        return None;
    }
    let Some(ParsedType::Predicate(predicate)) = &signature.return_type else {
        return None;
    };
    if predicate.parameter_name == "this" {
        return None;
    }
    let index = signature
        .parameter_names
        .iter()
        .position(|name| name.as_deref() == Some(predicate.parameter_name.as_str()))?;
    let ParsedExpression::Identifier { name, .. } = &arguments.get(index)?.expression else {
        return None;
    };
    Some(name.clone())
}
