//! A binding that is only ever *assigned* is unused: a plain `x = value` writes
//! `x` without reading it, so `noUnusedLocals` reports it exactly as tsc does.

use surge_ts_checker::{CheckerOptions, SourceFileInput, check_program_with_options};

fn ts6133_codes(source_text: &str) -> Vec<String> {
    let options = CheckerOptions {
        no_unused_locals: true,
        ..Default::default()
    };
    check_program_with_options(
        vec![SourceFileInput {
            file_name: "a.ts".to_string(),
            source_text: source_text.to_string(),
        }],
        options,
    )
    .iter()
    .map(|diagnostic| diagnostic.code.to_string())
    .filter(|code| code == "TS6133")
    .collect()
}

#[test]
fn a_write_only_local_is_unused() {
    let source = "export function f(data: Record<string, unknown>) {\n\
         let writeOnly: unknown;\n\
         for (const key in data) writeOnly = data[key];\n\
         return 1;\n\
     }\n";
    assert_eq!(ts6133_codes(source), vec!["TS6133"]);
}

#[test]
fn a_read_after_assignment_is_used() {
    let source = "export function f(data: Record<string, unknown>) {\n\
         let value: unknown;\n\
         for (const key in data) value = data[key];\n\
         return value;\n\
     }\n";
    assert!(ts6133_codes(source).is_empty());
}

// `x += 1` reads `x` before writing it, so the binding stays used.
#[test]
fn a_compound_assignment_reads_its_target() {
    let source = "export function f() {\n\
         let total = 0;\n\
         total += 1;\n\
         return 1;\n\
     }\n";
    assert!(ts6133_codes(source).is_empty());
}

// `o.p = v` reads `o`.
#[test]
fn a_member_assignment_reads_its_receiver() {
    let source = "export function f() {\n\
         const holder = { value: 0 };\n\
         holder.value = 1;\n\
         return 1;\n\
     }\n";
    assert!(ts6133_codes(source).is_empty());
}
