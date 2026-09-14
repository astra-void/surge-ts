//! Classic `for`, `do … while` and `for … in` statements used to be dropped by
//! the function-body parser, so nothing inside them was ever checked. These
//! pin the lowering that made their bodies visible, and the flow facts tsc
//! derives from the same statements.

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
fn classic_for_body_is_checked() {
    let diagnostics = check(
        "export function f() {\n\
         \x20 for (let i = 0; i < 3; i++) {\n\
         \x20   const x: number = \"s\";\n\
         \x20 }\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn classic_for_binding_is_visible_in_its_body() {
    let diagnostics = check(
        "export function f(items: string[]) {\n\
         \x20 for (let i = 0; i < items.length; i++) {\n\
         \x20   items[i].length;\n\
         \x20   i.toFixed(1);\n\
         \x20 }\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

/// Two sibling loops each declare `i`; the lowering keeps the initializer in a
/// block of its own so they do not collide.
#[test]
fn sibling_for_loops_may_both_declare_the_same_binding() {
    let diagnostics = check(
        "export function f() {\n\
         \x20 for (let i = 0; i < 2; i++) {}\n\
         \x20 for (let i = 0; i < 2; i++) {}\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn do_while_body_is_checked() {
    let diagnostics = check(
        "export function f() {\n\
         \x20 do { const y: string = 1; } while (false);\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

/// A `do … while` body runs before the condition, so what it assigns is
/// definitely assigned afterwards — the plain `while` lowering would report
/// TS2454 here.
#[test]
fn do_while_body_assignment_is_definite() {
    let diagnostics = check(
        "export function f() {\n\
         \x20 let y: string;\n\
         \x20 do { y = \"a\"; } while (false);\n\
         \x20 return y.length;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn for_in_binds_the_key_as_string() {
    let diagnostics = check(
        "export function f(o: { [k: string]: number }) {\n\
         \x20 for (const k in o) { k.toFixed(1); }\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2339"]);
}

/// tsc (flow.go): "for (const _ in ref) acts as a nonnull on ref".
#[test]
fn for_in_narrows_its_iterated_reference_to_non_null() {
    let diagnostics = check(
        "declare const o: { [k: string]: number } | undefined;\n\
         export function f() {\n\
         \x20 for (const k in o) { o[k].toFixed(1); }\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

/// tsc only runs the missing-return check when the function's end point is
/// reachable, and its binder folds only the `true`/`false` *keywords*.
#[test]
fn endless_loops_have_no_reachable_end_point() {
    for body in [
        "while (true) { }",
        "for (;;) { }",
        "do { } while (true);",
        "if (true) { return 1; }",
    ] {
        let diagnostics = check(&format!("export function f(): number {{ {body} }}\n"));
        assert!(diagnostics.is_empty(), "{body}: {:?}", codes(&diagnostics));
    }
}

#[test]
fn a_loop_that_can_break_still_reports_a_missing_return() {
    let diagnostics = check("export function f(): number { while (true) { if (0) break; } }\n");
    assert_eq!(codes(&diagnostics), vec!["TS2355"]);
    // `while (1)` is not folded by tsc's binder either.
    let diagnostics = check("export function f(): number { while (1) { } }\n");
    assert_eq!(codes(&diagnostics), vec!["TS2355"]);
}

/// A label only names a `break`/`continue` target; the statement it labels was
/// being dropped along with it.
#[test]
fn labelled_statement_bodies_are_checked() {
    let diagnostics = check(
        "export function f() {\n\
         \x20 outer: for (const row of [[1]]) { const s: string = row; }\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);

    let diagnostics = check(
        "export function f() {\n\
         \x20 lab: { const t: number = \"x\"; }\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

/// `while (true)` enters its body unconditionally, so what the body assigns is
/// assigned after the loop.
#[test]
fn an_endless_loop_body_assigns_definitely() {
    let diagnostics = check(
        "export function f() {\n\
         \x20 let x: number;\n\
         \x20 outer: while (true) { x = 1; break outer; }\n\
         \x20 return x;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}
