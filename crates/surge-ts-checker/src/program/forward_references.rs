//! Class used before its declaration (TS2449), ported from
//! `checkResolvedBlockScopedVariable` / `isBlockScopedNameDeclaredBeforeUse` in
//! `tsc/internal/checker/checker.go`.
//!
//! A class binding is in its temporal dead zone until the declaration is
//! evaluated, so a reference that runs *before* it is an error — but only a
//! reference that actually runs then. tsc expresses that with
//! `isUsedInFunctionOrInstanceProperty`, which walks up from the use site and
//! quits at the first function-like ancestor: anything inside a function body
//! is deferred and legal however early it appears. The walk here mirrors that
//! by never descending into a function or arrow body.
//!
//! Type positions are exempt for the same reason (`isInAmbientOrTypeNode`), and
//! come for free: annotations are a separate tree from the expressions walked
//! here.

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{
    ParsedClassDeclaration, ParsedClassMember, ParsedExportDeclaration, ParsedExpression,
    ParsedStatement, TextSpan,
};

use crate::context::{CheckerContext, convert_span};
use surge_ts_types::fx::FxHashMap;

/// Where each non-ambient class in this file is declared. An ambient
/// (`declare class`) declaration has no evaluation to be early of, which is why
/// tsc exempts it.
pub(crate) fn file_class_declarations(statements: &[ParsedStatement]) -> FxHashMap<String, TextSpan> {
    let mut declarations = FxHashMap::default();
    collect(statements, &mut declarations);
    declarations
}

fn collect(statements: &[ParsedStatement], out: &mut FxHashMap<String, TextSpan>) {
    for statement in statements {
        match statement {
            ParsedStatement::ClassDeclaration(class) => {
                if class.is_declare {
                    continue;
                }
                if let Some(span) = class.name_span {
                    out.entry(class.name.clone()).or_insert(span);
                }
            }
            ParsedStatement::ExportDeclaration(export) => {
                if let ParsedExportDeclaration::Statement { declaration, .. } = export.as_ref() {
                    collect(std::slice::from_ref(declaration), out);
                }
            }
            _ => {}
        }
    }
}

pub(crate) fn check_statement_forward_references(
    statement: &ParsedStatement,
    classes: &FxHashMap<String, TextSpan>,
    ctx: &mut CheckerContext,
) {
    if classes.is_empty() {
        return;
    }
    let mut reported = Vec::new();
    for_each_statement_expression(statement, &mut |expression| {
        walk(expression, classes, &mut reported);
    });
    for (name, span) in reported {
        let diagnostic = Diagnostic::ts2449(&name, ctx.file_name.clone());
        ctx.push(diagnostic.with_span(convert_span(span)));
    }
}

/// The expressions a module-level statement evaluates as it runs. An
/// `export { X }` clause names a binding without reading it, which is why tsc
/// exempts an export specifier here.
fn for_each_statement_expression(
    statement: &ParsedStatement,
    visit: &mut impl FnMut(&ParsedExpression),
) {
    match statement {
        ParsedStatement::VariableDeclaration(variable) => {
            if let Some(initializer) = &variable.initializer {
                visit(initializer);
            }
        }
        ParsedStatement::Expression(expression) => visit(expression),
        ParsedStatement::Assignment(assignment) => visit(&assignment.value),
        ParsedStatement::MemberAssignment(assignment) => visit(&assignment.value),
        ParsedStatement::ExportDeclaration(export) => {
            if let ParsedExportDeclaration::Statement { declaration, .. } = export.as_ref() {
                for_each_statement_expression(declaration, visit);
            }
        }
        _ => {}
    }
}

fn walk(
    expression: &ParsedExpression,
    classes: &FxHashMap<String, TextSpan>,
    reported: &mut Vec<(String, TextSpan)>,
) {
    if let ParsedExpression::Identifier { name, span } = expression
        && let Some(declared_at) = classes.get(name)
        && let Some(use_span) = span
        && use_span.start < declared_at.start
    {
        reported.push((name.clone(), *use_span));
        return;
    }

    for_each_child_expression(expression, &mut |child| walk(child, classes, reported));
}

/// Sub-expressions evaluated as part of this one. A function or arrow body is
/// deliberately not among them: its contents run later, which is exactly what
/// makes an early reference inside one legal.
fn for_each_child_expression(
    expression: &ParsedExpression,
    visit: &mut impl FnMut(&ParsedExpression),
) {
    match expression {
        ParsedExpression::New {
            callee, arguments, ..
        } => {
            visit(callee);
            for argument in arguments {
                visit(&argument.expression);
            }
        }
        ParsedExpression::Call { arguments, .. } => {
            for argument in arguments {
                visit(&argument.expression);
            }
        }
        ParsedExpression::ExpressionCall {
            callee, arguments, ..
        }
        | ParsedExpression::OptionalCall {
            callee, arguments, ..
        } => {
            visit(callee);
            for argument in arguments {
                visit(&argument.expression);
            }
        }
        ParsedExpression::PropertyCall {
            object, arguments, ..
        }
        | ParsedExpression::OptionalPropertyCall {
            object, arguments, ..
        } => {
            visit(object);
            for argument in arguments {
                visit(&argument.expression);
            }
        }
        ParsedExpression::PropertyAccess { object, .. }
        | ParsedExpression::OptionalPropertyAccess { object, .. } => visit(object),
        ParsedExpression::ElementAccess { object, index, .. } => {
            visit(object);
            visit(index);
        }
        ParsedExpression::IndexAccess { index, .. } => visit(index),
        ParsedExpression::Unary { operand, .. }
        | ParsedExpression::Update { operand, .. }
        | ParsedExpression::Await { operand, .. } => visit(operand),
        ParsedExpression::Binary { left, right, .. }
        | ParsedExpression::Logical { left, right, .. }
        | ParsedExpression::NullishCoalescing { left, right, .. } => {
            visit(left);
            visit(right);
        }
        ParsedExpression::Conditional {
            condition,
            when_true,
            when_false,
            ..
        } => {
            visit(condition);
            visit(when_true);
            visit(when_false);
        }
        ParsedExpression::ArrayLiteral { elements, .. } => {
            for element in elements {
                visit(&element.expression);
            }
        }
        ParsedExpression::ObjectLiteral { properties, .. } => {
            for property in properties {
                // A method shorthand carries a body, which runs later.
                if !property.is_method && !property.is_accessor {
                    visit(&property.value);
                }
            }
        }
        ParsedExpression::TemplateLiteral { expressions, .. } => {
            for interpolation in expressions {
                visit(interpolation);
            }
        }
        ParsedExpression::TypeAssertion { expression, .. }
        | ParsedExpression::SatisfiesExpression { expression, .. }
        | ParsedExpression::ConstAssertion { expression, .. }
        | ParsedExpression::NonNullAssertion { expression, .. } => visit(expression),
        _ => {}
    }
}

/// What a class property initializer is allowed to read off `this`, ported from
/// `checkPropertyNotUsedBeforeDeclaration`.
///
/// Field initializers run in declaration order, before the constructor body, so
/// one may only read a property whose initializer has already run. Two parts of
/// the rule are easy to miss and both are load-bearing:
///
/// - A property **without an initializer** never satisfies the "declared before
///   use" test through `this`, even when it appears earlier in the class: there
///   is nothing to have run. Only a `!` definite assignment exempts it, and only
///   there — a property declared *later* is unusable however it is written.
///   This is the `declaration.Initializer() == nil` clause of
///   `isBlockScopedNameDeclaredBeforeUse`, which sits inside its
///   `declaration.Pos() <= usage.Pos()` branch.
/// - An optional property is exempt outright (`isOptionalPropertyDeclaration`),
///   because reading it as `undefined` is what its type already says.
///
/// A method is not a property declaration and is always available.
struct InitializedProperty {
    declared_at: usize,
    has_initializer: bool,
    optional: bool,
    asserted: bool,
}

pub(crate) fn check_class_property_initializers(
    class: &ParsedClassDeclaration,
    ctx: &mut CheckerContext,
) {
    let mut properties: FxHashMap<String, InitializedProperty> = FxHashMap::default();
    for member in &class.members {
        let ParsedClassMember::Property(property) = member else {
            continue;
        };
        let Some(span) = property.name_span else {
            continue;
        };
        properties.insert(
            property.name.clone(),
            InitializedProperty {
                declared_at: span.start,
                has_initializer: property.initializer.is_some(),
                optional: property.optional,
                asserted: property.has_definite_assertion,
            },
        );
    }
    if properties.is_empty() {
        return;
    }

    let mut reported = Vec::new();
    for member in &class.members {
        let ParsedClassMember::Property(property) = member else {
            continue;
        };
        let Some(initializer) = &property.initializer else {
            continue;
        };
        let Some(use_span) = property.name_span else {
            continue;
        };
        walk_this_reads(initializer, &class.name, &mut |name, span| {
            let Some(target) = properties.get(name) else {
                return;
            };
            if target.optional {
                return;
            }
            // The `!` and "has an initializer" exemptions belong only to the
            // *declared-earlier* branch of `isBlockScopedNameDeclaredBeforeUse`:
            // they say an earlier property is already usable. A property
            // declared later is unusable however it is written.
            if target.declared_at < use_span.start
                && (target.has_initializer || target.asserted)
            {
                return;
            }
            reported.push((name.to_string(), span));
        });
    }

    for (name, span) in reported {
        let diagnostic = Diagnostic::ts2729(&name, ctx.file_name.clone());
        ctx.push(diagnostic.with_span(convert_span(span)));
    }
}

/// Visits every `this.<name>` and `<ClassName>.<name>` read in `expression`,
/// stopping at function boundaries for the reason [`walk`] does.
fn walk_this_reads(
    expression: &ParsedExpression,
    class_name: &str,
    visit: &mut impl FnMut(&str, TextSpan),
) {
    if let ParsedExpression::PropertyAccess {
        object,
        property_name,
        property_span,
        ..
    } = expression
    {
        let reads_own_member = match object.as_ref() {
            ParsedExpression::This { .. } => true,
            ParsedExpression::Identifier { name, .. } => name == class_name,
            _ => false,
        };
        if reads_own_member && let Some(span) = property_span {
            visit(property_name, *span);
            return;
        }
    }

    for_each_child_expression(expression, &mut |child| {
        walk_this_reads(child, class_name, visit)
    });
}
