//! Cover Grammar for Destructuring Assignment

use oxc_ast::ast::*;
use oxc_span::{GetSpan, Span};

use crate::{ParserConfig as Config, ParserImpl, diagnostics};

pub trait CoverGrammar<'a, T, C: Config>: Sized {
    fn cover(value: T, p: &mut ParserImpl<'a, C>) -> Self;
}

impl<'a, C: Config> CoverGrammar<'a, Expression<'a>, C> for AssignmentTarget<'a> {
    fn cover(expr: Expression<'a>, p: &mut ParserImpl<'a, C>) -> Self {
        match expr {
            Expression::ArrayExpression(array_expr) => {
                let pat = ArrayAssignmentTarget::cover(array_expr.unbox(), p);
                AssignmentTarget::ArrayAssignmentTarget(p.alloc(pat))
            }
            Expression::ObjectExpression(object_expr) => {
                let pat = ObjectAssignmentTarget::cover(object_expr.unbox(), p);
                AssignmentTarget::ObjectAssignmentTarget(p.alloc(pat))
            }
            _ => AssignmentTarget::from(SimpleAssignmentTarget::cover(expr, p)),
        }
    }
}

impl<'a, C: Config> CoverGrammar<'a, Expression<'a>, C> for SimpleAssignmentTarget<'a> {
    fn cover(expr: Expression<'a>, p: &mut ParserImpl<'a, C>) -> Self {
        match expr {
            Expression::Identifier(ident) => {
                SimpleAssignmentTarget::AssignmentTargetIdentifier(ident)
            }
            match_member_expression!(Expression) => {
                let member_expr = expr.into_member_expression();
                SimpleAssignmentTarget::from(member_expr)
            }
            Expression::ParenthesizedExpression(expr) => {
                if matches!(
                    expr.expression,
                    Expression::ObjectExpression(_) | Expression::ArrayExpression(_)
                ) {
                    let span = expr.span;
                    return invalid_target(p, span, Expression::ParenthesizedExpression(expr));
                }
                SimpleAssignmentTarget::cover(expr.unbox().expression, p)
            }
            Expression::TSAsExpression(expr) => match expr.expression.get_inner_expression() {
                Expression::Identifier(_)
                | Expression::StaticMemberExpression(_)
                | Expression::ComputedMemberExpression(_)
                | Expression::PrivateFieldExpression(_) => {
                    SimpleAssignmentTarget::TSAsExpression(expr)
                }
                _ => invalid_target(p, expr.span, Expression::TSAsExpression(expr)),
            },
            Expression::TSSatisfiesExpression(expr) => {
                match expr.expression.get_inner_expression() {
                    Expression::Identifier(_)
                    | Expression::StaticMemberExpression(_)
                    | Expression::ComputedMemberExpression(_)
                    | Expression::PrivateFieldExpression(_) => {
                        SimpleAssignmentTarget::TSSatisfiesExpression(expr)
                    }
                    _ => invalid_target(p, expr.span, Expression::TSSatisfiesExpression(expr)),
                }
            }
            Expression::TSNonNullExpression(expr) => match expr.expression.get_inner_expression() {
                Expression::Identifier(_)
                | Expression::StaticMemberExpression(_)
                | Expression::ComputedMemberExpression(_)
                | Expression::PrivateFieldExpression(_) => {
                    SimpleAssignmentTarget::TSNonNullExpression(expr)
                }
                _ => invalid_target(p, expr.span, Expression::TSNonNullExpression(expr)),
            },
            Expression::TSTypeAssertion(expr) => match expr.expression.get_inner_expression() {
                Expression::Identifier(_)
                | Expression::StaticMemberExpression(_)
                | Expression::ComputedMemberExpression(_)
                | Expression::PrivateFieldExpression(_) => {
                    SimpleAssignmentTarget::TSTypeAssertion(expr)
                }
                _ => invalid_target(p, expr.span, Expression::TSTypeAssertion(expr)),
            },
            Expression::TSInstantiationExpression(expr) => {
                p.fatal_error(diagnostics::invalid_lhs_assignment(expr.span()))
            }
            expr => invalid_target(p, expr.span(), expr),
        }
    }
}

// surge: TS2364/TS2357/TS2779/TS2777 — tsc reports an invalid write target from the checker and
// keeps the file. The expression survives inside a non-null wrapper of exactly its span, which
// surge recognizes as this recovery.
fn invalid_target<'a, C: Config>(
    p: &mut ParserImpl<'a, C>,
    span: Span,
    expr: Expression<'a>,
) -> SimpleAssignmentTarget<'a> {
    p.error(diagnostics::invalid_assignment(span));
    SimpleAssignmentTarget::TSNonNullExpression(p.ast.alloc_ts_non_null_expression(span, expr))
}

impl<'a, C: Config> CoverGrammar<'a, ArrayExpression<'a>, C> for ArrayAssignmentTarget<'a> {
    fn cover(expr: ArrayExpression<'a>, p: &mut ParserImpl<'a, C>) -> Self {
        let mut elements = p.ast.vec();
        let mut rest = None;

        let len = expr.elements.len();
        for (i, elem) in expr.elements.into_iter().enumerate() {
            match elem {
                match_expression!(ArrayExpressionElement) => {
                    let expr = elem.into_expression();
                    let target = AssignmentTargetMaybeDefault::cover(expr, p);
                    elements.push(Some(target));
                }
                ArrayExpressionElement::SpreadElement(elem) => {
                    // surge: TS2462 — tsc reports a rest element that is not last and keeps the
                    // pattern; the last spread seen stays the rest target.
                    if i != len - 1 {
                        p.error(diagnostics::spread_last_element(elem.span));
                    }
                    if i == len - 1 || rest.is_none() {
                        let span = elem.span;
                        let argument = elem.unbox().argument;
                        if !matches!(
                            argument.get_inner_expression(),
                            Expression::Identifier(_)
                                | Expression::ArrayExpression(_)
                                | Expression::ObjectExpression(_)
                                | Expression::StaticMemberExpression(_)
                                | Expression::ComputedMemberExpression(_)
                                | Expression::PrivateFieldExpression(_)
                        ) {
                            p.error(diagnostics::invalid_rest_assignment_target(argument.span()));
                        }
                        let target = AssignmentTarget::cover(argument, p);
                        rest = Some(p.ast.alloc_assignment_target_rest(span, target));
                        if let Some(span) = p.state.trailing_commas.get(&expr.span.start) {
                            p.error(diagnostics::rest_element_trailing_comma(*span));
                        }
                    }
                }
                ArrayExpressionElement::Elision(_) => elements.push(None),
            }
        }

        p.ast.array_assignment_target(expr.span, elements, rest)
    }
}

impl<'a, C: Config> CoverGrammar<'a, Expression<'a>, C> for AssignmentTargetMaybeDefault<'a> {
    fn cover(expr: Expression<'a>, p: &mut ParserImpl<'a, C>) -> Self {
        match expr {
            Expression::AssignmentExpression(assignment_expr) => {
                if assignment_expr.operator != AssignmentOperator::Assign {
                    p.error(diagnostics::invalid_assignment_target_default_value_operator(
                        assignment_expr.span,
                    ));
                }
                let target = AssignmentTargetWithDefault::cover(assignment_expr.unbox(), p);
                AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(p.alloc(target))
            }
            expr => {
                let target = AssignmentTarget::cover(expr, p);
                AssignmentTargetMaybeDefault::from(target)
            }
        }
    }
}

impl<'a, C: Config> CoverGrammar<'a, AssignmentExpression<'a>, C>
    for AssignmentTargetWithDefault<'a>
{
    fn cover(expr: AssignmentExpression<'a>, p: &mut ParserImpl<'a, C>) -> Self {
        p.ast.assignment_target_with_default(expr.span, expr.left, expr.right)
    }
}

impl<'a, C: Config> CoverGrammar<'a, ObjectExpression<'a>, C> for ObjectAssignmentTarget<'a> {
    fn cover(expr: ObjectExpression<'a>, p: &mut ParserImpl<'a, C>) -> Self {
        let mut properties = p.ast.vec();
        let mut rest = None;

        let len = expr.properties.len();
        for (i, elem) in expr.properties.into_iter().enumerate() {
            match elem {
                ObjectPropertyKind::ObjectProperty(property) => {
                    let target = AssignmentTargetProperty::cover(property.unbox(), p);
                    properties.push(target);
                }
                ObjectPropertyKind::SpreadProperty(spread) => {
                    // surge: TS2462 — as for the array pattern above.
                    if i != len - 1 {
                        p.error(diagnostics::spread_last_element(spread.span));
                    }
                    if i == len - 1 || rest.is_none() {
                        let span = spread.span;
                        let argument = spread.unbox().argument;
                        if !matches!(
                            argument.get_inner_expression(),
                            Expression::Identifier(_)
                                | Expression::StaticMemberExpression(_)
                                | Expression::ComputedMemberExpression(_)
                                | Expression::PrivateFieldExpression(_)
                        ) {
                            p.error(diagnostics::invalid_rest_assignment_target(argument.span()));
                        }
                        if let Some(span) = p.state.trailing_commas.get(&expr.span.start) {
                            p.error(diagnostics::rest_element_trailing_comma(*span));
                        }
                        let target = AssignmentTarget::cover(argument, p);
                        rest = Some(p.ast.alloc_assignment_target_rest(span, target));
                    }
                }
            }
        }

        p.ast.object_assignment_target(expr.span, properties, rest)
    }
}

impl<'a, C: Config> CoverGrammar<'a, ObjectProperty<'a>, C> for AssignmentTargetProperty<'a> {
    fn cover(property: ObjectProperty<'a>, p: &mut ParserImpl<'a, C>) -> Self {
        if property.shorthand {
            let binding = match property.key {
                PropertyKey::StaticIdentifier(ident) => {
                    let ident = ident.unbox();
                    p.ast.identifier_reference(ident.span, ident.name)
                }
                _ => return p.unexpected(),
            };
            // convert `CoverInitializedName`
            let init = p.state.cover_initialized_name.remove(&property.span.start).map(|e| e.right);
            p.ast.assignment_target_property_assignment_target_property_identifier(
                property.span,
                binding,
                init,
            )
        } else {
            let binding = AssignmentTargetMaybeDefault::cover(property.value, p);
            p.ast.assignment_target_property_assignment_target_property_property(
                property.span,
                property.key,
                binding,
                property.computed,
            )
        }
    }
}
