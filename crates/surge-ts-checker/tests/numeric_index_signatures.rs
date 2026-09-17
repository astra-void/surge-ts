//! A numeric index signature answers a numeric key and nothing else. tsc keeps
//! the two kinds apart (`findApplicableIndexInfo`): a numeric key prefers the
//! number signature and falls back to the string one, a string key a
//! number-only type cannot answer is TS7015, and the key of a `for…in` over
//! such a type indexes as a number even though the binding is a `string`.

use surge_ts_checker::{CheckerOptions, check_source_with_options};

fn codes(diagnostics: &[surge_ts_diagnostics::Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.to_string())
        .collect()
}

fn check(source_text: &str) -> Vec<surge_ts_diagnostics::Diagnostic> {
    let options = CheckerOptions {
        no_implicit_any: true,
        strict_property_initialization: false,
        ..CheckerOptions::default()
    };
    check_source_with_options(source_text, "example.ts", options)
}

const NUMBER_KEYED: &str = "interface NumberKeyed { [index: number]: string }\n\
     declare const numberKeyed: NumberKeyed;\n\
     declare const numericKey: number;\n\
     declare const stringKey: string;\n";

#[test]
fn a_numeric_key_reads_the_number_index() {
    let diagnostics = check(&format!(
        "{NUMBER_KEYED}export const a: string = numberKeyed[0];\n\
         export const b: string = numberKeyed[numericKey];\n"
    ));
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));

    let diagnostics = check(&format!(
        "{NUMBER_KEYED}export const a: number = numberKeyed[1];\n"
    ));
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn a_string_key_a_number_only_type_cannot_answer_is_an_implicit_any() {
    let diagnostics = check(&format!(
        "{NUMBER_KEYED}export function f() {{ return numberKeyed[stringKey]; }}\n"
    ));
    assert_eq!(codes(&diagnostics), vec!["TS7015"]);

    let diagnostics = check(&format!(
        "{NUMBER_KEYED}export function f() {{ numberKeyed[stringKey] = \"value\"; }}\n"
    ));
    assert_eq!(codes(&diagnostics), vec!["TS7015"]);
}

/// tsc's `isForInVariableForNumericPropertyNames`: the binding stays a
/// `string`, and the access through it reads the numeric index.
#[test]
fn a_for_in_key_over_numeric_property_names_indexes_as_a_number() {
    let diagnostics = check(&format!(
        "{NUMBER_KEYED}export function f() {{\n\
         \x20 for (const key in numberKeyed) {{\n\
         \x20   const value: string = numberKeyed[key];\n\
         \x20   const asString: string = key;\n\
         \x20   return value + asString;\n\
         \x20 }}\n\
         \x20 return \"\";\n\
         }}\n"
    ));
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));

    // The key is a `string`, not a `number`.
    let diagnostics = check(&format!(
        "{NUMBER_KEYED}export function f() {{\n\
         \x20 for (const key in numberKeyed) {{\n\
         \x20   const asNumber: number = key;\n\
         \x20   return asNumber;\n\
         \x20 }}\n\
         \x20 return 0;\n\
         }}\n"
    ));
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn a_string_index_answers_a_numeric_key_too() {
    let diagnostics = check(
        "interface StringKeyed { [key: string]: number }\n\
         declare const stringKeyed: StringKeyed;\n\
         export const a: number = stringKeyed[3];\n\
         export const b: string = stringKeyed[4];\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn a_numeric_key_prefers_the_number_index_when_both_are_declared() {
    let diagnostics = check(
        "interface BothKeys { [index: number]: string; [key: string]: string | number }\n\
         declare const bothKeys: BothKeys;\n\
         declare const stringKey: string;\n\
         export const a: string = bothKeys[0];\n\
         export const b: string | number = bothKeys[stringKey];\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));

    let diagnostics = check(
        "interface BothKeys { [index: number]: string; [key: string]: string | number }\n\
         declare const bothKeys: BothKeys;\n\
         export const a: number = bothKeys[2];\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

/// A write is what a later read of the same element sees, which is what keeps
/// `counts[key] = (counts[key] ?? 0) + 1` from leaving the next read
/// possibly-undefined under `noUncheckedIndexedAccess`.
#[test]
fn an_element_write_narrows_the_element_read() {
    let options = CheckerOptions {
        no_implicit_any: true,
        strict_property_initialization: false,
        no_unchecked_indexed_access: true,
        ..CheckerOptions::default()
    };
    let diagnostics = check_source_with_options(
        "export function f(specifiers: string[]) {\n\
         \x20 const counts: Record<string, number> = {};\n\
         \x20 let max = 0;\n\
         \x20 for (const specifier of specifiers) {\n\
         \x20   counts[specifier] = (counts[specifier] ?? 0) + 1;\n\
         \x20   if (counts[specifier] > max) { max = counts[specifier]; }\n\
         \x20 }\n\
         \x20 return max;\n\
         }\n",
        "example.ts",
        options,
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}
