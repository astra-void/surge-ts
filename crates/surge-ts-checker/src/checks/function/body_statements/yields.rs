//! The `yield`s of an unannotated generator that tsc types as an implicit
//! `any` (TS7057).

use surge_ts_syntax::{
    ParsedCallArgument, ParsedExpression, ParsedFunctionBodyStatement, ParsedLogicalOperator,
    ParsedUnaryOperator,
};
use surge_ts_types::Type;

thread_local! {
    /// What the generator bodies being checked yield, innermost last. A body
    /// whose yields type nothing (an annotated generator) holds `None`, so a
    /// generator nested in it does not report into an outer one.
    static GENERATOR_YIELDS: std::cell::RefCell<Vec<Option<GeneratorYields>>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// The `yield`s of one generator body, as tsc's
/// `checkAndAggregateYieldOperandTypes` aggregates them.
#[derive(Default)]
pub(crate) struct GeneratorYields {
    pub(crate) yielded: Vec<Type>,
    /// The types a `yield*` delegate is resumed with.
    pub(crate) sent: Vec<Type>,
    /// Some yield surge could not type, so any aggregate would be a guess.
    pub(crate) unmodelled: bool,
}

/// Runs `check` over a generator body, collecting its yields when `collect`.
pub(crate) fn collecting_generator_yields<R>(
    collect: bool,
    check: impl FnOnce() -> R,
) -> (R, Option<GeneratorYields>) {
    struct Frame(usize);
    impl Drop for Frame {
        fn drop(&mut self) {
            GENERATOR_YIELDS.with(|frames| frames.borrow_mut().truncate(self.0));
        }
    }
    let depth = GENERATOR_YIELDS.with(|frames| {
        let mut frames = frames.borrow_mut();
        frames.push(collect.then(GeneratorYields::default));
        frames.len() - 1
    });
    let _frame = Frame(depth);
    let result = check();
    let yields = GENERATOR_YIELDS.with(|frames| frames.borrow_mut().get_mut(depth).and_then(Option::take));
    (result, yields)
}

/// Records a `yield` of the innermost generator body: what it yields (`None`
/// when surge could not type it) and, for a `yield*`, what its delegate is
/// resumed with.
pub(crate) fn record_generator_yield(yielded: Option<Type>, sent: Option<Type>) {
    GENERATOR_YIELDS.with(|frames| {
        let mut frames = frames.borrow_mut();
        let Some(Some(yields)) = frames.last_mut() else {
            return;
        };
        match yielded {
            Some(ty) if !yields.yielded.contains(&ty) => yields.yielded.push(ty),
            Some(_) => {}
            None => yields.unmodelled = true,
        }
        if let Some(sent) = sent
            && !yields.sent.contains(&sent)
        {
            yields.sent.push(sent);
        }
    });
}

/// tsc's `checkYieldExpression` reports a `yield` of a generator with neither
/// a return type annotation nor a contextual signature when its value is used
/// (`expressionResultIsUnused`) where it has no contextual type of its own
/// (`getContextualType` answers nothing, or `any`). The starts collected are
/// those of the `yield`s whose position has no contextual type by syntax
/// alone — an unannotated initializer, a returned or thrown value, a
/// condition, an operator's operand, a template substitution, another
/// `yield`'s operand. A call argument, an assigned value, an asserted
/// expression and a destructuring source are contextually typed by a type
/// this walk cannot see, and are left out.
pub(crate) fn implicit_any_yield_starts(body: &[ParsedFunctionBodyStatement]) -> Vec<usize> {
    let mut starts = Vec::new();
    collect_statements(body, &mut starts);
    starts
}

fn collect_statements(statements: &[ParsedFunctionBodyStatement], out: &mut Vec<usize>) {
    for statement in statements {
        collect_statement(statement, out);
    }
}

fn collect_statement(statement: &ParsedFunctionBodyStatement, out: &mut Vec<usize>) {
    match statement {
        ParsedFunctionBodyStatement::VariableDeclaration(variable) => {
            // A pattern's source is contextually typed by the pattern, and the
            // lowering reads it once per binding.
            if let Some(initializer) = &variable.initializer
                && !variable.from_binding_pattern
            {
                let uncontextual =
                    variable.declared_type.is_none() && variable.annotated_pattern.is_none();
                collect_expression(initializer, uncontextual, out);
            }
        }
        ParsedFunctionBodyStatement::Return(statement) => {
            if let Some(expression) = &statement.expression {
                collect_expression(expression, true, out);
            }
        }
        ParsedFunctionBodyStatement::Throw(statement) => {
            collect_expression(&statement.expression, true, out);
        }
        ParsedFunctionBodyStatement::Expression(expression) => {
            collect_expression(expression, false, out);
        }
        ParsedFunctionBodyStatement::Block(statements) => collect_statements(statements, out),
        ParsedFunctionBodyStatement::If(statement) => {
            collect_expression(&statement.condition, true, out);
            collect_statements(&statement.then_body, out);
            collect_statements(&statement.else_body, out);
        }
        ParsedFunctionBodyStatement::While(statement) => {
            collect_expression(&statement.condition, true, out);
            collect_statements(&statement.body, out);
        }
        ParsedFunctionBodyStatement::ForOf(statement) => {
            collect_expression(&statement.iterable, true, out);
            collect_statements(&statement.body, out);
        }
        ParsedFunctionBodyStatement::Switch(statement) => {
            collect_expression(&statement.discriminant, true, out);
            for case in &statement.cases {
                if let Some(test) = &case.test {
                    collect_expression(test, true, out);
                }
                collect_statements(&case.consequent, out);
            }
        }
        ParsedFunctionBodyStatement::Try(statement) => {
            collect_statements(&statement.block, out);
            if let Some(handler) = &statement.handler {
                collect_statements(&handler.body, out);
            }
            collect_statements(&statement.finalizer, out);
        }
        _ => {}
    }
}

/// `uncontextual`: the expression's own position gives it no contextual type
/// and uses its value.
fn collect_expression(expression: &ParsedExpression, uncontextual: bool, out: &mut Vec<usize>) {
    match expression {
        ParsedExpression::Yield {
            operand,
            delegate,
            span,
            ..
        } => {
            // A `yield*` evaluates to its operand's return type instead.
            if uncontextual
                && !*delegate
                && let Some(span) = span
            {
                out.push(span.start);
            }
            if let Some(operand) = operand {
                collect_expression(operand, true, out);
            }
        }
        ParsedExpression::TemplateLiteral { expressions, .. } => {
            for expression in expressions {
                collect_expression(expression, true, out);
            }
        }
        ParsedExpression::Unary { operator, operand, .. } => {
            collect_expression(operand, *operator != ParsedUnaryOperator::Void, out);
        }
        ParsedExpression::Update { operand, .. } => collect_expression(operand, true, out),
        ParsedExpression::Await { operand, .. } => collect_expression(operand, uncontextual, out),
        ParsedExpression::Binary { left, right, .. } => {
            collect_expression(left, true, out);
            collect_expression(right, true, out);
        }
        // `getContextualTypeForBinaryOperand`: `&&` hands its right operand
        // its own contextual type; `||` and `??` hand their left operand
        // theirs and the right operand the left operand's type.
        ParsedExpression::Logical {
            left,
            operator,
            right,
            ..
        } => match operator {
            ParsedLogicalOperator::And => {
                collect_expression(left, true, out);
                collect_expression(right, uncontextual, out);
            }
            ParsedLogicalOperator::Or => {
                collect_expression(left, uncontextual, out);
                collect_expression(right, false, out);
            }
        },
        ParsedExpression::NullishCoalescing { left, right, .. } => {
            collect_expression(left, uncontextual, out);
            collect_expression(right, false, out);
        }
        ParsedExpression::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => {
            collect_expression(condition, true, out);
            collect_expression(when_true, uncontextual, out);
            collect_expression(when_false, uncontextual, out);
        }
        ParsedExpression::PropertyAccess {
            object,
            binding_element,
            ..
        } => collect_expression(object, !*binding_element, out),
        ParsedExpression::OptionalPropertyAccess { object, .. } => {
            collect_expression(object, true, out);
        }
        ParsedExpression::IndexAccess { index, .. } => collect_expression(index, true, out),
        ParsedExpression::OptionalIndexAccess { object, index, .. } => {
            collect_expression(object, true, out);
            collect_expression(index, true, out);
        }
        // The object may be a destructuring source the lowering indexes.
        ParsedExpression::ElementAccess { object, index, .. } => {
            collect_expression(object, false, out);
            collect_expression(index, true, out);
        }
        ParsedExpression::Call { arguments, .. } => collect_arguments(arguments, out),
        ParsedExpression::PropertyCall {
            object, arguments, ..
        }
        | ParsedExpression::OptionalPropertyCall {
            object, arguments, ..
        } => {
            collect_expression(object, true, out);
            collect_arguments(arguments, out);
        }
        ParsedExpression::New {
            callee, arguments, ..
        }
        | ParsedExpression::OptionalCall {
            callee, arguments, ..
        }
        | ParsedExpression::ExpressionCall {
            callee, arguments, ..
        } => {
            collect_expression(callee, true, out);
            collect_arguments(arguments, out);
        }
        ParsedExpression::ArrayLiteral {
            elements,
            tuple_context,
            ..
        } => {
            for element in elements {
                collect_expression(
                    &element.expression,
                    uncontextual && !*tuple_context && !element.spread,
                    out,
                );
            }
        }
        ParsedExpression::ObjectLiteral { properties, .. } => {
            for property in properties {
                if property.is_method || property.is_accessor {
                    continue;
                }
                if let Some(key) = &property.computed_key {
                    collect_expression(key, true, out);
                }
                collect_expression(&property.value, uncontextual && !property.is_spread, out);
            }
        }
        ParsedExpression::Sequence { expressions } => {
            let last = expressions.len().saturating_sub(1);
            for (index, (expression, _)) in expressions.iter().enumerate() {
                collect_expression(expression, uncontextual && index == last, out);
            }
        }
        ParsedExpression::NonNullAssertion { expression, .. } => {
            collect_expression(expression, uncontextual, out);
        }
        ParsedExpression::Assignment { value, .. } => collect_expression(value, false, out),
        ParsedExpression::TypeAssertion { expression, .. }
        | ParsedExpression::SatisfiesExpression { expression, .. }
        | ParsedExpression::ConstAssertion { expression, .. } => {
            collect_expression(expression, false, out);
        }
        _ => {}
    }
}

fn collect_arguments(arguments: &[ParsedCallArgument], out: &mut Vec<usize>) {
    for argument in arguments {
        collect_expression(&argument.expression, false, out);
    }
}

/// Whether the body yields and every `yield` in it yields a widening
/// `undefined` or `null`: a bare `yield`, `yield undefined`, `yield null`,
/// `yield void …`, or a `yield*` of `[]` (an `undefined[]` without
/// `strictNullChecks`). Without `strictNullChecks` that is what tsc's
/// `getReturnTypeFromBody` widens to an implicit `any` yield type
/// (`reportErrorsFromWidening`); any other yielded type absorbs those under
/// subtype reduction. A class in the body is not looked into, so it answers
/// `false`.
/// Every `yield` of a generator body with its operand and whether it
/// delegates, in source order; the yields of nested functions are theirs.
/// `None` when a class in the body could hide one.
pub(crate) fn yield_operands(body: &[ParsedFunctionBodyStatement]) -> Option<Vec<(Option<&ParsedExpression>, bool)>> {
    type Found<'a> = Option<Vec<(Option<&'a ParsedExpression>, bool)>>;
    fn walk_statements<'a>(statements: &'a [ParsedFunctionBodyStatement], out: &mut Found<'a>) {
        for child in statements {
            walk_statement(child, out);
        }
    }
    fn walk_statement<'a>(node: &'a ParsedFunctionBodyStatement, out: &mut Found<'a>) {
        match node {
            ParsedFunctionBodyStatement::VariableDeclaration(variable) => {
                if let Some(initializer) = &variable.initializer {
                    walk_expression(initializer, out);
                }
            }
            ParsedFunctionBodyStatement::Return(returned) => {
                if let Some(value) = &returned.expression {
                    walk_expression(value, out);
                }
            }
            ParsedFunctionBodyStatement::Throw(thrown) => walk_expression(&thrown.expression, out),
            ParsedFunctionBodyStatement::Assignment(assignment) => walk_expression(&assignment.value, out),
            ParsedFunctionBodyStatement::ThisPropertyAssignment(assignment) => {
                walk_expression(&assignment.value, out);
            }
            ParsedFunctionBodyStatement::MemberAssignment(assignment) => {
                walk_expression(&assignment.target, out);
                walk_expression(&assignment.value, out);
            }
            ParsedFunctionBodyStatement::Expression(value) => walk_expression(value, out),
            ParsedFunctionBodyStatement::Block(statements) => walk_statements(statements, out),
            ParsedFunctionBodyStatement::If(branch) => {
                walk_expression(&branch.condition, out);
                walk_statements(&branch.then_body, out);
                walk_statements(&branch.else_body, out);
            }
            ParsedFunctionBodyStatement::While(looped) => {
                walk_expression(&looped.condition, out);
                walk_statements(&looped.body, out);
            }
            ParsedFunctionBodyStatement::ForOf(looped) => {
                walk_expression(&looped.iterable, out);
                if let Some((target, _)) = &looped.head_target {
                    walk_expression(target, out);
                }
                walk_statements(&looped.body, out);
            }
            ParsedFunctionBodyStatement::Switch(switch) => {
                walk_expression(&switch.discriminant, out);
                for case in &switch.cases {
                    if let Some(test) = &case.test {
                        walk_expression(test, out);
                    }
                    walk_statements(&case.consequent, out);
                }
            }
            ParsedFunctionBodyStatement::Try(attempt) => {
                walk_statements(&attempt.block, out);
                if let Some(handler) = &attempt.handler {
                    walk_statements(&handler.body, out);
                }
                walk_statements(&attempt.finalizer, out);
            }
            ParsedFunctionBodyStatement::Class(_) => *out = None,
            _ => {}
        }
    }
    fn walk_expression<'a>(expression: &'a ParsedExpression, out: &mut Found<'a>) {
        match expression {
            ParsedExpression::Yield { operand, delegate, .. } => {
                if let Some(found) = out.as_mut() {
                    found.push((operand.as_deref(), *delegate));
                }
            }
            ParsedExpression::ClassExpression(_) => *out = None,
            _ => {}
        }
        expression.for_each_child(&mut |child| walk_expression(child, out));
    }
    let mut out = Some(Vec::new());
    walk_statements(body, &mut out);
    out
}

pub(crate) fn yields_only_widening_nullish(body: &[ParsedFunctionBodyStatement]) -> bool {
    let mut census = YieldCensus::default();
    census.statements(body);
    census.nullish > 0 && !census.other
}

#[derive(Default)]
struct YieldCensus {
    nullish: usize,
    other: bool,
}

impl YieldCensus {
    fn statements(&mut self, statements: &[ParsedFunctionBodyStatement]) {
        for statement in statements {
            self.statement(statement);
        }
    }

    fn statement(&mut self, statement: &ParsedFunctionBodyStatement) {
        match statement {
            ParsedFunctionBodyStatement::VariableDeclaration(variable) => {
                if let Some(initializer) = &variable.initializer {
                    self.expression(initializer);
                }
            }
            ParsedFunctionBodyStatement::Return(statement) => {
                if let Some(expression) = &statement.expression {
                    self.expression(expression);
                }
            }
            ParsedFunctionBodyStatement::Throw(statement) => self.expression(&statement.expression),
            ParsedFunctionBodyStatement::Assignment(assignment) => self.expression(&assignment.value),
            ParsedFunctionBodyStatement::ThisPropertyAssignment(assignment) => {
                self.expression(&assignment.value);
            }
            ParsedFunctionBodyStatement::MemberAssignment(assignment) => {
                self.expression(&assignment.target);
                self.expression(&assignment.value);
            }
            ParsedFunctionBodyStatement::Expression(expression) => self.expression(expression),
            ParsedFunctionBodyStatement::Block(statements) => self.statements(statements),
            ParsedFunctionBodyStatement::If(statement) => {
                self.expression(&statement.condition);
                self.statements(&statement.then_body);
                self.statements(&statement.else_body);
            }
            ParsedFunctionBodyStatement::While(statement) => {
                self.expression(&statement.condition);
                self.statements(&statement.body);
            }
            ParsedFunctionBodyStatement::ForOf(statement) => {
                self.expression(&statement.iterable);
                if let Some((target, _)) = &statement.head_target {
                    self.expression(target);
                }
                self.statements(&statement.body);
            }
            ParsedFunctionBodyStatement::Switch(statement) => {
                self.expression(&statement.discriminant);
                for case in &statement.cases {
                    if let Some(test) = &case.test {
                        self.expression(test);
                    }
                    self.statements(&case.consequent);
                }
            }
            ParsedFunctionBodyStatement::Try(statement) => {
                self.statements(&statement.block);
                if let Some(handler) = &statement.handler {
                    self.statements(&handler.body);
                }
                self.statements(&statement.finalizer);
            }
            ParsedFunctionBodyStatement::Class(_) => self.other = true,
            _ => {}
        }
    }

    fn expression(&mut self, expression: &ParsedExpression) {
        match expression {
            ParsedExpression::Yield {
                operand, delegate, ..
            } => {
                let nullish = match (operand.as_deref(), *delegate) {
                    (None, _) => true,
                    (Some(ParsedExpression::UndefinedLiteral | ParsedExpression::NullLiteral), false) => {
                        true
                    }
                    (
                        Some(ParsedExpression::Unary {
                            operator: ParsedUnaryOperator::Void,
                            ..
                        }),
                        false,
                    ) => true,
                    (Some(ParsedExpression::Identifier { name, .. }), false) => name == "undefined",
                    (Some(ParsedExpression::ArrayLiteral { elements, .. }), true) => elements.is_empty(),
                    _ => false,
                };
                if nullish {
                    self.nullish += 1;
                } else {
                    self.other = true;
                }
            }
            ParsedExpression::ClassExpression(_) => self.other = true,
            _ => {}
        }
        expression.for_each_child(&mut |child| self.expression(child));
    }
}
