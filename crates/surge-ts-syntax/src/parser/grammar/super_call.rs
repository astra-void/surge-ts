//! tsc's `checkConstructorDeclaration` placement rules for `super()` (TS2376,
//! TS2401). The checker keeps them only while class fields are emitted with
//! [[Set]] semantics (`GetEmitStandardClassFields` off), where the emitted
//! constructor runs the initializers right after the `super()` call.

use oxc_ast::ast::{Class, ClassElement, Expression, FormalParameters, FunctionBody, PropertyKey, Statement};
use oxc_ast_visit::Visit;
use oxc_span::{GetSpan, Span};

use super::GrammarCollector;
use crate::ParsedGrammarDiagnosticKind as Kind;

impl GrammarCollector {
    /// `super()` must be a root-level statement, and the first statement to
    /// refer to `this` or `super`, when the derived class has initialized
    /// instance properties, private identifiers, or parameter properties.
    pub(super) fn check_super_call_placement(
        &mut self,
        class: &Class<'_>,
        constructor_key_span: Span,
        parameters: &FormalParameters<'_>,
        body: &FunctionBody<'_>,
    ) {
        let has_parameter_property = parameters
            .items
            .iter()
            .any(|parameter| parameter.accessibility.is_some() || parameter.readonly || parameter.r#override);
        if !has_parameter_property
            && !class
                .body
                .body
                .iter()
                .any(is_instance_property_with_initializer_or_private_identifier)
        {
            return;
        }
        let Some(super_call) = first_super_call(body) else {
            return;
        };
        if !super_call_is_root_level(super_call, body) {
            self.push(Kind::Ts(2401), super_call, None);
            return;
        }
        for statement in &body.statements {
            if let Statement::ExpressionStatement(expression) = statement
                && is_super_call(&expression.expression)
            {
                return;
            }
            if statement_references_super_or_this(statement) {
                break;
            }
        }
        self.push(Kind::Ts(2376), constructor_key_span, None);
    }
}

fn is_instance_property_with_initializer_or_private_identifier(element: &ClassElement<'_>) -> bool {
    match element {
        ClassElement::PropertyDefinition(property) => {
            matches!(property.key, PropertyKey::PrivateIdentifier(_))
                || (!property.r#static && property.value.is_some())
        }
        ClassElement::MethodDefinition(method) => matches!(method.key, PropertyKey::PrivateIdentifier(_)),
        ClassElement::AccessorProperty(accessor) => matches!(accessor.key, PropertyKey::PrivateIdentifier(_)),
        _ => false,
    }
}

fn is_super_call(expression: &Expression<'_>) -> bool {
    let mut expression = expression;
    while let Expression::ParenthesizedExpression(parenthesized) = expression {
        expression = &parenthesized.expression;
    }
    matches!(expression, Expression::CallExpression(call) if matches!(call.callee, Expression::Super(_)))
}

/// tsc's `findFirstSuperCall`: the first `super()` in source order, not
/// looking into nested functions.
fn first_super_call(body: &FunctionBody<'_>) -> Option<Span> {
    struct Finder {
        found: Option<Span>,
    }

    impl<'a> Visit<'a> for Finder {
        fn visit_call_expression(&mut self, call: &oxc_ast::ast::CallExpression<'a>) {
            if self.found.is_some() {
                return;
            }
            if matches!(call.callee, Expression::Super(_)) {
                self.found = Some(call.span);
                return;
            }
            oxc_ast_visit::walk::walk_call_expression(self, call);
        }
        fn visit_function(&mut self, _: &oxc_ast::ast::Function<'a>, _: oxc_syntax::scope::ScopeFlags) {}
        fn visit_arrow_function_expression(&mut self, _: &oxc_ast::ast::ArrowFunctionExpression<'a>) {}
    }

    let mut finder = Finder { found: None };
    finder.visit_function_body(body);
    finder.found
}

/// tsc's `superCallIsRootLevelInConstructor`: the call is, up to parentheses,
/// the whole of a statement directly in the body.
fn super_call_is_root_level(super_call: Span, body: &FunctionBody<'_>) -> bool {
    body.statements.iter().any(|statement| {
        let Statement::ExpressionStatement(expression) = statement else {
            return false;
        };
        let mut inner = &expression.expression;
        while let Expression::ParenthesizedExpression(parenthesized) = inner {
            inner = &parenthesized.expression;
        }
        inner.span() == super_call
    })
}

/// tsc's `nodeImmediatelyReferencesSuperOrThis`: a `this` or `super` in the
/// statement outside any nested function or class property.
fn statement_references_super_or_this(statement: &Statement<'_>) -> bool {
    struct Finder {
        found: bool,
    }

    impl<'a> Visit<'a> for Finder {
        fn visit_this_expression(&mut self, _: &oxc_ast::ast::ThisExpression) {
            self.found = true;
        }
        fn visit_super(&mut self, _: &oxc_ast::ast::Super) {
            self.found = true;
        }
        fn visit_function(&mut self, _: &oxc_ast::ast::Function<'a>, _: oxc_syntax::scope::ScopeFlags) {}
        fn visit_arrow_function_expression(&mut self, _: &oxc_ast::ast::ArrowFunctionExpression<'a>) {}
        fn visit_property_definition(&mut self, _: &oxc_ast::ast::PropertyDefinition<'a>) {}
    }

    let mut finder = Finder { found: false };
    finder.visit_statement(statement);
    finder.found
}
