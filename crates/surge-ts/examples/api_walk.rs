//! Walks every node of a program's own files and prints each symbol it finds
//! with its type, then the program's diagnostics: the compiler API's first
//! milestone, from Rust.
//!
//! Usage: `api_walk <file.ts>...` or `api_walk --project <tsconfig.json>`.

use surge_ts::api::{CreateProgramOptions, NodeId, Program, TypeChecker};

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let program = if args.first().map(String::as_str) == Some("--project") {
        let path = args.get(1).expect("--project needs a path");
        let text = std::fs::read_to_string(path).expect("cannot read the config");
        let json = surge_ts::api::parse_config_file_text(path, &text).expect("config is not JSON");
        let directory = std::path::Path::new(path).parent().unwrap().to_string_lossy().into_owned();
        let parsed = surge_ts::api::parse_json_config_file_content(&json, &directory, None, Some(path));
        Program::create(CreateProgramOptions {
            root_names: parsed.file_names,
            options: parsed.options,
            ..Default::default()
        })
    } else {
        Program::create(CreateProgramOptions { root_names: args, ..Default::default() })
    }
    .expect("program");

    let checker = program.type_checker();
    for file in program.source_files() {
        if program.is_source_file_default_library(file) || program.is_source_file_from_external_library(file) {
            continue;
        }
        println!("{}", program.file_name(file));
        for child in program.node_children(program.root(file)) {
            visit(&program, &checker, child);
        }
    }
    for diagnostic in program.pre_emit_diagnostics(None) {
        let file = diagnostic.file.map(|file| program.file_name(file).to_string()).unwrap_or_default();
        println!("{file} TS{} {}", diagnostic.code, diagnostic.message_text);
    }
}

fn visit(program: &Program, checker: &TypeChecker, node: NodeId) {
    if let Some(symbol) = checker.symbol_at_location(node) {
        let ty = checker.type_of_symbol_at_location(symbol, node);
        let position = program.line_and_character_of_position(node.source_file(), program.node_start(node));
        println!(
            "  {}:{} {} {}",
            position.line + 1,
            position.character + 1,
            checker.symbol_name(symbol),
            checker.type_to_string(ty)
        );
    }
    for child in program.node_children(node) {
        visit(program, checker, child);
    }
}
