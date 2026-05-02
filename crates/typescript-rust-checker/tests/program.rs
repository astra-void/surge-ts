use typescript_rust_checker::{
    CheckerOptions, SourceFileInput, check_program, check_program_with_options, check_source,
    check_source_with_options,
};

fn codes(diagnostics: &[typescript_rust_diagnostics::Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.to_string())
        .collect()
}

fn file_names(diagnostics: &[typescript_rust_diagnostics::Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.file_name.clone())
        .collect()
}

fn program(files: &[(&str, &str)]) -> Vec<typescript_rust_diagnostics::Diagnostic> {
    check_program(
        files
            .iter()
            .map(|(file_name, source_text)| SourceFileInput {
                file_name: (*file_name).to_string(),
                source_text: (*source_text).to_string(),
            })
            .collect(),
    )
}

fn program_with_options(
    files: &[(&str, &str)],
    options: CheckerOptions,
) -> Vec<typescript_rust_diagnostics::Diagnostic> {
    check_program_with_options(
        files
            .iter()
            .map(|(file_name, source_text)| SourceFileInput {
                file_name: (*file_name).to_string(),
                source_text: (*source_text).to_string(),
            })
            .collect(),
        options,
    )
}

#[test]
fn program_empty_files_valid() {
    assert!(check_program(Vec::new()).is_empty());
}

#[test]
fn program_single_file_matches_check_source_for_basic_valid() {
    let source = "let value: string = \"Ada\";";
    let program_diagnostics = program(&[("example.ts", source)]);
    let single_file_diagnostics = check_source(source, "example.ts");

    assert_eq!(codes(&program_diagnostics), codes(&single_file_diagnostics));
}

#[test]
fn program_parser_errors_preserve_file_name() {
    let diagnostics = program(&[("a.ts", "let value: string | = \"ok\";")]);

    assert!(!diagnostics.is_empty());
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_type_alias_cross_file_valid() {
    let diagnostics = program(&[
        ("a.ts", "type Name = string;"),
        ("b.ts", "let value: Name = \"Ada\";"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_type_alias_cross_file_mismatch() {
    let diagnostics = program(&[
        ("a.ts", "type Name = string;"),
        ("b.ts", "let value: Name = 123;"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_interface_cross_file_valid() {
    let diagnostics = program(&[
        ("a.ts", "interface User { name: string; }"),
        ("b.ts", "let user: User = { name: \"Ada\" };"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_interface_forward_reference_across_files_valid() {
    let diagnostics = program(&[
        ("a.ts", "let user: User = { name: \"Ada\" };"),
        ("b.ts", "interface User { name: string; }"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_type_alias_forward_reference_across_files_valid() {
    let diagnostics = program(&[
        ("a.ts", "let value: Name = \"Ada\";"),
        ("b.ts", "type Name = string;"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_duplicate_type_alias_across_files_ts2300() {
    let diagnostics = program(&[
        ("a.ts", "type Name = string;"),
        ("b.ts", "type Name = number; let value: Name = \"Ada\";"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2300"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_duplicate_interface_across_files_ts2300() {
    let diagnostics = program(&[
        ("a.ts", "interface User { name: string; }"),
        ("b.ts", "interface User { name: number; }"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2300"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_duplicate_alias_interface_across_files_ts2300() {
    let diagnostics = program(&[
        ("a.ts", "type User = { name: string };"),
        ("b.ts", "interface User { name: string; }"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2300"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_duplicate_interface_alias_across_files_ts2300() {
    let diagnostics = program(&[
        ("a.ts", "interface User { name: string; }"),
        ("b.ts", "type User = { name: string };"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2300"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_function_cross_file_call_valid() {
    let diagnostics = program(&[
        ("a.ts", "function getName(): string { return \"Ada\"; }"),
        ("b.ts", "let value: string = getName();"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_function_cross_file_call_return_mismatch() {
    let diagnostics = program(&[
        ("a.ts", "function getName(): string { return \"Ada\"; }"),
        ("b.ts", "let value: number = getName();"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_function_cross_file_argument_mismatch() {
    let diagnostics = program(&[
        ("a.ts", "function greet(name: string): void { }"),
        ("b.ts", "greet(1);"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2345"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_function_cross_file_wrong_arity() {
    let diagnostics = program(&[
        ("a.ts", "function greet(name: string): void { }"),
        ("b.ts", "greet();"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2554"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_function_forward_reference_across_files_valid() {
    let diagnostics = program(&[
        ("a.ts", "getName();"),
        ("b.ts", "function getName(): string { return \"Ada\"; }"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_duplicate_function_across_files_ts2393() {
    let diagnostics = program(&[
        ("a.ts", "function getName(): string { return \"Ada\"; }"),
        ("b.ts", "function getName(): string { return \"Grace\"; }"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2393"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_duplicate_function_same_file_still_ts2393() {
    let diagnostics = program(&[(
        "a.ts",
        "function getName(): string { return \"Ada\"; }\nfunction getName(): string { return \"Grace\"; }",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2393"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_function_body_uses_cross_file_type_alias_valid() {
    let diagnostics = program(&[
        ("a.ts", "type Name = string;"),
        ("b.ts", "function f(value: Name): Name { return value; }"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_function_body_uses_cross_file_interface_valid() {
    let diagnostics = program(&[
        ("a.ts", "interface User { name: string; }"),
        (
            "b.ts",
            "function f(user: User): string { return user.name; }",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_function_body_uses_cross_file_function_valid() {
    let diagnostics = program(&[
        ("a.ts", "function getName(): string { return \"Ada\"; }"),
        ("b.ts", "function f(): string { return getName(); }"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_tuple_type_cross_file_valid() {
    let diagnostics = program(&[
        ("a.ts", "type Pair = [string, number];"),
        ("b.ts", "let pair: Pair = [\"Ada\", 36];"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_array_type_cross_file_valid() {
    let diagnostics = program(&[
        ("a.ts", "type Names = string[];"),
        ("b.ts", "let names: Names = [\"Ada\"];"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_property_call_cross_file_type_valid() {
    let diagnostics = program(&[
        ("a.ts", "interface Store { getState: () => string; }"),
        (
            "b.ts",
            "let store: Store = { getState: () => \"ok\" }; let value: string = store.getState();",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_top_level_let_not_shared_or_policy_pinned() {
    let diagnostics = program(&[
        ("a.ts", "let name = \"Ada\";"),
        ("b.ts", "let value: string = name;"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_top_level_const_not_shared_or_policy_pinned() {
    let diagnostics = program(&[
        ("a.ts", "const name = \"Ada\";"),
        ("b.ts", "let value: string = name;"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_file_local_variable_does_not_leak() {
    let diagnostics = program(&[
        ("a.ts", "let name = \"Ada\";"),
        ("b.ts", "function f(): string { return name; }"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_diagnostics_are_in_input_file_order() {
    let diagnostics = program(&[
        ("a.ts", "type Name = Missing; let a: Name = 1;"),
        ("b.ts", "type Other = Missing; let b: Other = 2;"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304", "TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts", "b.ts"]);
}

#[test]
fn program_no_cascade_unknown_cross_file_type() {
    let diagnostics = program(&[
        ("a.ts", "type Name = Missing;"),
        ("b.ts", "let value: Name = 1;"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_api_empty_files_valid() {
    assert!(program(&[]).is_empty());
}

#[test]
fn program_api_single_file_matches_check_source_valid() {
    let source = "let value: string = \"Ada\";";
    let program_diagnostics = program(&[("example.ts", source)]);
    let single_file_diagnostics = check_source(source, "example.ts");

    assert_eq!(codes(&program_diagnostics), codes(&single_file_diagnostics));
    assert_eq!(
        file_names(&program_diagnostics),
        file_names(&single_file_diagnostics)
    );
}

#[test]
fn program_api_single_file_matches_check_source_mismatch() {
    let source = "let value: string = 123;";
    let program_diagnostics = program(&[("example.ts", source)]);
    let single_file_diagnostics = check_source(source, "example.ts");

    assert_eq!(codes(&program_diagnostics), codes(&single_file_diagnostics));
    assert_eq!(
        file_names(&program_diagnostics),
        file_names(&single_file_diagnostics)
    );
}

#[test]
fn program_api_single_file_no_implicit_any_matches_check_source_with_options() {
    let source = "function f(value): string { return \"ok\"; }";
    let program_diagnostics = program_with_options(
        &[("example.ts", source)],
        CheckerOptions {
            no_implicit_any: true,
        },
    );
    let single_file_diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: true,
        },
    );

    assert_eq!(codes(&program_diagnostics), codes(&single_file_diagnostics));
    assert_eq!(
        file_names(&program_diagnostics),
        file_names(&single_file_diagnostics)
    );
}

#[test]
fn program_api_preserves_input_file_names() {
    let diagnostics = program(&[
        ("src/a.ts", "type Name = Missing;"),
        ("src/b.ts", "let value: Name = 1;"),
    ]);

    assert_eq!(file_names(&diagnostics), vec!["src/a.ts"]);
}

#[test]
fn program_api_accepts_owned_source_file_inputs() {
    let diagnostics = check_program(vec![SourceFileInput {
        file_name: "a.ts".into(),
        source_text: "let value: string = 123;".into(),
    }]);

    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_order_parser_before_type_prepass() {
    let diagnostics = program_with_options(
        &[
            ("a.ts", "let value: string | = \"bad\";"),
            ("b.ts", "type Name = string; type Name = number;"),
            ("c.ts", "function f(value): string { return 123; }"),
        ],
        CheckerOptions {
            no_implicit_any: true,
        },
    );

    assert_eq!(
        codes(&diagnostics),
        vec![
            "typescript-rust::parser-error",
            "TS2300",
            "TS7006",
            "TS2322"
        ]
    );
    assert_eq!(
        file_names(&diagnostics),
        vec!["a.ts", "b.ts", "c.ts", "c.ts"]
    );
}

#[test]
fn program_order_statement_errors_in_input_file_order() {
    let diagnostics = program(&[
        ("a.ts", "let value: number = \"a\";"),
        ("b.ts", "let value: number = \"b\";"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2322", "TS2322"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts", "b.ts"]);
}

#[test]
fn program_file_name_duplicate_function() {
    let diagnostics = program(&[
        ("a.ts", "function getValue(): string { return \"Ada\"; }"),
        ("b.ts", "function getValue(): number { return \"Ada\"; }"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2393", "TS2322"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts", "b.ts"]);
}

#[test]
fn program_function_second_body_checked_against_own_signature() {
    let diagnostics = program(&[
        ("a.ts", "function getValue(): string { return \"Ada\"; }"),
        ("b.ts", "function getValue(): number { return \"Ada\"; }"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2393", "TS2322"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts", "b.ts"]);
}

#[test]
fn program_function_first_signature_wins_for_calls() {
    let diagnostics = program(&[
        ("a.ts", "function getValue(): string { return \"Ada\"; }"),
        ("b.ts", "function getValue(): number { return \"Ada\"; }"),
        ("c.ts", "let value: number = getValue();"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2393", "TS2322", "TS2322"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts", "b.ts", "c.ts"]);
}

#[test]
fn program_top_level_variable_not_shared() {
    let diagnostics = program(&[
        ("a.ts", "let name = \"Ada\";"),
        ("b.ts", "let value: string = name;"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_statement_file_starts_from_global_symbols() {
    let diagnostics = program(&[
        ("a.ts", "function getName(): string { return \"Ada\"; }"),
        ("b.ts", "let first = getName(); let second: string = first;"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_no_cascade_unknown_cross_file_function_return() {
    let diagnostics = program(&[
        ("a.ts", "function take(value: Missing): void { }"),
        ("b.ts", "take(123);"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_file_name_cross_file_function_return_mismatch_declaration() {
    let diagnostics = program(&[
        ("a.ts", "function getName(): string { return 123; }"),
        ("b.ts", "let value: string = getName();"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_file_name_unknown_type_inside_function_signature_file() {
    let diagnostics = program(&[("a.ts", "function take(value: Missing): void { }")]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_function_body_can_see_same_file_top_level_variables_current_policy() {
    let diagnostics = program(&[(
        "a.ts",
        "let name = \"Ada\"; function f(): string { return name; }",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_exported_interface_does_not_contribute_to_global_script() {
    let diagnostics = program(&[
        ("a.ts", "export interface User { name: string; }"),
        ("b.ts", "let user: User = { name: \"Ada\" };"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_exported_type_alias_does_not_contribute_to_global_script() {
    let diagnostics = program(&[
        ("a.ts", "export type Name = string;"),
        ("b.ts", "let value: Name = \"Ada\";"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_exported_function_does_not_contribute_to_global_script() {
    let diagnostics = program(&[
        (
            "a.ts",
            "export function getName(): string { return \"Ada\"; }",
        ),
        ("b.ts", "let value: string = getName();"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_module_file_local_exported_interface_visible_same_file() {
    let diagnostics = program(&[(
        "a.ts",
        "export interface User { name: string; }\nlet user: User = { name: \"Ada\" };",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_file_local_exported_type_alias_visible_same_file() {
    let diagnostics = program(&[(
        "a.ts",
        "export type Name = string;\nlet value: Name = \"Ada\";",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_file_local_exported_function_visible_same_file() {
    let diagnostics = program(&[(
        "a.ts",
        "export function getName(): string { return \"Ada\"; }\nlet value: string = getName();",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_import_named_does_not_bind_type_yet() {
    let diagnostics = program(&[(
        "a.ts",
        "import { User } from \"./user\";\nlet user: User = { name: \"Ada\" };",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_import_named_does_not_bind_value_yet() {
    let diagnostics = program(&[(
        "a.ts",
        "import { getName } from \"./user\";\nlet value: string = getName();",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_import_type_named_does_not_bind_type_yet() {
    let diagnostics = program(&[(
        "a.ts",
        "import type { User } from \"./user\";\nlet user: User = { name: \"Ada\" };",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_import_side_effect_no_diagnostic() {
    let diagnostics = program(&[("a.ts", "import \"./setup\";\nlet value: string = \"ok\";")]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_empty_export_does_not_emit_diagnostic() {
    let diagnostics = program(&[("a.ts", "export {};\nlet value: string = \"ok\";")]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_file_does_not_see_script_global_current_policy() {
    let diagnostics = program(&[
        ("a.ts", "interface User { name: string; }"),
        ("b.ts", "export {};\nlet user: User = { name: \"Ada\" };"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_script_file_does_not_see_module_export_current_policy() {
    let diagnostics = program(&[
        ("a.ts", "export interface User { name: string; }"),
        ("b.ts", "let user: User = { name: \"Ada\" };"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_module_duplicate_type_is_file_local_or_policy_pinned() {
    let diagnostics = program(&[
        ("a.ts", "export interface User { name: string; }"),
        ("b.ts", "export interface User { name: number; }"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_duplicate_function_is_file_local_or_policy_pinned() {
    let diagnostics = program(&[
        (
            "a.ts",
            "export function getName(): string { return \"Ada\"; }",
        ),
        ("b.ts", "export function getName(): number { return 1; }"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_imported_type_unresolved_no_cascade() {
    let diagnostics = program(&[(
        "a.ts",
        "import { User } from \"./user\";\nlet user: User = { name: 123 };",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
}

#[test]
fn program_imported_function_unresolved_no_cascade() {
    let diagnostics = program(&[(
        "a.ts",
        "import { getName } from \"./user\";\nlet value: string = getName();",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
}

#[test]
fn program_exported_unknown_type_no_cascade() {
    let diagnostics = program(&[("a.ts", "export type Name = Missing;\nlet value: Name = 1;")]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_parser_import_export_recovery_does_not_stop_other_files() {
    let diagnostics = program(&[
        ("a.ts", "import { User from \"./user\";"),
        ("b.ts", "let value: string = \"ok\";"),
    ]);

    assert!(!diagnostics.is_empty());
    assert_eq!(file_names(&diagnostics)[0], "a.ts");
}
