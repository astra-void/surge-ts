//! Prints the diagnostics tsc reports for a program of these files before type
//! checking, the way `tsc --pretty false` does: the syntactic diagnostics if
//! any file has one, otherwise the binder's and the global merge's.
//! Usage: tsc_syntax <file>...

fn main() {
    let mut files = Vec::new();
    for path in std::env::args().skip(1) {
        let Ok(text) = std::fs::read_to_string(&path) else {
            eprintln!("cannot read {path}");
            continue;
        };
        let Some(options) = surge_ts_tsc_syntax::ParseOptions::for_file_name(&path) else {
            continue;
        };
        let diagnostics = surge_ts_tsc_syntax::file_diagnostics(&text, &options);
        files.push((path, text, diagnostics));
    }
    let syntactic = files.iter().any(|(_, _, diagnostics)| !diagnostics.syntactic.is_empty());
    let empty = surge_ts_tsc_syntax::FileGlobals::default();
    let inputs: Vec<_> = files
        .iter()
        .map(|(path, _, diagnostics)| surge_ts_tsc_syntax::GlobalsInput {
            globals: if path.ends_with(".js") || path.ends_with(".jsx") { &empty } else { &diagnostics.globals },
            plain_js: false,
        })
        .collect();
    let mut merged = if syntactic { vec![Vec::new(); files.len()] } else { surge_ts_tsc_syntax::merge_globals(&inputs) };
    drop(inputs);
    for ((path, text, diagnostics), merged) in files.into_iter().zip(merged.drain(..)) {
        let reported = if syntactic {
            diagnostics.syntactic
        } else {
            diagnostics.bind.into_iter().chain(merged).collect()
        };
        for diagnostic in reported {
            let before = &text[..diagnostic.start.min(text.len())];
            let line = before.matches('\n').count() + 1;
            let line_start = before.rfind('\n').map_or(0, |index| index + 1);
            let column = before[line_start..].encode_utf16().count() + 1;
            println!("{path}({line},{column}): error TS{}: {}", diagnostic.code, diagnostic.message);
        }
    }
}
