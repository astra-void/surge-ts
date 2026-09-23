//! Arrow-function expression inference.

use super::*;

use surge_ts_syntax::{ParsedArrowFunction, ParsedArrowFunctionBody};
use surge_ts_types::Type;

use crate::context::CheckerContext;
use crate::metrics::alloc_function_type;
use crate::symbols::SymbolTable;

use crate::infer::InferredExpression;

pub(crate) fn infer_arrow_function(
    arrow_function: &ParsedArrowFunction,
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> surge_ts_types::FunctionType {
    infer_arrow_function_with_contextual_parameters(arrow_function, &[], symbols, ctx)
}

/// [`infer_arrow_function`] with the parameter types the signature the callback
/// is being passed to gives it. A generic call's second inference pass supplies
/// them: an un-annotated parameter sketched as `any` types the body as `any`
/// too, and a type parameter that appears only in the callback's *return*
/// position (`map<U>(f: (v: T) => U): U`) is then bound to `any` — the call's
/// result silences every diagnostic downstream of it.
pub(crate) fn infer_arrow_function_with_contextual_parameters(
    arrow_function: &ParsedArrowFunction,
    contextual_parameters: &[Type],
    symbols: &SymbolTable,
    ctx: &mut CheckerContext,
) -> surge_ts_types::FunctionType {
    // This sketch feeds generic-call inference (`fn(impl?: T)` bound from a
    // `vi.fn((n: number) => …)` argument); the authoritative
    // `check_arrow_function_expression` pass follows with the contextual
    // signature and reports the annotations' own diagnostics, so the ones the
    // mapping emits here are discarded. A generic arrow keeps the untyped
    // sketch: its annotations name type parameters no scope here declares.
    let typed_annotations = arrow_function.type_parameters.is_empty();
    let diagnostics_before = ctx.diagnostics().len();
    let parameters = arrow_function
        .parameters
        .iter()
        .enumerate()
        .map(|(index, parameter)| match &parameter.declared_type {
            // An annotation that does not resolve stays `any`, which is what
            // every parameter was before annotations were read at all, so it
            // cannot newly disable an inference that used to succeed.
            Some(declared_type) if typed_annotations => {
                let mapped = map_parsed_type(declared_type.clone(), ctx);
                if mapped.is_unknown() {
                    Type::Any
                } else {
                    mapped
                }
            }
            _ => contextual_parameter_type(contextual_parameters, index),
        })
        .collect::<Vec<_>>();
    let declared_return_type = arrow_function.return_type.as_ref().and_then(|ty| {
        if typed_annotations {
            let mapped = map_parsed_type(ty.clone(), ctx);
            (!mapped.is_unknown()).then_some(mapped)
        } else {
            primitive_declared_return_type(ty)
        }
    });
    ctx.truncate_diagnostics(diagnostics_before);

    // A generator returns a `Generator`/`AsyncGenerator`, not whatever its body
    // completes with — surge does not model that shape, so the sketch stays at
    // the sentinel rather than claiming `void` and binding a caller's type
    // parameter to it (`run(async function* () { yield 'a' })`).
    if arrow_function.is_generator && declared_return_type.is_none() {
        return alloc_function_type(
            parameters,
            Type::Unknown,
            false,
            required_parameter_count(arrow_function.parameters.as_slice()),
        );
    }

    let infer_return_type = |ctx: &mut CheckerContext| match &arrow_function.body {
        ParsedArrowFunctionBody::Expression(expression) => {
            declared_return_type.unwrap_or_else(|| {
                let locals = body_locals(arrow_function, &parameters, symbols);
                match infer_expression(expression, &locals, ctx).flowing_type() {
                    Some(ty) => widen_fresh_literal_return(expression, ty),
                    None => Type::Unknown,
                }
            })
        }
        ParsedArrowFunctionBody::Block(body) => declared_return_type
            .or_else(|| {
                let locals = body_locals(arrow_function, &parameters, symbols);
                infer_block_body_return_type(body, locals, ctx)
            })
            // A body with no `return` anywhere returns `void`, as tsc types it.
            // Leaving it at the degradation sentinel made the sketch unusable for
            // generic inference: `vi.fn((result) => { … })` inferred no `T`, so
            // `Mock<T>` stayed uninstantiated and every use of the mock was a
            // false error.
            //
            // A body that cannot complete at all (`() => { throw new Error(…) }`)
            // returns `never`, which is assignable everywhere; typing it `void`
            // made zustand's throwing storage stub unassignable to `StateStorage`.
            .or_else(|| {
                (!body_contains_return(body)).then(|| {
                    if crate::flow::analyze_function_body_flow(body).guarantees_exit {
                        Type::Never
                    } else {
                        Type::Void
                    }
                })
            })
            .unwrap_or(Type::Unknown),
    };
    // The body sees the arrow's own type parameters wherever it names them
    // (`<U>() => <U[]>null`), as tsc's lexical `resolveName` finds them.
    let return_type = if arrow_function.type_parameters.is_empty() {
        infer_return_type(ctx)
    } else {
        crate::checks::function::with_type_parameter_scope(
            &arrow_function.type_parameters,
            ctx,
            infer_return_type,
        )
    };

    // An async function returns a promise of what its body completes with.
    let return_type = if arrow_function.is_async
        && arrow_function.return_type.is_none()
        && !return_type.is_unknown()
    {
        crate::checks::call::promise_of(&return_type, ctx)
    } else {
        return_type
    };

    with_written_predicate(
        alloc_function_type(
            parameters,
            return_type,
            false,
            required_parameter_count(arrow_function.parameters.as_slice()),
        ),
        arrow_function,
        ctx,
    )
}

/// A function expression — an object-literal method among them — whose
/// written return type is a type predicate keeps its written signature on
/// the handle, as a declared member does (`DeclaredMemberSignature`), so a
/// guard that calls it through a property (`Utils.isA(node)`) can narrow.
pub(crate) fn with_written_predicate(
    function_type: surge_ts_types::FunctionType,
    arrow_function: &ParsedArrowFunction,
    ctx: &CheckerContext,
) -> surge_ts_types::FunctionType {
    if !matches!(arrow_function.return_type, Some(surge_ts_syntax::ParsedType::Predicate(_)))
        || function_type.declaration().is_some()
    {
        return function_type;
    }
    function_type.with_declaration(std::sync::Arc::new(crate::checks::call::DeclaredMemberSignature {
        signature: crate::checks::function::function_signature_info(
            &arrow_function.type_parameters,
            &arrow_function.parameters,
            arrow_function.return_type.as_ref(),
            &ctx.file_name,
        ),
        outer_type_arguments: Vec::new(),
    }))
}

/// tsc widens the *fresh* literal a body expression returns, so `() => ''`
/// infers `() => string` and an object literal's property declared that way is
/// assignable from any `() => string`. A literal that came from a `const` is not
/// fresh and keeps its own type, so freshness is decided syntactically here, the
/// same way the generic-argument path decides it.
fn widen_fresh_literal_return(expression: &ParsedExpression, ty: Type) -> Type {
    if matches!(
        expression,
        ParsedExpression::StringLiteral(_)
            | ParsedExpression::NumberLiteral(_)
            | ParsedExpression::BooleanLiteral(_)
            | ParsedExpression::ObjectLiteral { .. }
            | ParsedExpression::ArrayLiteral { .. }
    ) {
        crate::checks::expr::widen_type(&ty)
    } else {
        ty
    }
}

/// A contextual parameter type is usable only once it is a real type: an
/// unresolved shape or a still-open type parameter would type the body as
/// something the caller cannot bind, which is worse than the `any` the
/// un-contextualized sketch uses.
fn contextual_parameter_type(contextual_parameters: &[Type], index: usize) -> Type {
    match contextual_parameters.get(index) {
        Some(ty)
            if !ty.is_unknown()
                && !matches!(ty, Type::TypeParameter(_))
                && !matches!(ty.peeled(), Type::Unknown) =>
        {
            ty.clone()
        }
        _ => Type::Any,
    }
}

fn primitive_declared_return_type(ty: &surge_ts_syntax::ParsedType) -> Option<Type> {
    match ty {
        surge_ts_syntax::ParsedType::String => Some(Type::String),
        surge_ts_syntax::ParsedType::Number => Some(Type::Number),
        surge_ts_syntax::ParsedType::Boolean => Some(Type::Boolean),
        surge_ts_syntax::ParsedType::Any => Some(Type::Any),
        surge_ts_syntax::ParsedType::Unknown => Some(Type::Unknown),
        surge_ts_syntax::ParsedType::UnknownKeyword => Some(Type::GenuineUnknown),
        surge_ts_syntax::ParsedType::Undefined => Some(Type::Undefined),
        surge_ts_syntax::ParsedType::Void => Some(Type::Void),
        _ => None,
    }
}

fn body_locals(
    arrow_function: &ParsedArrowFunction,
    parameter_types: &[Type],
    symbols: &SymbolTable,
) -> SymbolTable {
    // A destructured parameter binds its names from the parameter's type the
    // way the checking pass does; skipping it left `({ ctx }) => …` with `ctx`
    // unbound, so the body's return type was unknowable.
    let mut scopes = crate::symbols::ScopeStack::from_root(
        symbols.clone_with_reason(surge_ts_types::TypeCopyReason::ScopeOrContext),
    );
    // A named `function` expression's own name shadows the enclosing scope's;
    // the sketch being inferred is its type, so it stands at the sentinel.
    if let Some(name) = &arrow_function.name {
        scopes.insert_current(
            name.as_str(),
            crate::symbols::SymbolInfo {
                ty: Type::Unknown,
                kind: crate::symbols::SymbolKind::Function,
                function_signature: None,
            },
        );
    }
    for (parameter, ty) in arrow_function.parameters.iter().zip(parameter_types) {
        crate::checks::function::insert_binding_name(
            &parameter.binding_name,
            ty.clone(),
            &mut scopes,
        );
    }
    scopes
        .visible_symbols()
        .clone_with_reason(surge_ts_types::TypeCopyReason::ScopeOrContext)
}

pub(crate) fn required_parameter_count(
    parameters: &[surge_ts_syntax::ParsedFunctionParameter],
) -> usize {
    let mut required = parameters.len();

    while required > 0 {
        let parameter = &parameters[required - 1];
        if parameter.optional || parameter.initializer.is_some() {
            required -= 1;
        } else {
            break;
        }
    }

    required
}

/// Infers an unannotated block-bodied arrow's return type from its `return`
/// statements. Deliberately narrow: a run of local declarations followed by
/// returns is the shape a callback argument almost always has, and it is what
/// lets a lazy initializer (`useState(() => { const rows = …; return rows; })`)
/// contribute its type to the call's inference. Anything with branching, a bare
/// `return;`, or a binding pattern yields `None`, keeping the previous
/// `Unknown` — the body would need real flow analysis to type honestly.
/// Whether a function body contains a `return` anywhere, nested statements
/// included. A nested `function` declaration is its own body and does not count.
fn body_contains_return(body: &[surge_ts_syntax::ParsedFunctionBodyStatement]) -> bool {
    use surge_ts_syntax::ParsedFunctionBodyStatement as Statement;

    body.iter().any(|statement| match statement {
        Statement::Return(_) => true,
        Statement::Block(statements) => body_contains_return(statements),
        Statement::If(statement) => {
            body_contains_return(&statement.then_body) || body_contains_return(&statement.else_body)
        }
        Statement::While(statement) => body_contains_return(&statement.body),
        Statement::ForOf(statement) => body_contains_return(&statement.body),
        Statement::Switch(statement) => statement
            .cases
            .iter()
            .any(|case| body_contains_return(&case.consequent)),
        Statement::Try(statement) => {
            body_contains_return(&statement.block)
                || statement
                    .handler
                    .as_ref()
                    .is_some_and(|handler| body_contains_return(&handler.body))
                || body_contains_return(&statement.finalizer)
        }
        _ => false,
    })
}

fn infer_block_body_return_type(
    body: &[surge_ts_syntax::ParsedFunctionBodyStatement],
    mut locals: SymbolTable,
    ctx: &mut CheckerContext,
) -> Option<Type> {
    use surge_ts_syntax::ParsedFunctionBodyStatement;

    let returns_here = body
        .iter()
        .any(|statement| matches!(statement, ParsedFunctionBodyStatement::Return(_)));
    if !returns_here {
        return None;
    }
    // Any statement that can carry control flow (or a return this walk would
    // miss) disqualifies the body.
    let straight_line = body.iter().all(|statement| {
        matches!(
            statement,
            ParsedFunctionBodyStatement::VariableDeclaration(_)
                | ParsedFunctionBodyStatement::Return(_)
                | ParsedFunctionBodyStatement::Expression(_)
                | ParsedFunctionBodyStatement::TypeAlias(_)
        )
    });
    if !straight_line {
        return None;
    }

    let mut returned = Vec::new();
    for statement in body {
        match statement {
            ParsedFunctionBodyStatement::VariableDeclaration(variable) => {
                if variable.declared_type.is_some() {
                    return None;
                }
                let initializer = variable.initializer.as_ref()?;
                let InferredExpression::Known(ty) = infer_expression(initializer, &locals, ctx)
                else {
                    return None;
                };
                let kind = match variable.kind {
                    surge_ts_syntax::ParsedVariableKind::Var => crate::symbols::SymbolKind::Var,
                    surge_ts_syntax::ParsedVariableKind::Let => crate::symbols::SymbolKind::Let,
                    surge_ts_syntax::ParsedVariableKind::Const => {
                        crate::symbols::SymbolKind::Const
                    }
                };
                // The binding holds what its declaration widens the initializer
                // to (`const r = { ok: true }` is `{ ok: boolean }`), as the
                // checking pass binds it.
                let ty = crate::checks::var::widen_implicit_variable_initializer_type(
                    kind,
                    initializer,
                    &ty,
                    false,
                );
                let _ = locals.insert(
                    variable.name.clone(),
                    crate::symbols::SymbolInfo {
                        ty,
                        kind,
                        function_signature: None,
                    },
                );
            }
            ParsedFunctionBodyStatement::Return(statement) => {
                let expression = statement.expression.as_ref()?;
                let ty = infer_expression(expression, &locals, ctx).flowing_type()?;
                if ty.is_unknown() && !matches!(ty, Type::ErrorType) {
                    return None;
                }
                returned.push(widen_fresh_literal_return(expression, ty));
            }
            _ => {}
        }
    }

    match returned.len() {
        0 => None,
        1 => returned.pop(),
        _ => Some(surge_ts_types::union_type(returned)),
    }
}
