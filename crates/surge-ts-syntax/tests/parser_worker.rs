//! Reusing one `ParserWorker` across many files must produce results identical
//! to a fresh single-shot `parse_source` for every file, including after parse
//! failures, and across source kinds (TS, TSX, declarations, empty input).

use surge_ts_syntax::{ParsedSource, ParserWorker, parse_source};

fn assert_worker_matches_fresh(cases: &[(&str, String)]) {
    let mut worker = ParserWorker::new();
    for (file_name, source_text) in cases {
        let fresh: ParsedSource = parse_source(source_text, file_name);
        let reused: ParsedSource = worker.parse(source_text, file_name);
        assert_eq!(
            reused, fresh,
            "worker parse of {file_name} diverged from fresh parse"
        );
    }
}

#[test]
fn worker_matches_fresh_parse_across_source_kinds() {
    let large_source = {
        let mut out = String::new();
        for i in 0..2_000 {
            out.push_str(&format!(
                "const value_{i}: number = {i};\nfunction fn_{i}(arg_{i}: string): string {{ return arg_{i}; }}\n"
            ));
        }
        out
    };
    let many_identifiers = {
        let mut out = String::from("export const table = {\n");
        for i in 0..1_000 {
            out.push_str(&format!("  key_{i}: \"literal value number {i}\",\n"));
        }
        out.push_str("};\n");
        out
    };

    assert_worker_matches_fresh(&[
        (
            "valid.ts",
            "interface User { name: string }\nconst u: User = { name: \"a\" };\n".to_string(),
        ),
        (
            "invalid.ts",
            "const broken: = {{{;\nfunction (\n".to_string(),
        ),
        (
            "component.tsx",
            "export function App(props: { title: string }) {\n  return <div className=\"x\">{props.title}</div>;\n}\n"
                .to_string(),
        ),
        ("empty.ts", String::new()),
        (
            "decls.d.ts",
            "declare module \"pkg\" { export const x: number; }\ndeclare global { interface Window { y: string } }\n"
                .to_string(),
        ),
        ("large.ts", large_source),
        ("identifiers.ts", many_identifiers),
    ]);
}

#[test]
fn worker_recovers_after_parse_failure() {
    let mut worker = ParserWorker::new();

    let failed = worker.parse("const x: = ;;;", "bad.ts");
    assert!(
        !failed.parser_errors.is_empty(),
        "expected parser errors for invalid source"
    );

    let ok_source = "const y: number = 1;\nexport type Id = string;\n";
    let recovered = worker.parse(ok_source, "good.ts");
    let fresh = parse_source(ok_source, "good.ts");
    assert_eq!(recovered, fresh);
    assert!(recovered.parser_errors.is_empty());
}

/// Interleave many distinct files through one worker. Every returned
/// `ParsedSource` is kept alive until the end, so any accidental retention of
/// arena-backed memory (which the reset would invalidate) would surface as
/// corrupted strings or a crash under this pattern.
#[test]
fn worker_outputs_stay_valid_after_later_parses() {
    let mut worker = ParserWorker::new();
    let sources: Vec<(String, String)> = (0..200)
        .map(|i| {
            (
                format!("file_{i}.ts"),
                format!(
                    "import {{ dep_{i} }} from \"./dep_{i}\";\nexport const marker_{i}: string = \"payload_{i}\";\n"
                ),
            )
        })
        .collect();

    let parsed: Vec<ParsedSource> = sources
        .iter()
        .map(|(name, text)| worker.parse(text, name))
        .collect();

    for (i, (name, text)) in sources.iter().enumerate() {
        let fresh = parse_source(text, name);
        assert_eq!(parsed[i], fresh, "retained result for {name} diverged");
        assert_eq!(parsed[i].file_name, *name);
    }
}
