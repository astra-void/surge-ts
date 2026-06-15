pub(crate) mod assign;
pub(crate) mod call;
pub(crate) mod expected;
pub(crate) mod expr;
pub(crate) mod function;
pub(crate) mod jsx;
pub(crate) mod ops;
pub(crate) mod var;

use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::TextSpan;

use crate::context::{CheckerContext, convert_span};

pub(crate) fn emit_type_only_as_value_diagnostic(
    name: &str,
    span: Option<TextSpan>,
    ctx: &mut CheckerContext,
) -> bool {
    if ctx.lookup_type_declaration(name).is_none() {
        return false;
    }

    let mut diagnostic = Diagnostic::ts2693(name, ctx.file_name.clone());

    if let Some(span) = span {
        diagnostic = diagnostic.with_span(convert_span(span));
    }

    ctx.push(diagnostic);
    true
}
