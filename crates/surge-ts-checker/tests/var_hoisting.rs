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
fn var_declared_in_a_block_is_visible_after_it() {
    let diagnostics = check(
        "export function f() {\n\
             {\n\
                 var hoisted = 1;\n\
             }\n\
             return hoisted + 1;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// The hoisted binding keeps the type it was declared with, so a misuse after the
// block still reports.
#[test]
fn hoisted_var_keeps_its_type() {
    let diagnostics = check(
        "export function f(): string {\n\
             {\n\
                 var hoisted = 1;\n\
             }\n\
             return hoisted;\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn var_hoists_out_of_an_if_branch_and_a_loop() {
    let diagnostics = check(
        "declare const cond: boolean;\n\
         export function f() {\n\
             if (cond) {\n\
                 var fromBranch = 1;\n\
             }\n\
             for (const _ of [1]) {\n\
                 var fromLoop = 2;\n\
             }\n\
             return fromBranch + fromLoop;\n\
         }\n",
    );
    // Visible, but not definitely assigned on every path.
    assert_eq!(codes(&diagnostics), vec!["TS2454", "TS2454"]);
}

// `let` and `const` stay block-scoped.
#[test]
fn let_and_const_do_not_hoist_out_of_a_block() {
    let diagnostics = check(
        "export function f() {\n\
             {\n\
                 let scoped = 1;\n\
                 const alsoScoped = 2;\n\
                 scoped;\n\
                 alsoScoped;\n\
             }\n\
             return scoped + alsoScoped;\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2304", "TS2304"]);
}

// Hoisting stops at the function boundary: a nested body's `var` must not leak
// into the enclosing one.
#[test]
fn var_does_not_escape_a_nested_function() {
    let diagnostics = check(
        "export function outer() {\n\
             function inner() {\n\
                 var innerOnly = 1;\n\
                 return innerOnly;\n\
             }\n\
             inner();\n\
             return innerOnly;\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
}

// A block-local `var` that shadows an outer binding restores the outer one at
// the *function* boundary, not at the block's.
#[test]
fn hoisted_var_shadow_is_restored_outside_the_function() {
    let diagnostics = check(
        "const shadowed: string = \"outer\";\n\
         export function f() {\n\
             {\n\
                 var shadowed = 1;\n\
             }\n\
             return shadowed;\n\
         }\n\
         export const outer: string = shadowed;\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}
