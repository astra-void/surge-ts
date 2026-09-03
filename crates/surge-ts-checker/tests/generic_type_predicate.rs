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

const RESULT_TYPES: &str = "type INVALID = { status: \"aborted\" };\n\
     type OK<T> = { status: \"valid\"; value: T };\n\
     type Sync<T> = OK<T> | INVALID;\n";

// A generic predicate's type arguments are inferred from the tested argument.
#[test]
fn generic_predicate_narrows_a_concrete_subject() {
    let diagnostics = check(&format!(
        "{RESULT_TYPES}\
         declare function isValid<T>(x: Sync<T>): x is OK<T>;\n\
         export function f(r: Sync<string>): string | null {{\n\
             return isValid(r) ? r.value : null;\n\
         }}\n"
    ));
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// The written parameter is an alias whose expansion is a union, so `T` cannot be
// aligned and stays unbound. Narrowing still selects the `OK` member.
#[test]
fn generic_predicate_narrows_under_a_generic_enclosing_function() {
    let diagnostics = check(&format!(
        "{RESULT_TYPES}\
         declare function isValid<T>(x: Sync<T>): x is OK<T>;\n\
         export function f<O>(r: Sync<O>): O | null {{\n\
             return isValid(r) ? r.value : null;\n\
         }}\n"
    ));
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// zod's shape: a generic arrow bound to a `const`, with a wider parameter type
// than the argument (`ParseReturnType<T>` against `SyncParseReturnType<Output>`).
#[test]
fn generic_arrow_predicate_with_a_wider_parameter_type_narrows() {
    let diagnostics = check(
        "type INVALID = { status: \"aborted\" };\n\
         type DIRTY<T> = { status: \"dirty\"; value: T };\n\
         type OK<T> = { status: \"valid\"; value: T };\n\
         type Sync<T> = OK<T> | DIRTY<T> | INVALID;\n\
         type Any<T> = Sync<T> | Promise<Sync<T>>;\n\
         const isValid = <T>(x: Any<T>): x is OK<T> => (x as any).status === \"valid\";\n\
         export const handle = <O>(r: Sync<O>): O | null => (isValid(r) ? r.value : null);\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A predicate written as a `const` annotation carries no collected signature
// unless the annotation is kept for it.
#[test]
fn predicate_written_as_a_const_annotation_narrows() {
    let diagnostics = check(&format!(
        "{RESULT_TYPES}\
         declare const isValid: (x: Sync<string>) => x is OK<string>;\n\
         export function f(r: Sync<string>): string | null {{\n\
             return isValid(r) ? r.value : null;\n\
         }}\n"
    ));
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// The false branch must keep the members the predicate did *not* match.
#[test]
fn generic_predicate_false_branch_keeps_the_other_member() {
    let diagnostics = check(&format!(
        "{RESULT_TYPES}\
         declare function isValid<T>(x: Sync<T>): x is OK<T>;\n\
         export function f<O>(r: Sync<O>): string {{\n\
             if (isValid(r)) {{\n\
                 return \"ok\";\n\
             }}\n\
             return r.status;\n\
         }}\n"
    ));
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// Narrowing must not invent members: a property the matched member does not
// declare still reports.
#[test]
fn generic_predicate_does_not_admit_an_unknown_property() {
    let diagnostics = check(&format!(
        "{RESULT_TYPES}\
         declare function isValid<T>(x: Sync<T>): x is OK<T>;\n\
         export function f<O>(r: Sync<O>): unknown {{\n\
             if (isValid(r)) {{\n\
                 return r.missing;\n\
             }}\n\
             return null;\n\
         }}\n"
    ));
    assert_eq!(codes(&diagnostics), vec!["TS2339"]);
}

// A predicate whose target matches *every* member proves nothing and must leave
// the subject alone rather than collapsing it to an `Any`-filled predicate.
#[test]
fn generic_predicate_matching_every_member_does_not_narrow() {
    let diagnostics = check(
        "type Box<T> = { value: T };\n\
         declare function isBox<T>(x: Box<T>): x is Box<T>;\n\
         export function f(r: Box<string>): number {\n\
             if (isBox(r)) {\n\
                 return r.value;\n\
             }\n\
             return 0;\n\
         }\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}
