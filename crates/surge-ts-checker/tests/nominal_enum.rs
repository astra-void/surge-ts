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

// An enum type is displayed by the enum's name. The nominal wrapper carries that
// without disturbing the literal-union payload underneath, so everything an enum
// member could do before it still does.
#[test]
fn enum_member_is_assignable_to_its_underlying_primitive() {
    let diagnostics = check(
        "enum Color { Red = 1 }\n\
         enum Names { Wide = \"wide\" }\n\
         declare const c: Color.Red;\n\
         declare const n: Names.Wide;\n\
         export const a: number = c;\n\
         export const b: string = n;\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn enum_member_is_assignable_to_its_own_member_type() {
    let diagnostics = check(
        "enum Names { Wide = \"wide\", Tall = \"tall\" }\n\
         export const ok: Names.Wide = Names.Wide;\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn enum_type_still_reports_a_mismatch_against_an_unrelated_type() {
    let diagnostics = check(
        "enum Color { Red = 1, Green = 2 }\n\
         declare const c: Color.Red;\n\
         export const bad: string = c;\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

// The nominal wrapper must not cost discriminant narrowing.
#[test]
fn string_enum_discriminant_still_narrows() {
    let diagnostics = check(
        "enum Names { Wide = \"wide\", Tall = \"tall\" }\n\
         interface W { kind: Names.Wide; w: number }\n\
         interface T { kind: Names.Tall; t: number }\n\
         export function area(v: W | T): number {\n\
             return v.kind === Names.Wide ? v.w : v.t;\n\
         }\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn numeric_enum_member_still_accepts_a_number_literal() {
    let diagnostics = check(
        "enum Color { Red = 1 }\n\
         export const ok: Color.Red = 1;\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}
