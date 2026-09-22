use oxc_ast::ast::{
    ArrayPattern, AssignmentOperator, AssignmentTarget, BindingPattern, BindingProperty,
    Declaration, Expression, ExpressionStatement, ModuleDeclaration, ObjectPattern, PropertyKey,
    Statement, TSGlobalDeclaration, TSModuleDeclaration, TSModuleDeclarationBody,
    TSModuleDeclarationName, VariableDeclaration, VariableDeclarationKind,
};

use crate::{
    ParsedAssignment, ParsedDeclareModuleDeclaration, ParsedExportDeclaration, ParsedExpression,
    ParsedNamespaceDeclaration, ParsedStatement, ParsedVariableDeclaration, ParsedVariableKind,
};

mod classes;
mod entry;
mod enums;
mod exports;
mod expressions;
mod function_types;
mod functions;
mod grammar;
mod grammar_context;
mod import_calls;
mod imports;
mod interfaces;
mod json;
mod reads;
mod reference_directives;
mod spans;
mod suppressions;
mod types;
mod let_assignments;
mod writes;

use self::classes::parse_class_declaration;
use self::exports::parse_export_named_declaration;
use self::exports::{
    parse_export_all_declaration, parse_export_assignment, parse_export_default_declaration,
};
use self::expressions::{
    parse_call_expression, parse_conditional_expression, parse_expression,
    parse_static_member_expression, parse_unary_expression,
};
use self::functions::parse_function_declaration;
use self::imports::{parse_import_declarations, parse_import_equals_declaration};
use self::interfaces::parse_interface_declaration;
pub use self::reference_directives::{
    extract_reference_path_directives, extract_reference_type_directives,
};
use self::spans::text_span_from_oxc_span;
use self::types::{parse_type_alias_declaration, parse_type_annotation};
pub use entry::{ParserWorker, parse_source};
pub use json::{is_json_file_name, parse_json_module_type};

fn parse_statement(statement: &Statement<'_>) -> Option<Vec<ParsedStatement>> {
    if let Some(module_declaration) = statement.as_module_declaration() {
        return parse_module_declaration(module_declaration);
    }

    if let Some(declaration) = statement.as_declaration() {
        return parse_declaration(declaration);
    }

    match statement {
        Statement::ExpressionStatement(expression_statement) => {
            let destructured = parse_destructuring_assignment(&expression_statement.expression);
            if !destructured.is_empty() {
                return Some(
                    destructured
                        .into_iter()
                        .map(|assignment| ParsedStatement::Assignment(Box::new(assignment)))
                        .collect(),
                );
            }
            parse_expression_statement(expression_statement).map(|statement| vec![statement])
        }
        Statement::IfStatement(if_statement) => functions::parse_if_statement(if_statement)
            .map(|if_statement| vec![ParsedStatement::If(Box::new(if_statement))]),
        Statement::BlockStatement(_)
        | Statement::ForStatement(_)
        | Statement::ForInStatement(_)
        | Statement::ForOfStatement(_)
        | Statement::WhileStatement(_)
        | Statement::DoWhileStatement(_)
        | Statement::SwitchStatement(_)
        | Statement::TryStatement(_)
        | Statement::ThrowStatement(_)
        | Statement::LabeledStatement(_) => functions::parse_function_body_statement(statement)
            .map(|statements| vec![ParsedStatement::Block(statements)]),
        _ => None,
    }
}

fn parse_module_declaration(
    module_declaration: &ModuleDeclaration<'_>,
) -> Option<Vec<ParsedStatement>> {
    match module_declaration {
        ModuleDeclaration::ImportDeclaration(import) => parse_import_declarations(import).map(|imports| {
            imports
                .into_iter()
                .map(|import| ParsedStatement::ImportDeclaration(Box::new(import)))
                .collect()
        }),
        ModuleDeclaration::ExportNamedDeclaration(export) => parse_export_named_declaration(export),
        ModuleDeclaration::ExportDefaultDeclaration(export) => {
            parse_export_default_declaration(export)
        }
        ModuleDeclaration::ExportAllDeclaration(export) => parse_export_all_declaration(export),
        ModuleDeclaration::TSExportAssignment(export) => parse_export_assignment(export),
        ModuleDeclaration::TSNamespaceExportDeclaration(export) => {
            Some(vec![ParsedStatement::ExportDeclaration(Box::new(
                ParsedExportDeclaration::NamespaceExport {
                    exported_name: export.id.name.to_string(),
                    exported_name_span: Some(text_span_from_oxc_span(export.id.span)),
                    span: Some(text_span_from_oxc_span(export.span)),
                },
            ))])
        }
    }
}

fn parse_declaration(declaration: &Declaration<'_>) -> Option<Vec<ParsedStatement>> {
    // Exhaustive against today's oxc, but the fallback arm stays: a new oxc
    // `Declaration` variant must degrade to `UnsupportedDeclaration`, not fail
    // the build.
    #[allow(unreachable_patterns)]
    match declaration {
        Declaration::VariableDeclaration(declaration) => {
            Some(parse_variable_declaration(declaration))
        }
        Declaration::FunctionDeclaration(function) => parse_function_declaration(function)
            .map(|function| vec![ParsedStatement::FunctionDeclaration(Box::new(function))]),
        Declaration::TSTypeAliasDeclaration(type_alias) => parse_type_alias_declaration(type_alias)
            .map(|type_alias| vec![ParsedStatement::TypeAliasDeclaration(Box::new(type_alias))]),
        Declaration::TSInterfaceDeclaration(interface) => parse_interface_declaration(interface)
            .map(|interface| vec![ParsedStatement::InterfaceDeclaration(Box::new(interface))]),
        Declaration::ClassDeclaration(class) => parse_class_declaration(class)
            .map(|class| vec![ParsedStatement::ClassDeclaration(Box::new(class))]),
        Declaration::TSEnumDeclaration(enum_declaration) => {
            Some(enums::parse_enum_declaration(enum_declaration, false))
        }
        Declaration::TSModuleDeclaration(module) => Some(parse_ts_module_declaration(module)),
        Declaration::TSGlobalDeclaration(global) => Some(parse_ts_global_declaration(global)),
        Declaration::TSImportEqualsDeclaration(import_equals) => {
            parse_import_equals_declaration(import_equals)
                .map(|import| vec![ParsedStatement::ImportDeclaration(Box::new(import))])
        }
        _ => Some(vec![ParsedStatement::UnsupportedDeclaration {
            span: Some(text_span_from_oxc_span(oxc_span::GetSpan::span(
                declaration,
            ))),
        }]),
    }
}

fn parse_variable_declaration(declaration: &VariableDeclaration<'_>) -> Vec<ParsedStatement> {
    let kind = match declaration.kind {
        VariableDeclarationKind::Var => ParsedVariableKind::Var,
        VariableDeclarationKind::Let => ParsedVariableKind::Let,
        VariableDeclarationKind::Const => ParsedVariableKind::Const,
        _ => ParsedVariableKind::Var,
    };

    declaration
        .declarations
        .iter()
        .flat_map(|declarator| {
            let declared_type = match (&declarator.id, declarator.type_annotation.as_ref()) {
                (BindingPattern::BindingIdentifier(identifier), Some(annotation))
                    if matches!(
                        &annotation.type_annotation,
                        oxc_ast::ast::TSType::TSTypeOperatorType(operator)
                            if operator.operator == oxc_ast::ast::TSTypeOperatorOperator::Unique
                    ) =>
                {
                    Some(crate::ParsedType::UniqueSymbol(std::sync::Arc::from(
                        identifier.name.as_str(),
                    )))
                }
                (_, annotation) => {
                    annotation.and_then(|annotation| parse_type_annotation(annotation))
                }
            };
            let Some(init) = declarator.init.as_ref() else {
                return parse_binding_pattern_declarations_with_definite(
                    &declarator.id,
                    None,
                    None,
                    declaration.declare,
                    kind,
                    declared_type,
                    declarator.definite,
                );
            };

            let (initializer, initializer_span) = parse_expression(init);
            let initializer_span = Some(text_span_from_oxc_span(initializer_span));

            parse_binding_pattern_declarations(
                &declarator.id,
                Some(initializer),
                initializer_span,
                declaration.declare,
                kind,
                declared_type,
            )
        })
        .collect()
}

fn parse_expression_statement(
    expression_statement: &ExpressionStatement<'_>,
) -> Option<ParsedStatement> {
    match &expression_statement.expression {
        Expression::CallExpression(_) => {
            if let Some(call) = parse_call_expression(match &expression_statement.expression {
                Expression::CallExpression(call_expression) => call_expression,
                _ => unreachable!(),
            }) {
                return Some(ParsedStatement::Call(Box::new(call)));
            }

            let (expression, _) = parse_expression(&expression_statement.expression);
            Some(ParsedStatement::Expression(Box::new(expression)))
        }
        Expression::AssignmentExpression(assignment) => {
            if let Some(assignment) = parse_assignment_expression(assignment) {
                return Some(ParsedStatement::Assignment(Box::new(assignment)));
            }
            // A write to a member or element, which the identifier-target
            // parser above does not accept. Dropping it left every module-scope
            // `o.a = …` and `o[k] = …` unchecked.
            functions::parse_member_assignment(assignment)
                .map(|assignment| ParsedStatement::MemberAssignment(Box::new(assignment)))
        }
        Expression::UnaryExpression(unary_expression) => parse_unary_expression(unary_expression)
            .map(|expression| ParsedStatement::Expression(Box::new(expression))),
        Expression::ConditionalExpression(conditional_expression) => {
            parse_conditional_expression(conditional_expression)
                .map(|expression| ParsedStatement::Expression(Box::new(expression)))
        }
        Expression::StaticMemberExpression(member_expression) => {
            parse_static_member_expression(member_expression)
                .map(|expression| ParsedStatement::Expression(Box::new(expression)))
        }
        _ => {
            let (expression, _) = parse_expression(&expression_statement.expression);
            Some(ParsedStatement::Expression(Box::new(expression)))
        }
    }
}


/// The value a logical assignment stores: `x ??= v` is `x = x ?? v`, and
/// `||=`/`&&=` likewise, so the ordinary assignment check and the narrowing it
/// establishes apply. Other compound operators are not modelled and yield
/// `None`.
pub(super) fn logical_assignment_value(
    operator: AssignmentOperator,
    target: ParsedExpression,
    target_span: Option<crate::TextSpan>,
    value: ParsedExpression,
    value_span: Option<crate::TextSpan>,
) -> Option<ParsedExpression> {
    let logical = match operator {
        AssignmentOperator::Assign => return Some(value),
        AssignmentOperator::LogicalNullish => {
            return Some(ParsedExpression::NullishCoalescing {
                left: Box::new(target),
                left_span: target_span,
                right: Box::new(value),
                right_span: value_span,
            });
        }
        AssignmentOperator::LogicalOr => crate::ParsedLogicalOperator::Or,
        AssignmentOperator::LogicalAnd => crate::ParsedLogicalOperator::And,
        // `x op= v` is `x = x op v`, which is how tsc checks it too: the
        // operator's own result type is what the assignment is then checked
        // against (`checkBinaryLikeExpression` feeds `checkAssignmentOperator`).
        other => {
            let binary = compound_assignment_operator(other)?;
            return Some(ParsedExpression::Binary {
                left: Box::new(target),
                left_span: target_span,
                operator: binary,
                operator_span: None,
                right: Box::new(value),
                right_span: value_span,
            });
        }
    };
    Some(ParsedExpression::Logical {
        left: Box::new(target),
        left_span: target_span,
        operator: logical,
        operator_span: None,
        right: Box::new(value),
        right_span: value_span,
    })
}

/// The binary operator a compound assignment applies.
fn compound_assignment_operator(
    operator: AssignmentOperator,
) -> Option<crate::ParsedBinaryOperator> {
    use crate::ParsedBinaryOperator as B;
    Some(match operator {
        AssignmentOperator::Addition => B::Add,
        AssignmentOperator::Subtraction => B::Subtract,
        AssignmentOperator::Multiplication => B::Multiply,
        AssignmentOperator::Division => B::Divide,
        AssignmentOperator::Remainder => B::Remainder,
        AssignmentOperator::Exponential => B::Exponential,
        AssignmentOperator::ShiftLeft => B::ShiftLeft,
        AssignmentOperator::ShiftRight => B::ShiftRight,
        AssignmentOperator::ShiftRightZeroFill => B::ShiftRightZeroFill,
        AssignmentOperator::BitwiseOR => B::BitwiseOR,
        AssignmentOperator::BitwiseXOR => B::BitwiseXOR,
        AssignmentOperator::BitwiseAnd => B::BitwiseAnd,
        _ => return None,
    })
}

fn parse_assignment_expression(
    assignment: &oxc_ast::ast::AssignmentExpression<'_>,
) -> Option<ParsedAssignment> {
    let AssignmentTarget::AssignmentTargetIdentifier(identifier) = &assignment.left else {
        return None;
    };

    let target_span = Some(text_span_from_oxc_span(identifier.span));
    let (value, value_span) = parse_expression(&assignment.right);
    let value_span = Some(text_span_from_oxc_span(value_span));
    let target = ParsedExpression::Identifier {
        name: identifier.name.to_string(),
        span: target_span,
    };
    let value = logical_assignment_value(assignment.operator, target, target_span, value, value_span)?;

    // tsc reports on the target as written, parentheses included: `(x) = ''`.
    let written_target_span = Some(crate::TextSpan {
        start: assignment.span.start as usize,
        end: identifier.span.end as usize,
    });
    Some(ParsedAssignment {
        target_name: identifier.name.to_string(),
        target_span,
        written_target_span,
        value,
        value_span,
    })
}

/// `[a, b] = source` / `({ a, b: c } = source)`: each identifier target is an
/// ordinary assignment of the element or property it reads (with its default
/// applied as `??`, as a binding pattern does), so the checker reports each write
/// on its target the way tsc's `checkDestructuringAssignment` does. Nested
/// patterns and member targets are not lowered.
pub(crate) fn parse_destructuring_assignment(expression: &Expression<'_>) -> Vec<ParsedAssignment> {
    use oxc_ast::ast::{AssignmentTargetMaybeDefault, AssignmentTargetProperty};

    let Expression::AssignmentExpression(assignment) = expression.without_parentheses() else {
        return Vec::new();
    };
    if assignment.operator != oxc_syntax::operator::AssignmentOperator::Assign {
        return Vec::new();
    }
    let (source, source_span) = parse_expression(&assignment.right);
    let source_span = Some(text_span_from_oxc_span(source_span));

    let element_read = |index: usize, target_span: Option<crate::TextSpan>| match &source {
        // An array literal source is contextually a tuple: each target takes
        // its own element, not the union of all of them.
        ParsedExpression::ArrayLiteral { elements, .. }
            if elements.iter().take(index + 1).all(|element| !element.spread)
                && index < elements.len() =>
        {
            elements[index].expression.clone()
        }
        ParsedExpression::Identifier { name, .. } => ParsedExpression::IndexAccess {
            object_name: name.clone(),
            object_span: source_span,
            index: Box::new(ParsedExpression::NumberLiteral(index.to_string())),
            index_span: target_span,
        },
        _ => ParsedExpression::ElementAccess {
            object: Box::new(source.clone()),
            object_span: source_span,
            index: Box::new(ParsedExpression::NumberLiteral(index.to_string())),
            index_span: target_span,
        },
    };
    let with_default = |read: ParsedExpression, default: Option<&Expression<'_>>| match default {
        Some(default) => {
            let (default_value, default_span) = parse_expression(default);
            ParsedExpression::NullishCoalescing {
                left: Box::new(read),
                left_span: source_span,
                right: Box::new(default_value),
                right_span: Some(text_span_from_oxc_span(default_span)),
            }
        }
        None => read,
    };
    let lowered = |identifier: &oxc_ast::ast::IdentifierReference<'_>, value: ParsedExpression| {
        ParsedAssignment {
            target_name: identifier.name.to_string(),
            target_span: Some(text_span_from_oxc_span(identifier.span)),
            written_target_span: Some(text_span_from_oxc_span(identifier.span)),
            value,
            value_span: source_span,
        }
    };

    let mut assignments = Vec::new();
    match &assignment.left {
        AssignmentTarget::ArrayAssignmentTarget(pattern) => {
            for (index, element) in pattern.elements.iter().enumerate() {
                let (target, default) = match element {
                    Some(AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(with_default)) => {
                        (&with_default.binding, Some(&with_default.init))
                    }
                    Some(other) => match other.as_assignment_target() {
                        Some(target) => (target, None),
                        None => continue,
                    },
                    None => continue,
                };
                let AssignmentTarget::AssignmentTargetIdentifier(identifier) = target else {
                    continue;
                };
                let span = Some(text_span_from_oxc_span(identifier.span));
                assignments.push(lowered(identifier, with_default(element_read(index, span), default)));
            }
        }
        AssignmentTarget::ObjectAssignmentTarget(pattern) => {
            for property in &pattern.properties {
                let (identifier, name, default) = match property {
                    AssignmentTargetProperty::AssignmentTargetPropertyIdentifier(shorthand) => (
                        &shorthand.binding,
                        shorthand.binding.name.to_string(),
                        shorthand.init.as_ref(),
                    ),
                    AssignmentTargetProperty::AssignmentTargetPropertyProperty(property) => {
                        let Some(name) = property.name.static_name() else {
                            continue;
                        };
                        let (target, default) = match &property.binding {
                            AssignmentTargetMaybeDefault::AssignmentTargetWithDefault(with_default) => {
                                (&with_default.binding, Some(&with_default.init))
                            }
                            other => match other.as_assignment_target() {
                                Some(target) => (target, None),
                                None => continue,
                            },
                        };
                        let AssignmentTarget::AssignmentTargetIdentifier(identifier) = target else {
                            continue;
                        };
                        (identifier.as_ref(), name.to_string(), default)
                    }
                };
                let span = Some(text_span_from_oxc_span(identifier.span));
                let read = ParsedExpression::PropertyAccess {
                    object: Box::new(source.clone()),
                    object_span: source_span,
                    property_name: name,
                    property_span: span,
                    is_bracketed: false,
                };
                assignments.push(lowered(identifier, with_default(read, default)));
            }
        }
        _ => {}
    }
    assignments
}

fn parse_binding_pattern_declarations(
    binding: &BindingPattern<'_>,
    initializer: Option<ParsedExpression>,
    initializer_span: Option<crate::TextSpan>,
    is_declare: bool,
    kind: ParsedVariableKind,
    declared_type: Option<crate::ParsedType>,
) -> Vec<ParsedStatement> {
    parse_binding_pattern_declarations_with_definite(
        binding,
        initializer,
        initializer_span,
        is_declare,
        kind,
        declared_type,
        false,
    )
}

#[allow(clippy::too_many_arguments)]
fn parse_binding_pattern_declarations_with_definite(
    binding: &BindingPattern<'_>,
    initializer: Option<ParsedExpression>,
    initializer_span: Option<crate::TextSpan>,
    is_declare: bool,
    kind: ParsedVariableKind,
    declared_type: Option<crate::ParsedType>,
    has_definite_assertion: bool,
) -> Vec<ParsedStatement> {
    match binding {
        BindingPattern::BindingIdentifier(binding_identifier) => {
            vec![ParsedStatement::VariableDeclaration(Box::new(
                ParsedVariableDeclaration {
                    is_enum_object: false,
                    is_declare,
                    kind,
                    from_binding_pattern: false,
                    has_definite_assertion,
                    array_pattern_span: None,
                    name: binding_identifier.name.to_string(),
                    name_span: Some(text_span_from_oxc_span(binding_identifier.span)),
                    declared_type,
                    initializer,
                    initializer_span,
                },
            ))]
        }
        BindingPattern::AssignmentPattern(assignment_pattern) => {
            // `const { a = 0 } = o` binds `o.a ?? 0`: the default applies exactly
            // when the property is absent, so the binding is never `undefined`.
            let initializer = match initializer {
                Some(initializer) => {
                    let (default_value, default_span) = parse_expression(&assignment_pattern.right);
                    Some(ParsedExpression::NullishCoalescing {
                        left: Box::new(initializer),
                        left_span: initializer_span,
                        right: Box::new(default_value),
                        right_span: Some(text_span_from_oxc_span(default_span)),
                    })
                }
                None => None,
            };
            parse_binding_pattern_declarations_with_definite(
                &assignment_pattern.left,
                initializer,
                initializer_span,
                is_declare,
                kind,
                declared_type,
                has_definite_assertion,
            )
        }
        BindingPattern::ObjectPattern(object_pattern) => {
            mark_binding_pattern_declarations(parse_object_pattern_declarations(
                object_pattern,
                initializer,
                initializer_span,
                is_declare,
                kind,
            ))
        }
        BindingPattern::ArrayPattern(array_pattern) => {
            let pattern_span = text_span_from_oxc_span(array_pattern.span);
            let declarations = parse_array_pattern_declarations(
                array_pattern,
                initializer,
                initializer_span,
                is_declare,
                kind,
            )
            .into_iter()
            .map(|statement| match statement {
                ParsedStatement::VariableDeclaration(mut variable)
                    if variable.array_pattern_span.is_none() =>
                {
                    variable.array_pattern_span = Some(pattern_span);
                    ParsedStatement::VariableDeclaration(variable)
                }
                other => other,
            })
            .collect();
            mark_binding_pattern_declarations(declarations)
        }
    }
}

/// Flags every binding produced by a destructuring pattern, including the ones
/// nested inside it.
fn mark_binding_pattern_declarations(statements: Vec<ParsedStatement>) -> Vec<ParsedStatement> {
    statements
        .into_iter()
        .map(|statement| match statement {
            ParsedStatement::VariableDeclaration(mut variable) => {
                variable.from_binding_pattern = true;
                ParsedStatement::VariableDeclaration(variable)
            }
            other => other,
        })
        .collect()
}

fn parse_object_pattern_declarations(
    object_pattern: &ObjectPattern<'_>,
    initializer: Option<ParsedExpression>,
    initializer_span: Option<crate::TextSpan>,
    is_declare: bool,
    kind: ParsedVariableKind,
) -> Vec<ParsedStatement> {
    let Some(initializer) = initializer else {
        return Vec::new();
    };

    let mut declarations = Vec::new();

    for property in &object_pattern.properties {
        declarations.extend(parse_object_binding_property_declarations(
            property,
            initializer.clone(),
            initializer_span,
            is_declare,
            kind,
        ));
    }

    // `const { a, ...rest } = obj` binds `rest` to the remaining properties.
    // A computed key may name any property, so only a pattern of static keys
    // knows what the rest omits.
    if let Some(rest) = object_pattern.rest.as_deref() {
        let omitted: Option<Vec<String>> = object_pattern
            .properties
            .iter()
            .map(|property| match &property.key {
                PropertyKey::StaticIdentifier(identifier) => Some(identifier.name.to_string()),
                PropertyKey::StringLiteral(literal) => Some(literal.value.to_string()),
                _ => None,
            })
            .collect();
        let initializer = match omitted {
            Some(omitted) => ParsedExpression::ObjectRest {
                source: Box::new(initializer),
                omitted,
            },
            None => initializer,
        };
        declarations.extend(parse_binding_pattern_declarations(
            &rest.argument,
            Some(initializer),
            initializer_span,
            is_declare,
            kind,
            None,
        ));
    }

    declarations
}

fn parse_object_binding_property_declarations(
    property: &BindingProperty<'_>,
    source_initializer: ParsedExpression,
    source_initializer_span: Option<crate::TextSpan>,
    is_declare: bool,
    kind: ParsedVariableKind,
) -> Vec<ParsedStatement> {
    let PropertyKey::StaticIdentifier(identifier) = &property.key else {
        return Vec::new();
    };

    // An object literal initializer is contextually typed by the pattern, whose
    // defaulted elements are optional properties (`getTypeFromBindingPattern`),
    // so a defaulted name the literal does not write reads `undefined` rather
    // than a missing property.
    if matches!(property.value, BindingPattern::AssignmentPattern(_))
        && static_object_literal(&source_initializer).is_some_and(|properties| {
            literal_lacks_property(properties, identifier.name.as_str())
        })
    {
        return parse_binding_pattern_declarations(
            &property.value,
            Some(ParsedExpression::UndefinedLiteral),
            Some(text_span_from_oxc_span(identifier.span)),
            is_declare,
            kind,
            None,
        );
    }

    // Marked bracketed: this access is synthesized from a binding pattern, and
    // tsc does not apply `noPropertyAccessFromIndexSignature` (TS4111) to
    // destructuring — only to written dotted accesses.
    let property_initializer = ParsedExpression::PropertyAccess {
        object: Box::new(source_initializer),
        object_span: source_initializer_span,
        property_name: identifier.name.to_string(),
        property_span: Some(text_span_from_oxc_span(identifier.span)),
        is_bracketed: true,
    };

    parse_binding_pattern_declarations(
        &property.value,
        Some(property_initializer),
        Some(text_span_from_oxc_span(identifier.span)),
        is_declare,
        kind,
        None,
    )
}

/// The properties of the object literal `expression` statically evaluates to:
/// the literal itself, a property of one whose value is a literal, or the
/// default an absent value falls back to.
fn static_object_literal(expression: &ParsedExpression) -> Option<&[crate::ParsedObjectProperty]> {
    match expression {
        ParsedExpression::ObjectLiteral { properties, .. } => Some(properties),
        ParsedExpression::ConstAssertion { expression, .. } => static_object_literal(expression),
        ParsedExpression::PropertyAccess {
            object,
            property_name,
            ..
        } => {
            let properties = static_object_literal(object)?;
            let property = properties
                .iter()
                .find(|property| property.name == *property_name && !property.is_spread)?;
            static_object_literal(&property.value)
        }
        ParsedExpression::NullishCoalescing { left, right, .. } => match left.as_ref() {
            ParsedExpression::UndefinedLiteral => static_object_literal(right),
            left => static_object_literal(left),
        },
        _ => None,
    }
}

/// Whether a literal certainly does not write `name`: a spread or a computed key
/// could supply it.
fn literal_lacks_property(properties: &[crate::ParsedObjectProperty], name: &str) -> bool {
    properties
        .iter()
        .all(|property| !property.is_spread && property.computed_key.is_none() && property.name != name)
}

fn parse_array_pattern_declarations(
    array_pattern: &ArrayPattern<'_>,
    initializer: Option<ParsedExpression>,
    initializer_span: Option<crate::TextSpan>,
    is_declare: bool,
    kind: ParsedVariableKind,
) -> Vec<ParsedStatement> {
    let Some(initializer) = initializer else {
        return Vec::new();
    };

    let mut declarations = Vec::new();

    for (index, element) in array_pattern.elements.iter().enumerate() {
        let Some(element) = element else {
            continue;
        };

        // tsc reports an element the source lacks on the binding element itself.
        let element_span = Some(text_span_from_oxc_span(oxc_span::GetSpan::span(element)));
        let element_initializer = match &initializer {
            ParsedExpression::Identifier { name, .. } => ParsedExpression::IndexAccess {
                object_name: name.clone(),
                object_span: initializer_span,
                index: Box::new(ParsedExpression::NumberLiteral(index.to_string())),
                index_span: element_span,
            },
            // A non-identifier initializer (`const [a, b] = useState()`) indexes
            // the source expression directly so each binding gets its own element
            // type (e.g. the `Dispatch` setter), instead of the whole source.
            _ => ParsedExpression::ElementAccess {
                object: Box::new(initializer.clone()),
                object_span: initializer_span,
                index: Box::new(ParsedExpression::NumberLiteral(index.to_string())),
                index_span: element_span,
            },
        };

        declarations.extend(parse_binding_pattern_declarations(
            element,
            Some(element_initializer),
            initializer_span,
            is_declare,
            kind,
            None,
        ));
    }

    // `const [a, ...rest] = xs` binds `rest` to the remaining elements. Bound to
    // the whole initializer, the same shape the object-pattern sibling uses: for
    // an array source that is already the right element type. A tuple source is
    // over-wide here (tsc slices), which is still far better than leaving the
    // name unbound and reporting it as missing everywhere it is used.
    if let Some(rest) = array_pattern.rest.as_deref() {
        declarations.extend(parse_binding_pattern_declarations(
            &rest.argument,
            Some(initializer.clone()),
            initializer_span,
            is_declare,
            kind,
            None,
        ));
    }

    declarations
}

pub(crate) fn parse_ts_module_declaration(
    module: &TSModuleDeclaration<'_>,
) -> Vec<ParsedStatement> {
    use oxc_span::GetSpan;
    let module_specifier = match &module.id {
        TSModuleDeclarationName::StringLiteral(literal) => literal.value.to_string(),
        // `namespace JSX { ... }` / `module Foo { ... }` (identifier-named). The body
        // is preserved so the checker can resolve qualified members such as
        // `JSX.IntrinsicElements`.
        TSModuleDeclarationName::Identifier(identifier) => {
            return parse_ts_namespace_declaration(
                identifier.name.to_string(),
                Some(text_span_from_oxc_span(identifier.span)),
                module,
            );
        }
    };

    let statements = match &module.body {
        Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
            let mut statements: Vec<ParsedStatement> = block
                .body
                .iter()
                .filter_map(parse_statement)
                .flatten()
                .collect();
            enums::merge_lowered_enum_declarations(&mut statements);
            statements
        }
        _ => {
            return vec![ParsedStatement::UnsupportedDeclaration {
                span: Some(text_span_from_oxc_span(module.span)),
            }];
        }
    };

    vec![ParsedStatement::DeclareModuleDeclaration(Box::new(
        ParsedDeclareModuleDeclaration {
            module_specifier,
            module_specifier_span: Some(text_span_from_oxc_span(module.id.span())),
            statements,
            span: Some(text_span_from_oxc_span(module.span)),
        },
    ))]
}

fn parse_ts_namespace_declaration(
    name: String,
    name_span: Option<crate::TextSpan>,
    module: &TSModuleDeclaration<'_>,
) -> Vec<ParsedStatement> {
    let statements = match &module.body {
        Some(TSModuleDeclarationBody::TSModuleBlock(block)) => {
            let mut statements: Vec<ParsedStatement> = block
                .body
                .iter()
                .filter_map(parse_statement)
                .flatten()
                .collect();
            enums::merge_lowered_enum_declarations(&mut statements);
            statements
        }
        // `namespace A.B { ... }` nests as a module body; flatten it into a
        // dotted-name namespace so members resolve as `A.B.Member`.
        Some(TSModuleDeclarationBody::TSModuleDeclaration(inner)) => {
            let inner_name = match &inner.id {
                TSModuleDeclarationName::Identifier(identifier) => identifier.name.to_string(),
                TSModuleDeclarationName::StringLiteral(literal) => literal.value.to_string(),
            };
            let mut nested =
                parse_ts_namespace_declaration(format!("{name}.{inner_name}"), name_span, inner);
            if module.declare {
                for statement in &mut nested {
                    if let ParsedStatement::NamespaceDeclaration(namespace) = statement {
                        namespace.is_declare = true;
                    }
                }
            }
            nested
        }
        None => Vec::new(),
    };

    vec![ParsedStatement::NamespaceDeclaration(Box::new(
        ParsedNamespaceDeclaration {
            name,
            is_declare: module.declare,
            name_span,
            statements,
            span: Some(text_span_from_oxc_span(module.span)),
        },
    ))]
}

fn parse_ts_global_declaration(global: &TSGlobalDeclaration<'_>) -> Vec<ParsedStatement> {
    let mut statements: Vec<ParsedStatement> = global
        .body
        .body
        .iter()
        .filter_map(parse_statement)
        .flatten()
        .collect();
    enums::merge_lowered_enum_declarations(&mut statements);

    vec![ParsedStatement::DeclareModuleDeclaration(Box::new(
        ParsedDeclareModuleDeclaration {
            module_specifier: "global".to_string(),
            module_specifier_span: Some(text_span_from_oxc_span(global.global_span)),
            statements,
            span: Some(text_span_from_oxc_span(global.span)),
        },
    ))]
}
