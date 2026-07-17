//! Two fresh checks of the same multi-file program must render byte-identical
//! diagnostics. Process-global caches (canonical stores, generic caches) are
//! warm on the second run, so this also pins that cache hits do not change
//! diagnostic content or ordering.

use surge_ts_checker::{SourceFileInput, check_program};
use surge_ts_diagnostics::Diagnostic;

fn render(diagnostics: &[Diagnostic]) -> String {
    diagnostics
        .iter()
        .map(|diagnostic| format!("{diagnostic:?}\n"))
        .collect()
}

fn multi_file_program() -> Vec<SourceFileInput> {
    let mut files = Vec::new();
    for i in 0..12 {
        let mut source = String::new();
        if i > 0 {
            let previous = i - 1;
            source.push_str(&format!(
                "import {{ make{previous} }} from \"./file_{previous}\";\n"
            ));
        }
        source.push_str(&format!(
            "export type Choice{i} = \"a{i}\" | \"b{i}\" | \"c{i}\";\n\
             export interface Node{i}<T> {{ value: T; tag: Choice{i}; next?: Node{i}<T>; }}\n\
             export function make{i}(value: number): Node{i}<number> {{\n\
             \x20 return {{ value, tag: \"a{i}\" }};\n\
             }}\n\
             const wrongValue{i}: string = make{i}({i}).value;\n\
             const missing{i}: number = make{i}({i}).nope;\n"
        ));
        if i > 0 {
            let previous = i - 1;
            source.push_str(&format!(
                "export const chained{i}: string = make{previous}({i}).value;\n"
            ));
        }
        files.push(SourceFileInput {
            file_name: format!("src/file_{i}.ts"),
            source_text: source,
        });
    }
    files
}

#[test]
fn repeated_fresh_checks_render_identical_diagnostics() {
    let first = check_program(multi_file_program());
    let second = check_program(multi_file_program());

    let first_rendered = render(&first);
    let second_rendered = render(&second);

    assert!(
        !first.is_empty(),
        "expected the program to produce diagnostics so the comparison is meaningful"
    );
    assert_eq!(first_rendered, second_rendered);
}
