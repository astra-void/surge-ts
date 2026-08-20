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

// A rest parameter written as a tuple (`(...args: [value: number, p?: string])`)
// declares positional parameters, so a callback written against it takes one
// contextual type per tuple element rather than the whole tuple in slot 0.
#[test]
fn tuple_rest_parameter_types_each_positional_callback_parameter() {
    let diagnostics = check(
        "declare function run(f: (...args: [value: number, p?: string]) => void): void;\n\
         run((value, params) => {\n\
             const n: number = value;\n\
             const s: string | undefined = params;\n\
             void n;\n\
             void s;\n\
         });\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn tuple_rest_parameter_reports_a_mismatched_positional_use() {
    let diagnostics = check(
        "declare function run(f: (...args: [value: number, flag: boolean]) => void): void;\n\
         run((value, flag) => {\n\
             const s: string = flag;\n\
             void s;\n\
             void value;\n\
         });\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

// An array-typed rest parameter keeps its element-wise behaviour: every position
// is the element type, and no positional expansion happens.
#[test]
fn array_rest_parameter_still_types_every_position_with_the_element_type() {
    let diagnostics = check(
        "declare function run(f: (...args: number[]) => void): void;\n\
         run((first, second) => {\n\
             const a: number = first;\n\
             const b: number = second;\n\
             void a;\n\
             void b;\n\
         });\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// The zod `_LazyMethodsOf<T>` shape: a mapped type rebuilds every method as
// `(this: T, ...args: A) => R` with `A` an inferred tuple, and the installed
// object literal's method shorthands are contextually typed by it.
#[test]
fn mapped_variadic_tuple_rebuild_types_an_installed_method_shorthand() {
    let diagnostics = check(
        "type Methods<T> = Partial<{\n\
             [K in keyof T]: T[K] extends (...a: infer A) => infer R ? (...a: A) => R : never;\n\
         }>;\n\
         declare function install<T extends object>(instance: T, methods: Methods<T>): void;\n\
         interface Shape { gt(value: number, message?: string): Shape }\n\
         declare const shape: Shape;\n\
         install(shape, {\n\
             gt(value, message) {\n\
                 void value;\n\
                 void message;\n\
                 return shape;\n\
             },\n\
         });\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}
