//! tsc's `checkEnumMember`, and the computed-member case of
//! `computeConstantEnumMemberValue`: every member initializer is checked as an
//! expression with the enum's members in scope, and one with no constant value
//! must be numeric (TS18033).

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::ParsedEnumBody;
use surge_ts_types::{Type, is_assignable_to};

use crate::context::{CheckerContext, convert_span};
use crate::infer::InferredExpression;
use crate::symbols::{ScopeStack, SymbolInfo, SymbolKind, SymbolTable};

/// `symbols` is the scope the enum is declared in, after its object is bound.
pub(crate) fn check_enum_members(
    enum_name: &str,
    bodies: &[ParsedEnumBody],
    symbols: SymbolTable,
    ctx: &mut CheckerContext,
) {
    let Some(Type::Object(object)) = symbols.get(enum_name).map(|symbol| symbol.ty.peeled()) else {
        return;
    };
    let mut scopes = ScopeStack::from_root(symbols);
    scopes.push_child();
    // A bare name in an initializer is first a member of the enum, of any of
    // its declarations; the enum's object carries all of them.
    for (name, property) in object.properties.iter() {
        let _ = scopes.insert_current(
            name.to_string(),
            SymbolInfo {
                ty: property.ty.clone(),
                kind: SymbolKind::Const,
                function_signature: None,
            },
        );
    }
    for body in bodies {
        for member in &body.members {
            let inferred = crate::checks::expr::evaluate_expression(
                &member.initializer,
                member.initializer_span,
                scopes.visible_symbols(),
                ctx,
            );
            // A `const enum` reports a computed initializer as TS2474 instead.
            if body.is_const || !member.computed {
                continue;
            }
            let (InferredExpression::Known(ty), Some(span)) = (inferred, member.initializer_span)
            else {
                continue;
            };
            if ty.is_unknown()
                || crate::checks::function::type_contains_degradation(&ty)
                || is_assignable_to(&ty, &Type::Number)
            {
                continue;
            }
            let source = crate::checks::expr::source_display_name(&ty, &Type::Number);
            let diagnostic = Diagnostic::ts18033(source, "number", ctx.file_name.clone())
                .with_span(convert_span(span));
            ctx.push(diagnostic);
        }
    }
}
