use surge_ts_checker::{CheckerOptions, check_source_with_options};

fn codes(diagnostics: &[surge_ts_diagnostics::Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.to_string())
        .collect()
}

fn check(source_text: &str) -> Vec<surge_ts_diagnostics::Diagnostic> {
    check_source_with_options(
        source_text,
        "example.ts",
        CheckerOptions {
            no_implicit_any: true,
            ..Default::default()
        },
    )
}

// zod's `$constructor` shape: `T` occurs in no argument, only in the declared
// return type, so it is inferable solely from the target's annotation. Without
// that the initializer callback's parameter degrades and everything typed by it
// is silently unchecked.
#[test]
fn type_parameter_only_in_the_return_type_is_inferred_from_the_annotation() {
    let diagnostics = check(
        "interface Trait { tag: string }\n\
         interface Ctor<T extends Trait> { create(): T }\n\
         interface MyTrait extends Trait { alpha(n: number): void }\n\
         declare function make<T extends Trait>(name: string, init: (instance: T) => void): Ctor<T>;\n\
         const ctor: Ctor<MyTrait> = make(\"x\", (instance) => {\n\
             instance.alpha(1);\n\
         });\n\
         void ctor;\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn return_inferred_type_parameter_reports_a_mismatched_callback_body() {
    let diagnostics = check(
        "interface Trait { tag: string }\n\
         interface Ctor<T extends Trait> { create(): T }\n\
         interface MyTrait extends Trait { alpha(n: number): void }\n\
         declare function make<T extends Trait>(name: string, init: (instance: T) => void): Ctor<T>;\n\
         const ctor: Ctor<MyTrait> = make(\"x\", (instance) => {\n\
             const wrong: number = instance;\n\
             void wrong;\n\
         });\n\
         void ctor;\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

// An argument-derived candidate is authoritative: the contextual type only fills
// in what the arguments left unresolved, so a deliberately different annotation
// does not silently rewrite the inferred type argument.
#[test]
fn an_argument_inferred_type_parameter_wins_over_the_contextual_type() {
    let diagnostics = check(
        "declare function wrap<T>(value: T): T[];\n\
         const wrapped: string[] = wrap(1);\n\
         void wrapped;\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}
