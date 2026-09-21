//! A conditional alias's `infer` capture reaches diagnostics as what it was
//! bound to, not as the variable's written name. The instantiation display is
//! built from the *written* type arguments, so `Awaited<R>` inside
//! `T extends (...args: any[]) => infer R ? Awaited<R> : T` rendered as
//! "type 'Awaited<R>'" long after `R` was known — the type itself was right, so
//! this only ever showed up as message drift.

use surge_ts_checker::check_source;

fn rendered(diagnostics: &[surge_ts_diagnostics::Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
        .collect()
}

#[test]
fn an_infer_capture_renders_as_what_it_is_bound_to() {
    let diagnostics = check_source(
        "type Unwrap<T> = T extends (...args: any[]) => infer R ? Awaited<R> : T;\n\
         declare function make(): Promise<{ a: number }>;\n\
         declare const unwrapped: Unwrap<typeof make>;\n\
         export const missing = unwrapped.nope;\n",
        "example.ts",
    );
    let messages = rendered(&diagnostics);
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert!(
        !messages[0].contains("Awaited<R>"),
        "the capture's written name leaked into the message: {messages:?}"
    );
    assert!(messages[0].contains("a: number"), "{messages:?}");
}

// The member still resolves through the capture: the binding was always correct,
// and the display fix must not change what the type answers.
#[test]
fn an_infer_capture_still_answers_its_members() {
    let diagnostics = check_source(
        "type Unwrap<T> = T extends (...args: any[]) => infer R ? Awaited<R> : T;\n\
         declare function make(): Promise<{ a: number }>;\n\
         declare const unwrapped: Unwrap<typeof make>;\n\
         export const wrong: string = unwrapped.a;\n",
        "example.ts",
    );
    let messages = rendered(&diagnostics);
    assert_eq!(messages.len(), 1, "{messages:?}");
    assert!(messages[0].starts_with("TS2322"), "{messages:?}");
    assert!(
        messages[0].contains("'number' is not assignable to type 'string'"),
        "{messages:?}"
    );
}
