//! Prints a pre-order walk of each file's syntax tree, visiting children the
//! way tsc's `forEachChild` does as `properties::child_properties` describes
//! it: one `<Kind> <start> <end>` line per node, in byte offsets.
//! Usage: tree_walk <file>...
//! With several files, each file's walk follows a `== <file>` line. The
//! file's syntax errors go to stderr as `<file>: TS<code> at <start>: <message>`.

use std::io::{BufWriter, Write};

use surge_ts_tsc_syntax::properties::{child_properties, resolve};
use surge_ts_tsc_syntax::{ParseOptions, SyntaxTree};

fn main() -> std::io::Result<()> {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    let mut out = BufWriter::new(std::io::stdout().lock());
    for path in &paths {
        if paths.len() > 1 {
            writeln!(out, "== {path}")?;
        }
        let Ok(text) = std::fs::read_to_string(path) else {
            eprintln!("cannot read {path}");
            continue;
        };
        let Some(options) = ParseOptions::for_file_name(path) else {
            eprintln!("not a script file: {path}");
            continue;
        };
        let tree = SyntaxTree::parse(&text, &options);
        for diagnostic in tree.syntactic_diagnostics() {
            eprintln!("{path}: TS{} at {}: {}", diagnostic.code, diagnostic.start, diagnostic.message);
        }
        // An explicit stack: a long operator chain nests deeper than the
        // main thread's stack allows.
        let mut stack = vec![tree.root()];
        while let Some(id) = stack.pop() {
            writeln!(out, "{:?} {} {}", tree.kind(id), tree.start(&text, id), tree.end(id))?;
            let first_child = stack.len();
            for property in child_properties(tree.kind(id)) {
                stack.extend_from_slice(resolve(&tree, id, property.slot).nodes());
            }
            stack[first_child..].reverse();
        }
    }
    out.flush()
}
