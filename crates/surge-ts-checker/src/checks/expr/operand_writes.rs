//! Operand rules for `delete` and for `++`/`--`, which tsc checks in
//! `checkDeleteExpression` and `checkPre/PostfixUnaryExpression`
//! (`tsc/internal/checker/checker.go`). Both operators write through their
//! operand, so both reject a `readonly` or `const` target; `delete`
//! additionally requires a property reference whose type admits `undefined`.

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{ParsedExpression, TextSpan as SyntaxTextSpan};
use surge_ts_types::{Type, union_type};

use crate::checks::function::property_write_is_readonly;
use crate::context::{CheckerContext, convert_span};
use crate::infer::{InferredExpression, infer_expression};
use crate::symbols::SymbolTable;

/// The member a `delete` or update operand names, with the receiver it is read
/// from. `None` for anything that is not a property reference.
fn operand_property<'a>(
    operand: &'a ParsedExpression,
) -> Option<(&'a ParsedExpression, PropertyKey<'a>)> {
    match operand {
        ParsedExpression::PropertyAccess {
            object,
            property_name,
            ..
        }
        | ParsedExpression::OptionalPropertyAccess {
            object,
            property_name,
            ..
        } => Some((object, PropertyKey::Name(property_name))),
        ParsedExpression::ElementAccess { object, index, .. } => {
            Some((object, PropertyKey::Computed(index)))
        }
        _ => None,
    }
}

enum PropertyKey<'a> {
    Name(&'a str),
    Computed(&'a ParsedExpression),
}

impl PropertyKey<'_> {
    /// The literal member name this key resolves to, when it has one. A
    /// computed key that is not a literal names no single member, so the
    /// member-level rules cannot apply to it.
    fn literal_name(&self, symbols: &SymbolTable, ctx: &mut CheckerContext) -> Option<String> {
        match self {
            PropertyKey::Name(name) => Some((*name).to_string()),
            PropertyKey::Computed(index) => match infer_expression(index, symbols, ctx) {
                InferredExpression::Known(Type::StringLiteral(value)) => Some(value.to_string()),
                InferredExpression::Known(Type::NumberLiteral(value)) => Some(value.value),
                _ => None,
            },
        }
    }
}

/// An `IndexAccess` names its object by bare identifier rather than carrying an
/// expression, so it needs its own receiver lookup.
fn index_access_receiver(
    operand: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<(Type, String)> {
    let ParsedExpression::IndexAccess {
        object_name, index, ..
    } = operand
    else {
        return None;
    };
    let receiver = symbols.get(object_name)?.ty.clone();
    let name = match infer_expression(index, symbols, ctx) {
        InferredExpression::Known(Type::StringLiteral(value)) => value.to_string(),
        InferredExpression::Known(Type::NumberLiteral(value)) => value.value,
        _ => return None,
    };
    Some((receiver, name))
}

/// The receiver type and member name a write through `operand` targets.
fn write_target(
    operand: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<(Type, String)> {
    if let Some(target) = index_access_receiver(operand, symbols, ctx) {
        return Some(target);
    }
    let (object, key) = operand_property(operand)?;
    let name = key.literal_name(symbols, ctx)?;
    let receiver = match infer_expression(object, symbols, ctx) {
        InferredExpression::Known(ty) => ty,
        _ => return None,
    };
    Some((receiver, name))
}

/// Where tsc anchors a member-level diagnostic about `operand`: the property
/// name alone, not the whole access expression.
fn property_name_span(operand: &ParsedExpression) -> Option<SyntaxTextSpan> {
    match operand {
        ParsedExpression::PropertyAccess { property_span, .. }
        | ParsedExpression::OptionalPropertyAccess { property_span, .. } => *property_span,
        ParsedExpression::IndexAccess { index_span, .. }
        | ParsedExpression::ElementAccess { index_span, .. } => *index_span,
        _ => None,
    }
}

/// `delete o.p`. Mirrors `checkDeleteExpression`: the operand must be a property
/// reference, must not be a private identifier, and must be either `readonly`
/// (rejected) or optional.
pub(crate) fn check_delete_operand(
    operand: &ParsedExpression,
    operand_span: Option<SyntaxTextSpan>,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    let Some(span) = operand_span else {
        return;
    };

    // An operand surge did not model arrives as `Unknown`. A private field is
    // the case that reaches here today, and reporting a *shape* rule on it
    // would be a false positive about syntax that is in fact a property
    // reference — so TS18011 and TS2790 stay unreported rather than wrong.
    if matches!(operand, ParsedExpression::Unknown) {
        return;
    }

    let is_property_reference = matches!(
        operand,
        ParsedExpression::PropertyAccess { .. }
            | ParsedExpression::OptionalPropertyAccess { .. }
            | ParsedExpression::IndexAccess { .. }
            | ParsedExpression::ElementAccess { .. }
    );
    if !is_property_reference {
        let file_name = ctx.file_name.clone();
        ctx.push(Diagnostic::ts2703(file_name).with_span(convert_span(span)));
        return;
    }

    let Some((receiver, property_name)) = write_target(operand, symbols, ctx) else {
        return;
    };

    if property_write_is_readonly(&receiver, &property_name, ctx) {
        let file_name = ctx.file_name.clone();
        ctx.push(Diagnostic::ts2704(file_name).with_span(convert_span(span)));
        return;
    }

    // tsc gates this on `strictNullChecks`. surge models no such flag — it
    // checks as if it were always on, which is what its possibly-undefined
    // diagnostics already assume.
    let unnarrowed = unnarrowed_receiver(operand, symbols, ctx).unwrap_or(receiver);
    let Some(property_type) = declared_property_type(&unnarrowed, &property_name) else {
        return;
    };
    // `any`, `unknown` and `never` are exempt, as is anything surge failed to
    // model — the operand rule must not fire on a type we did not resolve.
    if matches!(
        property_type,
        Type::Any | Type::GenuineUnknown | Type::Never | Type::Unknown
    ) {
        return;
    }
    if !type_includes_undefined(&property_type) {
        let file_name = ctx.file_name.clone();
        ctx.push(Diagnostic::ts2790(file_name).with_span(convert_span(span)));
    }
}

/// The receiver type as *declared*, undoing flow narrowing along the access
/// path. `checkDeleteExpressionMustBeOptional` reads `getTypeOfSymbol(symbol)`,
/// which is the member's declaration, so the idiomatic `if (o.p) delete o.p`
/// must still see `p?: T` as optional even though the guard narrowed it.
/// `None` where nothing was narrowed, leaving the flow receiver in place.
fn unnarrowed_receiver(
    operand: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    let (object, _) = operand_property(operand)?;
    declared_expression_type(object, symbols, ctx)
}

fn declared_expression_type(
    expression: &ParsedExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    match expression {
        ParsedExpression::Identifier { name, .. } => symbols.declared_type(name).cloned(),
        ParsedExpression::PropertyAccess {
            object,
            property_name,
            ..
        }
        | ParsedExpression::OptionalPropertyAccess {
            object,
            property_name,
            ..
        } => {
            let receiver = declared_expression_type(object, symbols, ctx)?;
            let Type::Object(object_type) = receiver.peeled() else {
                return None;
            };
            Some(object_type.properties.get(property_name.as_str())?.ty.clone())
        }
        _ => None,
    }
}

fn declared_property_type(receiver: &Type, property_name: &str) -> Option<Type> {
    match receiver.peeled() {
        Type::Object(object) => {
            let property = object.properties.get(property_name)?;
            Some(if property.optional {
                union_type(vec![property.ty.clone(), Type::Undefined])
            } else {
                property.ty.clone()
            })
        }
        _ => None,
    }
}

fn type_includes_undefined(ty: &Type) -> bool {
    match ty {
        Type::Undefined | Type::Any | Type::Void => true,
        Type::Union(union) => union.types().iter().any(type_includes_undefined),
        _ => false,
    }
}

/// `x++` / `--o.p`. Mirrors the `++`/`--` arms of `checkPre/PostfixUnaryExpression`:
/// the write is rejected first (tsc's `checkIdentifier` reports it and hands back
/// the error type, which then satisfies the arithmetic check), and only an
/// accepted target has its operand type checked.
pub(crate) fn check_update_operand(
    operand: &ParsedExpression,
    operand_span: Option<SyntaxTextSpan>,
    operand_result: &InferredExpression,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) {
    let Some(span) = operand_span else {
        return;
    };

    if let ParsedExpression::Identifier { name, .. } = operand {
        if let Some(symbol) = symbols.get(name) {
            if let Some(diagnostic) = crate::checks::assign::unwritable_binding_diagnostic(
                name,
                &symbol,
                ctx.file_name.clone(),
            ) {
                ctx.push(diagnostic.with_span(convert_span(span)));
                return;
            }
        }
    } else if let Some((receiver, property_name)) = write_target(operand, symbols, ctx)
        && property_write_is_readonly(&receiver, &property_name, ctx)
    {
        let file_name = ctx.file_name.clone();
        let anchor = property_name_span(operand).unwrap_or(span);
        ctx.push(Diagnostic::ts2540(&property_name, file_name).with_span(convert_span(anchor)));
        return;
    }

    let InferredExpression::Known(operand_type) = operand_result else {
        return;
    };

    // tsc runs `checkNonNullType` on the operand first, so a possibly-undefined
    // operand is reported as that and never as a bad arithmetic operand.
    if super::maybe_emit_possibly_undefined_receiver(
        operand,
        operand_type,
        Some(span),
        None,
        symbols,
        ctx,
    ) {
        return;
    }

    if operand_type.is_unknown() || is_arithmetic_operand(operand_type) {
        return;
    }

    let file_name = ctx.file_name.clone();
    ctx.push(Diagnostic::ts2356(file_name).with_span(convert_span(span)));
}

/// tsc accepts an operand assignable to `number | bigint`, which `any` and a
/// numeric enum member both are. A union qualifies only if every constituent
/// does, ignoring `null`/`undefined`: tsc runs `checkNonNullType` first, so
/// `number | undefined` is reported as possibly-undefined and not as a bad
/// arithmetic operand.
fn is_arithmetic_operand(ty: &Type) -> bool {
    match ty {
        Type::Any => true,
        Type::Union(union) => union
            .types()
            .iter()
            .filter(|member| !matches!(member, Type::Undefined | Type::Void))
            .all(is_arithmetic_operand),
        other => matches!(
            other.base_primitive(),
            Some(Type::Number) | Some(Type::BigInt)
        ),
    }
}

/// tsc's `getUnaryResultType`: a bigint operand keeps `bigint`, everything else
/// coerces to `number`.
pub(crate) fn update_result_type(operand_result: &InferredExpression) -> InferredExpression {
    match operand_result {
        InferredExpression::Known(ty) if matches!(ty.base_primitive(), Some(Type::BigInt)) => {
            InferredExpression::Known(Type::BigInt)
        }
        _ => InferredExpression::Known(Type::Number),
    }
}
