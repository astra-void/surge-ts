//! `@ts-expect-error` / `@ts-ignore` apply to the next line carrying code: tsc
//! walks backwards from a diagnostic over blank and `//`-comment lines looking
//! for a directive (`getDiagnosticsWithPrecedingDirectives`, program.go).

use surge_ts_checker::check_source;

fn codes(diagnostics: &[surge_ts_diagnostics::Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.to_string())
        .collect()
}

fn check(source_text: &str) -> Vec<surge_ts_diagnostics::Diagnostic> {
    check_source(source_text, "example.ts")
}

#[test]
fn a_directive_reaches_past_intervening_comments_and_blank_lines() {
    let diagnostics = check(
        "export function f() {\n\
         \x20 // @ts-expect-error - intentional\n\
         \x20 // eslint-disable-next-line no-restricted-syntax\n\
         \x20 const a: number = \"s\";\n\
         \x20 return a;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));

    let diagnostics = check(
        "export function f() {\n\
         \x20 // @ts-ignore\n\
         \n\
         \x20 const a: number = \"s\";\n\
         \x20 return a;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn a_directive_suppresses_only_the_first_code_line_after_it() {
    let diagnostics = check(
        "export function f() {\n\
         \x20 // @ts-expect-error - intentional\n\
         \x20 const a: number = \"s\";\n\
         \x20 const b: number = \"s\";\n\
         \x20 return a + b;\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

/// A block comment is not a `//` comment: tsc's `isCommentOrBlankLine` stops at
/// it, so the directive above it applies to that line and no further.
#[test]
fn a_block_comment_line_stops_the_walk() {
    let diagnostics = check(
        "export function f() {\n\
         \x20 // @ts-expect-error - intentional\n\
         \x20 /* a block comment */\n\
         \x20 const a: number = \"s\";\n\
         \x20 return a;\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2578", "TS2322"]);
}
