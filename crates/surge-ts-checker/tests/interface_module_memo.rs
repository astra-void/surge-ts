//! Regression coverage for the module-scoped interface instantiation memo
//! (`resolve_interface` in `infer/types/interface.rs`). The memo reuses an
//! interface body expansion for a repeated `(declaration, substitution)` inside
//! one module-analysis region, *including* expansions that degraded — a
//! mutually recursive user cluster that degrades is otherwise re-derived at
//! every use site. These pin the properties that make the reuse safe.

use surge_ts_checker::{SourceFileInput, check_program, check_source};

fn codes(diagnostics: &[surge_ts_diagnostics::Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.to_string())
        .collect()
}

fn rendered(diagnostics: &[surge_ts_diagnostics::Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| format!("{}: {}", diagnostic.code, diagnostic.message))
        .collect()
}

fn program(files: &[(&str, &str)]) -> Vec<surge_ts_diagnostics::Diagnostic> {
    check_program(
        files
            .iter()
            .map(|(file_name, source_text)| SourceFileInput {
                file_name: (*file_name).to_string(),
                source_text: (*source_text).to_string(),
            })
            .collect(),
    )
}

// Two instantiations of the same degraded declaration must keep their own
// arguments. A memo entry is keyed on a *display-inclusive* fingerprint of the
// substitution precisely so `Box<string>` cannot be served `Box<number>`'s
// expansion (the canonical-store display-substitution class, which shows up as
// message drift rather than as a wrong type).
#[test]
fn repeated_degraded_instantiations_render_their_own_arguments() {
    let diagnostics = check_source(
        "interface Def<T> { tag: Missing.Tag; inner: T }\n\
         interface Box<T> { def: Def<T>; value: T }\n\
         declare const a: Box<string>;\n\
         declare const b: Box<number>;\n\
         export const x = a.nope;\n\
         export const y = b.nope;\n",
        "example.ts",
    );
    let messages = rendered(&diagnostics);
    assert_eq!(codes(&diagnostics), vec!["TS2339", "TS2339"], "{messages:?}");
    assert!(messages[0].contains("inner: string"), "{messages:?}");
    assert!(messages[1].contains("inner: number"), "{messages:?}");
}

// An interface that gains members from a later `declare global` augmentation
// must not be served an expansion memoized before the merge. The memo key
// carries the live declaration table's version for exactly this reason; without
// it, `NodeJS.ProcessEnv` — whose index signature arrives through
// `extends Dict<string>` in a second declaration — lost that signature and
// every `process.env.FOO` became a TS2339.
#[test]
fn a_globally_augmented_interface_keeps_the_merged_members() {
    let diagnostics = program(&[
        (
            "globals.d.ts",
            "interface Dict<T> { [key: string]: T | undefined }\n\
             interface Env { HOME: string }\n",
        ),
        (
            "augment.ts",
            "declare global {\n\
                 interface Env extends Dict<string> { PATH: string }\n\
             }\n\
             export {};\n",
        ),
        (
            "consumer.ts",
            "declare const env: Env;\n\
             export const home = env.HOME;\n\
             export const path = env.PATH;\n\
             export const anything = env.ANYTHING_AT_ALL;\n",
        ),
    ]);
    assert!(diagnostics.is_empty(), "{:?}", rendered(&diagnostics));
}

// A memoized expansion must not swallow a diagnostic its body emitted: only
// expansions whose body emitted nothing are stored, so an unresolved member
// type still reports however many consumers instantiate the declaration.
#[test]
fn a_body_diagnostic_survives_repeated_instantiation() {
    let diagnostics = program(&[
        (
            "types.ts",
            "export interface Box<T> { value: T; extra: NotDeclaredAnywhere }\n",
        ),
        (
            "consumer.ts",
            "import { Box } from \"./types\";\n\
             declare const a: Box<string>;\n\
             declare const b: Box<string>;\n\
             declare const c: Box<number>;\n\
             export const used = [a.value, b.value, c.value];\n",
        ),
    ]);
    assert!(
        codes(&diagnostics).iter().any(|code| code == "TS2304"),
        "{:?}",
        rendered(&diagnostics)
    );
}
