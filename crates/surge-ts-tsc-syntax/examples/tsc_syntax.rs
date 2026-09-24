//! Prints a file's syntactic diagnostics the way `tsc --pretty false` does.
//! Usage: tsc_syntax <file>...

fn main() {
    for path in std::env::args().skip(1) {
        let Ok(text) = std::fs::read_to_string(&path) else {
            eprintln!("cannot read {path}");
            continue;
        };
        let Some(options) = surge_ts_tsc_syntax::ParseOptions::for_file_name(&path) else {
            continue;
        };
        for diagnostic in surge_ts_tsc_syntax::syntactic_diagnostics(&text, &options) {
            let before = &text[..diagnostic.start.min(text.len())];
            let line = before.matches('\n').count() + 1;
            let line_start = before.rfind('\n').map_or(0, |index| index + 1);
            let column = before[line_start..].encode_utf16().count() + 1;
            println!("{path}({line},{column}): error TS{}: {}", diagnostic.code, diagnostic.message);
        }
    }
}
