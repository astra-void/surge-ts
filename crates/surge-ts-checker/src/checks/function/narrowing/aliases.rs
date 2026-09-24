//! tsc's aliased conditions (`narrowType`, flow.go): testing a `const` whose
//! initializer is a condition narrows by that condition, inlined at most five
//! levels deep and only for the references `isConstantReference` accepts.

use std::cell::Cell;

use surge_ts_syntax::{ParsedExpression, ParsedLogicalOperator, ParsedUnaryOperator};
use surge_ts_types::Type;

use crate::symbols::{SymbolKind, SymbolTable};
use super::guards::*;
use super::reference_path;

/// tsc inlines an alias only while fewer than five inlined aliases enclose it
/// (`inlineLevel`).
pub(crate) const ALIAS_INLINE_LIMIT: usize = 5;

thread_local! {
    static ALIAS_INLINE_LEVEL: Cell<usize> = const { Cell::new(0) };
}

/// Holds one level of `inlineLevel` while an alias's initializer is narrowed.
pub(super) struct AliasInlining(());

impl Drop for AliasInlining {
    fn drop(&mut self) {
        ALIAS_INLINE_LEVEL.with(|level| level.set(level.get() - 1));
    }
}

/// Enters one more level of alias inlining, or `None` at the limit.
pub(super) fn enter_alias_inlining() -> Option<AliasInlining> {
    ALIAS_INLINE_LEVEL.with(|level| {
        (level.get() < ALIAS_INLINE_LIMIT).then(|| {
            level.set(level.get() + 1);
            AliasInlining(())
        })
    })
}

/// A reference a guard narrows: a binding, the properties read off it, and
/// whether an element access (`obj[0]`) ends it.
struct NarrowedReference {
    base: String,
    path: Vec<String>,
    element: bool,
}

impl NarrowedReference {
    fn of(expression: &ParsedExpression) -> Option<Self> {
        match expression {
            ParsedExpression::IndexAccess { object_name, .. } => Some(Self {
                base: object_name.clone(),
                path: Vec::new(),
                element: true,
            }),
            ParsedExpression::ElementAccess { object, .. } => {
                reference_path(object).map(|(base, path)| Self {
                    base,
                    path,
                    element: true,
                })
            }
            _ => reference_path(expression).map(|(base, path)| Self {
                base,
                path,
                element: false,
            }),
        }
    }

    /// The object the reference's last property is read from.
    fn object(mut self) -> Self {
        if self.element {
            self.element = false;
        } else {
            self.path.pop();
        }
        self
    }
}

/// tsc's `isConstantReference`: `this`, a `const`, or a parameter or `let`
/// its container never assigns, reached through `readonly` properties only —
/// and, for an element access, a `readonly` tuple's element.
fn is_constant_reference(
    reference: &NarrowedReference,
    symbols: &SymbolTable,
    is_assigned: &dyn Fn(&str) -> bool,
) -> bool {
    let NarrowedReference {
        base,
        path,
        element,
    } = reference;
    let symbol = symbols.get(base);
    let root_constant = base == "this"
        || symbol.is_some_and(|symbol| match symbol.kind {
            SymbolKind::Const | SymbolKind::ForInNumericKey => true,
            SymbolKind::Parameter | SymbolKind::Let => !is_assigned(base),
            SymbolKind::Var | SymbolKind::Function | SymbolKind::ErrorImport => false,
        });
    if !root_constant {
        return false;
    }
    if path.is_empty() && !element {
        return true;
    }
    let Some(mut current) = symbol.map(|symbol| symbol.ty.clone()) else {
        return false;
    };
    for segment in path {
        let Some((property_ty, readonly)) = readonly_property(&current, segment) else {
            return false;
        };
        if !readonly {
            return false;
        }
        current = property_ty;
    }
    !element
        || matches!(&current, Type::Reference(reference) if reference.is_readonly_array())
            && matches!(current.peeled(), Type::Tuple(_) | Type::OpenTuple(_))
}

/// A property's type and whether it is `readonly`; through a union it is when
/// some member declares it so, as a union property's check flags read.
fn readonly_property(ty: &Type, name: &str) -> Option<(Type, bool)> {
    match ty.peeled() {
        Type::Object(object) => {
            let property = object.properties.get(name)?;
            Some((property.ty.clone(), property.readonly))
        }
        Type::Union(union) => {
            let mut types = Vec::new();
            let mut readonly = false;
            for member in union.types() {
                if matches!(member, Type::Undefined | Type::Null) {
                    continue;
                }
                let (property_ty, member_readonly) = readonly_property(member, name)?;
                types.push(property_ty);
                readonly |= member_readonly;
            }
            Some((surge_ts_types::union_type(types), readonly))
        }
        _ => None,
    }
}

/// The initializer of an aliased condition as tsc inlines it for the
/// references it may narrow: `isConstantReference` gates the inlining per
/// narrowed reference, so a guard none of whose references is constant is
/// dropped — it narrows nothing through the alias.
pub(crate) fn retain_constant_reference_guards(
    condition: &ParsedExpression,
    symbols: &SymbolTable,
    is_assigned: &dyn Fn(&str) -> bool,
) -> ParsedExpression {
    match condition {
        ParsedExpression::Logical {
            left,
            left_span,
            operator: operator @ (ParsedLogicalOperator::And | ParsedLogicalOperator::Or),
            operator_span,
            right,
            right_span,
        } => ParsedExpression::Logical {
            left: Box::new(retain_constant_reference_guards(left, symbols, is_assigned)),
            left_span: *left_span,
            operator: *operator,
            operator_span: *operator_span,
            right: Box::new(retain_constant_reference_guards(right, symbols, is_assigned)),
            right_span: *right_span,
        },
        ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Not,
            operator_span,
            operand,
            operand_span,
        } => ParsedExpression::Unary {
            operator: ParsedUnaryOperator::Not,
            operator_span: *operator_span,
            operand: Box::new(retain_constant_reference_guards(operand, symbols, is_assigned)),
            operand_span: *operand_span,
        },
        _ => {
            let guard = strip_boolean_literal_comparison(condition)
                .map_or(condition, |(inner, _)| inner);
            let references = narrowed_references(guard, symbols);
            let narrows_constant = references.is_empty()
                || references
                    .iter()
                    .any(|reference| is_constant_reference(reference, symbols, is_assigned));
            if narrows_constant {
                condition.clone()
            } else {
                ParsedExpression::Unknown
            }
        }
    }
}

/// The references a single guard narrows: the tested operand of `typeof`,
/// `instanceof`, `in`, `Array.isArray` and a predicate call, both operands of
/// an equality, and the object a discriminant or a truthiness test on one of
/// its properties filters.
fn narrowed_references(
    guard: &ParsedExpression,
    symbols: &SymbolTable,
) -> Vec<NarrowedReference> {
    let single =
        |expression: &ParsedExpression| NarrowedReference::of(expression).into_iter().collect();
    if let Some((operand, _, _)) = parse_typeof_condition(guard) {
        return single(operand);
    }
    if let Some((operand, _)) = parse_instanceof_condition(guard) {
        return single(operand);
    }
    if let Some((object, _)) = parse_in_condition(guard) {
        return single(object);
    }
    if let Some(operand) =
        parse_array_isarray_condition(guard).or_else(|| parse_arraybuffer_isview_condition(guard))
    {
        return single(operand);
    }
    if let Some((subject, _, _)) = parse_nullish_equality_condition(guard) {
        return single(subject);
    }
    if let Some((object, ..)) = parse_discriminant_condition_with(guard, &|expression| {
        const_member_literal_value(expression, symbols)
    }) {
        return single(object);
    }
    if let Some(test) = parse_equality_test(guard) {
        return [test.left, test.right].into_iter().filter_map(NarrowedReference::of).collect();
    }
    // A predicate call narrows its tested argument, and a `this is T` method
    // called with none narrows its receiver (`getTypePredicateArgument`).
    let argument_references = |arguments: &[surge_ts_syntax::ParsedCallArgument]| -> Vec<_> {
        arguments
            .iter()
            .filter_map(|argument| NarrowedReference::of(&argument.expression))
            .collect()
    };
    match guard {
        ParsedExpression::Call { arguments, .. } => argument_references(arguments),
        ParsedExpression::PropertyCall {
            object, arguments, ..
        } => {
            let references = argument_references(arguments);
            if references.is_empty() {
                single(object)
            } else {
                references
            }
        }
        _ => NarrowedReference::of(guard)
            .map(NarrowedReference::object)
            .into_iter()
            .collect(),
    }
}
