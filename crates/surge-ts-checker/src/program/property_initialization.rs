//! `strictPropertyInitialization` (TS2564), ported from
//! `checkPropertyInitialization` in `tsc/internal/checker/checker.go`.
//!
//! An instance property with no initializer must be definitely assigned in the
//! constructor. tsc answers "definitely assigned" with the same flow graph it
//! uses everywhere, by asking for the type of a synthesized `this.p` reference
//! at the constructor's end and checking it no longer includes `undefined`.
//! There is no such graph here, so the reachability rules are re-derived over
//! the statement list the same way [`crate::flow`] derives `guarantees_exit`:
//! a statement list assigns the property if some statement assigns it before
//! any statement that cannot complete.

use surge_ts_syntax::{
    ParsedClassDeclaration, ParsedClassMember, ParsedFunctionBodyStatement, ParsedType,
};
use surge_ts_types::Type;

use surge_ts_diagnostics::Diagnostic;

use crate::context::{CheckerContext, convert_span};
use crate::flow::analyze_function_body_flow;
use crate::infer::map_parsed_type;

/// Whether `body` assigns `this.<name>` on every path that reaches its end.
fn body_assigns(body: &[ParsedFunctionBodyStatement], name: &str) -> bool {
    for statement in body {
        if statement_assigns(statement, name) {
            return true;
        }
        // Nothing after a statement that cannot complete is on the path to the
        // constructor's end, so the property is not assigned along it.
        if statement_guarantees_exit(statement) {
            return false;
        }
    }
    false
}

fn statement_assigns(statement: &ParsedFunctionBodyStatement, name: &str) -> bool {
    match statement {
        ParsedFunctionBodyStatement::ThisPropertyAssignment(assignment) => {
            assignment.property_name == name
        }
        ParsedFunctionBodyStatement::Block(body) => body_assigns(body, name),
        ParsedFunctionBodyStatement::If(if_statement) => {
            // A branch that cannot complete leaves the other one as the only
            // path to the end, so it alone decides.
            let then_decides = body_assigns(&if_statement.then_body, name)
                || body_guarantees_exit(&if_statement.then_body);
            let else_decides = body_assigns(&if_statement.else_body, name)
                || body_guarantees_exit(&if_statement.else_body);
            !if_statement.else_body.is_empty() && then_decides && else_decides
        }
        // A loop whose body is not guaranteed to run assigns nothing; `do … while`
        // lowers with `runs_at_least_once` set and does.
        ParsedFunctionBodyStatement::While(while_statement) => {
            while_statement.runs_at_least_once && body_assigns(&while_statement.body, name)
        }
        // A `try` body may be abandoned part-way, so only `finally` is on every
        // path out of the statement.
        ParsedFunctionBodyStatement::Try(try_statement) => {
            body_assigns(&try_statement.finalizer, name)
        }
        _ => false,
    }
}

fn statement_guarantees_exit(statement: &ParsedFunctionBodyStatement) -> bool {
    analyze_function_body_flow(std::slice::from_ref(statement)).guarantees_exit
}

fn body_guarantees_exit(body: &[ParsedFunctionBodyStatement]) -> bool {
    body.iter().any(statement_guarantees_exit)
}

/// The constructor that carries a body. Overload signatures declare no
/// statements, so only the implementation can initialize anything.
fn constructor_body(class: &ParsedClassDeclaration) -> Option<&[ParsedFunctionBodyStatement]> {
    class.members.iter().find_map(|member| match member {
        ParsedClassMember::Constructor(constructor) if !constructor.body.is_empty() => {
            Some(constructor.body.as_slice())
        }
        _ => None,
    })
}

/// tsc exempts a property whose type already admits the uninitialized value:
/// `any`, `unknown`, or anything including `undefined`. A type surge failed to
/// model is exempt too — an unresolved annotation is not evidence of a missing
/// initializer.
fn type_exempts_initialization(declared: &ParsedType, ctx: &mut CheckerContext) -> bool {
    let ty = map_parsed_type(declared.clone(), ctx);
    contains_undefined_or_wide(&ty)
}

fn contains_undefined_or_wide(ty: &Type) -> bool {
    match ty {
        Type::Any | Type::GenuineUnknown | Type::Unknown | Type::Undefined | Type::Void => true,
        Type::Union(union) => union.types().iter().any(contains_undefined_or_wide),
        _ => false,
    }
}

pub(crate) fn check_property_initialization(
    class: &ParsedClassDeclaration,
    ctx: &mut CheckerContext,
) {
    if !ctx.options.strict_property_initialization || class.is_declare {
        return;
    }

    let constructor = constructor_body(class).map(<[_]>::to_vec);

    // A class type parameter is in scope for its members' annotations. Without
    // this the annotation is resolved in the enclosing scope, where the
    // parameter is not a name at all, and `map_parsed_type` reports the
    // *resolution* failure (TS2304 "Cannot find name 'T'") — a diagnostic about
    // this check's own lookup, on a class that is perfectly well typed.
    crate::checks::function::with_type_parameter_scope(&class.type_parameters, ctx, |ctx| {
        check_declared_properties(class, constructor.as_deref(), ctx)
    });
}

fn check_declared_properties(
    class: &ParsedClassDeclaration,
    constructor: Option<&[ParsedFunctionBodyStatement]>,
    ctx: &mut CheckerContext,
) {
    for member in &class.members {
        let ParsedClassMember::Property(property) = member else {
            continue;
        };
        if property.is_static
            || property.is_abstract
            || property.is_declare
            || property.has_definite_assertion
            || property.optional
            || property.initializer.is_some()
        {
            continue;
        }
        // An unannotated property has no declared type to inspect; surge types
        // it `any`, which is exempt in tsc for the same reason.
        let Some(declared) = property.declared_type.as_ref() else {
            continue;
        };
        if type_exempts_initialization(declared, ctx) {
            continue;
        }
        if constructor.is_some_and(|body| body_assigns(body, &property.name)) {
            continue;
        }

        let diagnostic = Diagnostic::ts2564(&property.name, ctx.file_name.clone());
        ctx.push(match property.name_span {
            Some(span) => diagnostic.with_span(convert_span(span)),
            None => diagnostic,
        });
    }
}
