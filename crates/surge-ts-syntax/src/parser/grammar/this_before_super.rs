//! tsc's `checkThisBeforeSuper` over a derived class's constructor: `this`, or
//! a `super.x` read, is an error unless every path from the constructor's
//! start to it runs a `super(...)` call (`isPostSuperFlowNode`).
//!
//! The binder makes a `FlowCall` of every `super(...)` call and joins
//! branches at labels; a label is post-super only when each reachable branch
//! into it is. A loop is post-super only when the path into it is, since its
//! label's first antecedent is the entry edge. Unreachable code is
//! post-super, which silences it. A nested function or class is a container
//! of its own and is not walked.

use oxc_ast::ast::{
    Argument, ArrayExpressionElement, AssignmentTarget, BindingPattern, Expression, ForStatementInit,
    ForStatementLeft, FormalParameters, FunctionBody, ObjectPropertyKind, PropertyKey, SimpleAssignmentTarget,
    Statement,
};
use oxc_span::Span;

use super::GrammarCollector;
use crate::ParsedGrammarDiagnosticKind as Kind;

#[derive(Clone, Copy)]
struct State {
    called: bool,
    reachable: bool,
}

impl State {
    const UNREACHABLE: State = State {
        called: true,
        reachable: false,
    };

    /// A branch label: post-super when every reachable antecedent is.
    fn join(self, other: State) -> State {
        match (self.reachable, other.reachable) {
            (false, _) => other,
            (_, false) => self,
            _ => State {
                called: self.called && other.called,
                reachable: true,
            },
        }
    }
}

#[derive(Default)]
struct SuperFlow {
    found: Vec<(Kind, Span)>,
    /// The states `break` carries out of each enclosing loop or `switch`.
    breaks: Vec<Vec<State>>,
}

impl GrammarCollector {
    pub(super) fn report_this_before_super(&mut self, parameters: &FormalParameters<'_>, body: &FunctionBody<'_>) {
        let mut flow = SuperFlow::default();
        let mut state = State {
            called: false,
            reachable: true,
        };
        for parameter in &parameters.items {
            state = flow.pattern(&parameter.pattern, state);
            if let Some(initializer) = &parameter.initializer {
                state = flow.expression(initializer, state);
            }
        }
        flow.statements(&body.statements, state);
        for (kind, span) in flow.found {
            self.push(kind, span, None);
        }
    }
}

impl SuperFlow {
    fn reference(&mut self, kind: Kind, span: Span, state: State) {
        if state.reachable && !state.called {
            self.found.push((kind, span));
        }
    }

    fn statements(&mut self, statements: &[Statement<'_>], mut state: State) -> State {
        for statement in statements {
            state = self.statement(statement, state);
        }
        state
    }

    fn with_breaks(&mut self, walk: impl FnOnce(&mut Self) -> State) -> (State, State) {
        self.breaks.push(Vec::new());
        let end = walk(self);
        let breaks = self.breaks.pop().unwrap_or_default();
        let broke = breaks.into_iter().fold(State::UNREACHABLE, State::join);
        (end, broke)
    }

    fn statement(&mut self, statement: &Statement<'_>, state: State) -> State {
        match statement {
            Statement::ExpressionStatement(statement) => self.expression(&statement.expression, state),
            Statement::VariableDeclaration(declaration) => {
                let mut state = state;
                for declarator in &declaration.declarations {
                    if let Some(init) = &declarator.init {
                        state = self.expression(init, state);
                    }
                    state = self.pattern(&declarator.id, state);
                }
                state
            }
            Statement::ReturnStatement(statement) => {
                if let Some(argument) = &statement.argument {
                    self.expression(argument, state);
                }
                State::UNREACHABLE
            }
            Statement::ThrowStatement(statement) => {
                self.expression(&statement.argument, state);
                State::UNREACHABLE
            }
            Statement::BlockStatement(block) => self.statements(&block.body, state),
            Statement::IfStatement(statement) => {
                let state = self.expression(&statement.test, state);
                let then_end = self.statement(&statement.consequent, state);
                let else_end = match &statement.alternate {
                    Some(alternate) => self.statement(alternate, state),
                    None => state,
                };
                then_end.join(else_end)
            }
            Statement::WhileStatement(statement) => {
                let state = self.expression(&statement.test, state);
                let (_, broke) = self.with_breaks(|flow| flow.statement(&statement.body, state));
                let exits = if is_true_literal(&statement.test) { State::UNREACHABLE } else { state };
                exits.join(broke)
            }
            Statement::DoWhileStatement(statement) => {
                let (end, broke) = self.with_breaks(|flow| flow.statement(&statement.body, state));
                let end = self.expression(&statement.test, end);
                let exits = if is_true_literal(&statement.test) { State::UNREACHABLE } else { end };
                exits.join(broke)
            }
            Statement::ForStatement(statement) => {
                let state = match &statement.init {
                    Some(ForStatementInit::VariableDeclaration(declaration)) => {
                        let mut state = state;
                        for declarator in &declaration.declarations {
                            if let Some(init) = &declarator.init {
                                state = self.expression(init, state);
                            }
                        }
                        state
                    }
                    Some(init) => match init.as_expression() {
                        Some(expression) => self.expression(expression, state),
                        None => state,
                    },
                    None => state,
                };
                let state = match &statement.test {
                    Some(test) => self.expression(test, state),
                    None => state,
                };
                let (end, broke) = self.with_breaks(|flow| flow.statement(&statement.body, state));
                if let Some(update) = &statement.update {
                    self.expression(update, end);
                }
                let exits = match &statement.test {
                    Some(test) if !is_true_literal(test) => state,
                    _ => State::UNREACHABLE,
                };
                exits.join(broke)
            }
            Statement::ForInStatement(statement) => {
                let state = self.expression(&statement.right, state);
                self.for_left(&statement.left, state);
                let (_, broke) = self.with_breaks(|flow| flow.statement(&statement.body, state));
                state.join(broke)
            }
            Statement::ForOfStatement(statement) => {
                let state = self.expression(&statement.right, state);
                self.for_left(&statement.left, state);
                let (_, broke) = self.with_breaks(|flow| flow.statement(&statement.body, state));
                state.join(broke)
            }
            Statement::SwitchStatement(statement) => {
                let state = self.expression(&statement.discriminant, state);
                let has_default = statement.cases.iter().any(|case| case.test.is_none());
                let (end, broke) = self.with_breaks(|flow| {
                    let mut fallthrough = State::UNREACHABLE;
                    for case in &statement.cases {
                        let matched = match &case.test {
                            Some(test) => flow.expression(test, state),
                            None => state,
                        };
                        let entry = matched.join(fallthrough);
                        fallthrough = flow.statements(&case.consequent, entry);
                    }
                    fallthrough
                });
                let unmatched = if has_default { State::UNREACHABLE } else { state };
                end.join(broke).join(unmatched)
            }
            Statement::LabeledStatement(statement) => self.statement(&statement.body, state),
            Statement::TryStatement(statement) => {
                let try_end = self.statements(&statement.block.body, state);
                let end = match &statement.handler {
                    Some(handler) => try_end.join(self.statements(&handler.body.body, state)),
                    None => try_end,
                };
                match &statement.finalizer {
                    Some(finalizer) => {
                        let finally_end = self.statements(&finalizer.body, state.join(end));
                        if end.reachable { finally_end } else { State::UNREACHABLE }
                    }
                    None => end,
                }
            }
            Statement::WithStatement(statement) => {
                let state = self.expression(&statement.object, state);
                self.statement(&statement.body, state)
            }
            Statement::BreakStatement(statement) => {
                if statement.label.is_none()
                    && let Some(breaks) = self.breaks.last_mut()
                {
                    breaks.push(state);
                }
                State::UNREACHABLE
            }
            Statement::ContinueStatement(_) => State::UNREACHABLE,
            _ => state,
        }
    }

    fn for_left(&mut self, left: &ForStatementLeft<'_>, state: State) {
        if let Some(target) = left.as_assignment_target() {
            self.assignment_target(target, state);
        }
    }

    /// Initializers and computed keys a binding pattern evaluates.
    fn pattern(&mut self, pattern: &BindingPattern<'_>, mut state: State) -> State {
        match pattern {
            BindingPattern::AssignmentPattern(assignment) => {
                state = self.pattern(&assignment.left, state);
                self.expression(&assignment.right, state)
            }
            BindingPattern::ObjectPattern(object) => {
                for property in &object.properties {
                    state = self.property_key(&property.key, state);
                    state = self.pattern(&property.value, state);
                }
                if let Some(rest) = &object.rest {
                    state = self.pattern(&rest.argument, state);
                }
                state
            }
            BindingPattern::ArrayPattern(array) => {
                for element in array.elements.iter().flatten() {
                    state = self.pattern(element, state);
                }
                if let Some(rest) = &array.rest {
                    state = self.pattern(&rest.argument, state);
                }
                state
            }
            BindingPattern::BindingIdentifier(_) => state,
        }
    }

    fn property_key(&mut self, key: &PropertyKey<'_>, state: State) -> State {
        match key.as_expression() {
            Some(expression) => self.expression(expression, state),
            None => state,
        }
    }

    fn assignment_target(&mut self, target: &AssignmentTarget<'_>, state: State) -> State {
        match target {
            AssignmentTarget::StaticMemberExpression(member) => self.member_object(&member.object, state),
            AssignmentTarget::ComputedMemberExpression(member) => {
                let state = self.member_object(&member.object, state);
                self.expression(&member.expression, state)
            }
            AssignmentTarget::PrivateFieldExpression(member) => self.member_object(&member.object, state),
            _ => match target.as_simple_assignment_target() {
                Some(SimpleAssignmentTarget::TSAsExpression(inner)) => self.expression(&inner.expression, state),
                Some(SimpleAssignmentTarget::TSSatisfiesExpression(inner)) => {
                    self.expression(&inner.expression, state)
                }
                Some(SimpleAssignmentTarget::TSNonNullExpression(inner)) => self.expression(&inner.expression, state),
                Some(SimpleAssignmentTarget::TSTypeAssertion(inner)) => self.expression(&inner.expression, state),
                _ => state,
            },
        }
    }

    /// The object of a member access: `super` there is a `super.x` read.
    fn member_object(&mut self, object: &Expression<'_>, state: State) -> State {
        if let Expression::Super(super_keyword) = object {
            self.reference(Kind::SuperPropertyBeforeSuperCall, super_keyword.span, state);
            return state;
        }
        self.expression(object, state)
    }

    fn arguments(&mut self, arguments: &[Argument<'_>], mut state: State) -> State {
        for argument in arguments {
            state = match argument {
                Argument::SpreadElement(spread) => self.expression(&spread.argument, state),
                other => match other.as_expression() {
                    Some(expression) => self.expression(expression, state),
                    None => state,
                },
            };
        }
        state
    }

    fn expression(&mut self, expression: &Expression<'_>, state: State) -> State {
        match expression {
            Expression::ThisExpression(this) => {
                self.reference(Kind::ThisBeforeSuperCall, this.span, state);
                state
            }
            Expression::CallExpression(call) => {
                if matches!(call.callee, Expression::Super(_)) {
                    let state = self.arguments(&call.arguments, state);
                    return State {
                        called: true,
                        ..state
                    };
                }
                let state = self.expression(&call.callee, state);
                self.arguments(&call.arguments, state)
            }
            Expression::NewExpression(new) => {
                let state = self.expression(&new.callee, state);
                self.arguments(&new.arguments, state)
            }
            Expression::StaticMemberExpression(member) => self.member_object(&member.object, state),
            Expression::PrivateFieldExpression(member) => self.member_object(&member.object, state),
            Expression::ComputedMemberExpression(member) => {
                let state = self.member_object(&member.object, state);
                self.expression(&member.expression, state)
            }
            Expression::ConditionalExpression(conditional) => {
                let state = self.expression(&conditional.test, state);
                let consequent = self.expression(&conditional.consequent, state);
                let alternate = self.expression(&conditional.alternate, state);
                consequent.join(alternate)
            }
            Expression::LogicalExpression(logical) => {
                let state = self.expression(&logical.left, state);
                let right = self.expression(&logical.right, state);
                state.join(right)
            }
            Expression::BinaryExpression(binary) => {
                let state = self.expression(&binary.left, state);
                self.expression(&binary.right, state)
            }
            Expression::AssignmentExpression(assignment) => {
                let state = self.assignment_target(&assignment.left, state);
                self.expression(&assignment.right, state)
            }
            Expression::SequenceExpression(sequence) => sequence
                .expressions
                .iter()
                .fold(state, |state, expression| self.expression(expression, state)),
            Expression::ParenthesizedExpression(inner) => self.expression(&inner.expression, state),
            Expression::UnaryExpression(unary) => self.expression(&unary.argument, state),
            Expression::AwaitExpression(inner) => self.expression(&inner.argument, state),
            Expression::YieldExpression(inner) => match &inner.argument {
                Some(argument) => self.expression(argument, state),
                None => state,
            },
            Expression::UpdateExpression(update) => match update.argument.as_member_expression() {
                Some(member) => {
                    let state = self.member_object(member.object(), state);
                    match member {
                        oxc_ast::ast::MemberExpression::ComputedMemberExpression(computed) => {
                            self.expression(&computed.expression, state)
                        }
                        _ => state,
                    }
                }
                None => state,
            },
            Expression::ChainExpression(chain) => match chain.expression.as_member_expression() {
                Some(member) => {
                    let state = self.member_object(member.object(), state);
                    match member {
                        oxc_ast::ast::MemberExpression::ComputedMemberExpression(computed) => {
                            self.expression(&computed.expression, state)
                        }
                        _ => state,
                    }
                }
                None => match &chain.expression {
                    oxc_ast::ast::ChainElement::CallExpression(call) => {
                        let state = self.expression(&call.callee, state);
                        self.arguments(&call.arguments, state)
                    }
                    _ => state,
                },
            },
            Expression::TSAsExpression(inner) => self.expression(&inner.expression, state),
            Expression::TSSatisfiesExpression(inner) => self.expression(&inner.expression, state),
            Expression::TSNonNullExpression(inner) => self.expression(&inner.expression, state),
            Expression::TSTypeAssertion(inner) => self.expression(&inner.expression, state),
            Expression::TSInstantiationExpression(inner) => self.expression(&inner.expression, state),
            Expression::TemplateLiteral(template) => template
                .expressions
                .iter()
                .fold(state, |state, expression| self.expression(expression, state)),
            Expression::TaggedTemplateExpression(tagged) => {
                let state = self.expression(&tagged.tag, state);
                tagged
                    .quasi
                    .expressions
                    .iter()
                    .fold(state, |state, expression| self.expression(expression, state))
            }
            Expression::ArrayExpression(array) => {
                let mut state = state;
                for element in &array.elements {
                    state = match element {
                        ArrayExpressionElement::SpreadElement(spread) => self.expression(&spread.argument, state),
                        ArrayExpressionElement::Elision(_) => state,
                        other => match other.as_expression() {
                            Some(expression) => self.expression(expression, state),
                            None => state,
                        },
                    };
                }
                state
            }
            Expression::ObjectExpression(object) => {
                let mut state = state;
                for property in &object.properties {
                    state = match property {
                        ObjectPropertyKind::ObjectProperty(property) => {
                            let state = self.property_key(&property.key, state);
                            if property.method || property.kind != oxc_ast::ast::PropertyKind::Init {
                                state
                            } else {
                                self.expression(&property.value, state)
                            }
                        }
                        ObjectPropertyKind::SpreadProperty(spread) => self.expression(&spread.argument, state),
                    };
                }
                state
            }
            Expression::ImportExpression(import) => {
                let state = self.expression(&import.source, state);
                match &import.options {
                    Some(options) => self.expression(options, state),
                    None => state,
                }
            }
            _ => state,
        }
    }
}

fn is_true_literal(expression: &Expression<'_>) -> bool {
    matches!(expression.without_parentheses(), Expression::BooleanLiteral(literal) if literal.value)
}
