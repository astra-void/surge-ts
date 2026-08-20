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

const PREDICATE: &str = "function isObject(v: unknown): v is Record<string, unknown> {\n\
                             return typeof v === \"object\" && v !== null;\n\
                         }\n";

// A user-defined predicate guards the right operand of `&&` the same way
// `typeof`/`instanceof` already do, so the subject is no longer a genuine
// `unknown` receiver there.
#[test]
fn a_predicate_call_guards_the_right_operand_of_and() {
    let diagnostics = check(&format!(
        "{PREDICATE}\
         export function f(err: unknown) {{\n\
             return isObject(err) && typeof err[\"m\"] === \"string\";\n\
         }}\n"
    ));
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// The guard also has to survive alongside a syntactic guard earlier in the
// chain, which is the case that already produced a narrowed table.
#[test]
fn a_predicate_call_guards_after_a_syntactic_guard() {
    let diagnostics = check(&format!(
        "{PREDICATE}\
         export function f(a: unknown, b: unknown) {{\n\
             return typeof a === \"string\" && isObject(b) && !!b[\"m\"];\n\
         }}\n"
    ));
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// Ternary branches see the guard exactly as the `if` form does.
#[test]
fn ternary_branches_downgrade_a_guarded_unknown() {
    let diagnostics = check(
        "export function f(err: unknown) {\n\
             const message = err instanceof Error ? err.message : \"\";\n\
             const length = typeof err === \"string\" ? err.length : 0;\n\
             return [message, length];\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A negated condition guards the *false* branch, not the true one.
#[test]
fn a_negated_ternary_condition_downgrades_the_false_branch() {
    let diagnostics = check(&format!(
        "{PREDICATE}\
         export function f(err: unknown) {{\n\
             const value = !isObject(err) ? 0 : err[\"x\"];\n\
             return value;\n\
         }}\n"
    ));
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// An unguarded `unknown` still reports; the downgrade must not leak to the
// branch where the guard does not hold.
#[test]
fn an_unguarded_unknown_still_reports() {
    let diagnostics = check(
        "export function f(err: unknown) {\n\
             const message = err instanceof Error ? \"\" : err.message;\n\
             return message;\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS18046"]);
}
