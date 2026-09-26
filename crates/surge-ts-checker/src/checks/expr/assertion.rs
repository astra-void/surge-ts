//! Assertion safety (TS2352), ported from `checkAssertionDeferred` in
//! `tsc/internal/checker/checker.go`.
//!
//! `x as T` is rejected only when the two types do not overlap in *either*
//! direction — an upcast and a downcast are both legitimate, and only a
//! conversion between unrelated types is the mistake the rule is about. tsc
//! widens the source's literal types before asking, which is why `lit as "z"`
//! on a `"a" | "b"` source is accepted: the source is compared as `string`.

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::TextSpan as SyntaxTextSpan;
use surge_ts_types::{Type, is_comparable_to};

use super::widen_type;
use crate::context::{CheckerContext, convert_span};
use crate::infer::InferredExpression;

/// `checkAssertionDeferred` asks the comparable relation in both directions: an
/// upcast relates target-to-source and a downcast source-to-target, and only a
/// conversion that relates neither way is the mistake. A type surge failed to
/// model, and `any`, relate to everything and so can never be it.
fn overlaps(source: &Type, target: &Type) -> bool {
    if source.is_unmodelled()
        || target.is_unmodelled()
        || matches!(source, Type::Any | Type::GenuineUnknown | Type::Never)
        || matches!(target, Type::Any | Type::GenuineUnknown | Type::Never)
    {
        return true;
    }
    is_comparable_to(target, source) || is_comparable_to(source, target)
}

/// `x as T` where neither type is comparable to the other. tsc anchors on the
/// whole assertion expression.
pub(crate) fn check_assertion_overlap(
    source_result: &InferredExpression,
    target: &Type,
    span: Option<SyntaxTextSpan>,
    ctx: &mut CheckerContext,
) {
    let (InferredExpression::Known(source), Some(span)) = (source_result, span) else {
        return;
    };

    // `getBaseTypeOfLiteralType` on the source, which is what makes a
    // literal-to-sibling-literal assertion legal.
    let widened = widen_type(source);
    if overlaps(&widened, target) {
        return;
    }

    let diagnostic = Diagnostic::ts2352(widened.name(), target.name(), ctx.file_name.clone());
    ctx.push(diagnostic.with_span(convert_span(span)));
}
