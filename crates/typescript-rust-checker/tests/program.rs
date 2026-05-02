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
fn program_import_named_relative_interface_valid() {
    let diagnostics = program(&[
        ("user.ts", "export interface User { name: string; }"),
        (
            "a.ts",
            "import { User } from \"./user\";\nlet user: User = { name: \"Ada\" };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_import_named_relative_function_valid() {
    let diagnostics = program(&[
        (
            "user.ts",
            "export function getName(): string { return \"Ada\"; }",
        ),
        (
            "a.ts",
            "import { getName } from \"./user\";\nlet value: string = getName();",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_import_type_named_relative_type_alias_valid() {
    let diagnostics = program(&[
        ("user.ts", "export type UserId = string;"),
        (
            "a.ts",
            "import type { UserId } from \"./user\";\nlet id: UserId = \"u1\";",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_import_side_effect_relative_valid() {
    let diagnostics = program(&[
        ("setup.ts", "export const initialized: boolean = true;"),
        ("a.ts", "import \"./setup\";\nlet value: string = \"ok\";"),
    ]);

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
fn program_module_file_does_not_see_script_type_alias_current_policy() {
    let diagnostics = program(&[
        ("a.ts", "type Name = string;"),
        ("b.ts", "export {};\nlet value: Name = \"Ada\";"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_module_file_does_not_see_script_function_current_policy() {
    let diagnostics = program(&[
        ("a.ts", "function getName(): string { return \"Ada\"; }"),
        ("b.ts", "export {};\nlet value: string = getName();"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_empty_export_isolates_file_from_script_globals() {
    let diagnostics = program(&[
        ("a.ts", "interface User { name: string; }"),
        ("b.ts", "export {};\nlet user: User = { name: \"Ada\" };"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_side_effect_import_isolates_file_from_script_globals() {
    let diagnostics = program(&[
        ("a.ts", "interface User { name: string; }"),
        ("setup.ts", "export {};"),
        (
            "b.ts",
            "import \"./setup\";\nlet user: User = { name: \"Ada\" };",
        ),
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
fn program_script_files_still_share_interface() {
    let diagnostics = program(&[
        ("a.ts", "interface User { name: string; }"),
        ("b.ts", "let user: User = { name: \"Ada\" };"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_script_files_still_share_type_alias() {
    let diagnostics = program(&[
        ("a.ts", "type Name = string;"),
        ("b.ts", "let value: Name = \"Ada\";"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_script_files_still_share_function() {
    let diagnostics = program(&[
        ("a.ts", "function getName(): string { return \"Ada\"; }"),
        ("b.ts", "let value: string = getName();"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_exported_interface_not_global() {
    let diagnostics = program(&[
        ("a.ts", "export interface User { name: string; }"),
        ("b.ts", "let user: User = { name: \"Ada\" };"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_module_exported_type_alias_not_global() {
    let diagnostics = program(&[
        ("a.ts", "export type Name = string;"),
        ("b.ts", "let value: Name = \"Ada\";"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_module_exported_function_not_global() {
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
fn program_module_exported_variable_not_global() {
    let diagnostics = program(&[
        ("a.ts", "export const value: string = \"Ada\";"),
        ("b.ts", "let other: string = value;"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_script_file_does_not_see_module_exported_interface() {
    let diagnostics = program(&[
        ("a.ts", "export interface User { name: string; }"),
        ("b.ts", "let user: User = { name: \"Ada\" };"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_script_file_does_not_see_module_exported_type_alias() {
    let diagnostics = program(&[
        ("a.ts", "export type Name = string;"),
        ("b.ts", "let value: Name = \"Ada\";"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_script_file_does_not_see_module_exported_function() {
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
fn program_imported_type_unresolved_no_cascade() {
    let diagnostics = program(&[(
        "a.ts",
        "import { User } from \"./user\";\nlet user: User = { name: 123 };",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
}

#[test]
fn program_imported_function_unresolved_no_cascade() {
    let diagnostics = program(&[(
        "a.ts",
        "import { getName } from \"./user\";\nlet value: string = getName();",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
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

#[test]
fn program_module_file_local_exported_const_visible_same_file() {
    let diagnostics = program(&[(
        "a.ts",
        "export const value: string = \"Ada\";\nlet other: string = value;",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_file_local_exported_let_visible_same_file() {
    let diagnostics = program(&[(
        "a.ts",
        "export let value: string = \"Ada\";\nlet other: string = value;",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_file_local_exported_var_visible_same_file() {
    let diagnostics = program(&[(
        "a.ts",
        "export var value: string = \"Ada\";\nlet other: string = value;",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_file_local_non_exported_interface_visible_same_module_file() {
    let diagnostics = program(&[(
        "a.ts",
        "export {};\ninterface User { name: string; }\nlet user: User = { name: \"Ada\" };",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_file_local_non_exported_type_alias_visible_same_module_file() {
    let diagnostics = program(&[(
        "a.ts",
        "export {};\ntype Name = string;\nlet value: Name = \"Ada\";",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_file_local_non_exported_function_visible_same_module_file() {
    let diagnostics = program(&[(
        "a.ts",
        "export {};\nfunction getName(): string { return \"Ada\"; }\nlet value: string = getName();",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_type_alias_forward_reference_valid() {
    let diagnostics = program(&[(
        "a.ts",
        "export {};\nlet value: Name = \"Ada\";\ntype Name = string;",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_interface_forward_reference_valid() {
    let diagnostics = program(&[(
        "a.ts",
        "export {};\nlet value: User = { name: \"Ada\" };\ninterface User { name: string; }",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_export_function_parameter_no_implicit_any() {
    let diagnostics = program_with_options(
        &[(
            "a.ts",
            "export function f(value): string { return \"ok\"; }",
        )],
        CheckerOptions {
            no_implicit_any: true,
        },
    );

    assert_eq!(codes(&diagnostics), vec!["TS7006"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_module_export_variable_initializer_mismatch() {
    let diagnostics = program(&[("a.ts", "export const value: string = 123;")]);

    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_module_export_variable_duplicate_let_same_file() {
    let diagnostics = program(&[(
        "a.ts",
        "export let value: string = \"Ada\";\nexport let value: string = \"Grace\";",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2451"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_module_export_type_alias_unknown_type_no_cascade() {
    let diagnostics = program(&[("a.ts", "export type Name = Missing;\nlet value: Name = 1;")]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_module_export_interface_unknown_property_no_cascade() {
    let diagnostics = program(&[(
        "a.ts",
        "export interface User { name: string; }\nlet user: User = { name: \"Ada\" };\nlet value: string = user.missing;",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2339"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_module_export_function_unknown_return_no_missing_return_cascade() {
    let diagnostics = program(&[(
        "a.ts",
        "export function getName(): string { return Missing; }",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_module_export_function_unknown_parameter_no_argument_cascade() {
    let diagnostics = program(&[(
        "a.ts",
        "export function greet(name: Missing): void {}\ngreet(1);",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_module_import_alias_type_unresolved_local_name() {
    let diagnostics = program(&[(
        "a.ts",
        "import { User as UserModel } from \"./user\";\nlet user: UserModel = { name: \"Ada\" };",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_module_import_alias_value_unresolved_local_name() {
    let diagnostics = program(&[(
        "a.ts",
        "import { getName as getUserName } from \"./user\";\nlet value: string = getUserName();",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_module_import_side_effect_no_unresolved_name() {
    let diagnostics = program(&[
        ("setup.ts", "export {};"),
        ("a.ts", "import \"./setup\";\nlet value: string = \"ok\";"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_import_side_effect_script_file_valid() {
    let diagnostics = program(&[
        ("setup.ts", "let initialized: boolean = true;"),
        ("a.ts", "import \"./setup\";\nlet value: string = \"ok\";"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_import_regular_type_export_type_usage_valid() {
    let diagnostics = program(&[
        ("user.ts", "export interface User { name: string; }"),
        (
            "index.ts",
            "import { User } from \"./user\";\nlet user: User = { name: \"Ada\" };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_import_regular_type_export_value_usage_unresolved() {
    let diagnostics = program(&[
        ("user.ts", "export interface User { name: string; }"),
        (
            "index.ts",
            "import { User } from \"./user\";\nlet value = User;",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["index.ts"]);
}

#[test]
fn program_module_import_regular_value_export_value_usage_valid() {
    let diagnostics = program(&[
        ("user.ts", "export const User: string = \"Ada\";"),
        (
            "index.ts",
            "import { User } from \"./user\";\nlet value: string = User;",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_import_regular_const_export_assignment_rejected() {
    let diagnostics = program(&[
        ("user.ts", "export const value: string = \"Ada\";"),
        (
            "index.ts",
            "import { value } from \"./user\";\nvalue = \"Grace\";",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2588"]);
    assert_eq!(file_names(&diagnostics), vec!["index.ts"]);
}

#[test]
fn program_module_import_regular_same_name_type_and_value_binds_both() {
    let diagnostics = program(&[
        (
            "user.ts",
            "export interface User { name: string; }\nexport const User: string = \"Ada\";",
        ),
        (
            "index.ts",
            "import { User } from \"./user\";\nlet user: User = { name: \"Ada\" };\nlet value: string = User;",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_import_regular_alias_type_usage_valid() {
    let diagnostics = program(&[
        ("user.ts", "export interface User { name: string; }"),
        (
            "index.ts",
            "import { User as UserModel } from \"./user\";\nlet user: UserModel = { name: \"Ada\" };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_import_regular_alias_value_usage_valid() {
    let diagnostics = program(&[
        ("user.ts", "export const User: string = \"Ada\";"),
        (
            "index.ts",
            "import { User as UserModel } from \"./user\";\nlet value: string = UserModel;",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_import_regular_alias_type_export_value_usage_unresolved() {
    let diagnostics = program(&[
        ("user.ts", "export interface User { name: string; }"),
        (
            "index.ts",
            "import { User as UserModel } from \"./user\";\nlet value = UserModel;",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["index.ts"]);
}

#[test]
fn program_module_import_regular_alias_value_export_type_usage_unresolved() {
    let diagnostics = program(&[
        ("user.ts", "export const User: string = \"Ada\";"),
        (
            "index.ts",
            "import { User as UserModel } from \"./user\";\nlet value: UserModel = \"Ada\";",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["index.ts"]);
}

#[test]
fn program_module_import_regular_value_export_type_usage_unresolved() {
    let diagnostics = program(&[
        ("user.ts", "export const User: string = \"Ada\";"),
        (
            "index.ts",
            "import { User } from \"./user\";\nlet value: User = \"Ada\";",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["index.ts"]);
}

#[test]
fn program_module_import_type_type_export_value_usage_unresolved() {
    let diagnostics = program(&[
        ("user.ts", "export type Name = string;"),
        (
            "index.ts",
            "import type { Name } from \"./user\";\nlet name: Name = \"Ada\";\nlet value = Name;",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["index.ts"]);
}

#[test]
fn program_module_import_regular_missing_export_no_cascade() {
    let diagnostics = program(&[
        ("user.ts", "export interface User { name: string; }"),
        (
            "index.ts",
            "import { Missing } from \"./user\";\nlet value: Missing = 123;",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2305"]);
    assert_eq!(file_names(&diagnostics), vec!["index.ts"]);
}

#[test]
fn program_module_import_missing_relative_no_cascade() {
    let diagnostics = program(&[(
        "index.ts",
        "import { User } from \"./missing\";\nlet user: User = { name: 123 };",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
    assert_eq!(file_names(&diagnostics), vec!["index.ts"]);
}

#[test]
fn program_module_named_import_from_script_file_reports_missing_export() {
    let diagnostics = program(&[
        ("setup.ts", "let value = 1;"),
        ("index.ts", "import { value } from \"./setup\";"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2305"]);
    assert_eq!(file_names(&diagnostics), vec!["index.ts"]);
}

#[test]
fn program_module_imported_value_unresolved_no_operator_cascade() {
    let diagnostics = program(&[(
        "a.ts",
        "import { getCount } from \"./count\";\nlet value: number = getCount() + 1;",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_module_imported_property_receiver_unresolved_no_property_cascade() {
    let diagnostics = program(&[(
        "a.ts",
        "import { store } from \"./store\";\nlet value: string = store.getName();",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_module_order_module_export_error_before_consumer_statement_error() {
    let diagnostics = program(&[
        ("a.ts", "export type Name = Missing;\nlet value: Name = 1;"),
        ("b.ts", "let value: number = \"bad\";"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304", "TS2322"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts", "b.ts"]);
}

#[test]
fn program_module_order_module_import_error_before_consumer_statement_error() {
    let diagnostics = program(&[
        ("a.ts", "import { User } from \"./missing\";"),
        ("b.ts", "let value: string = \"ok\";"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_module_order_all_import_errors_before_all_statement_errors() {
    let diagnostics = program(&[
        ("user.ts", "export interface User { name: string; }"),
        (
            "a.ts",
            "import { Missing, AlsoMissing } from \"./user\";\nlet value: Missing = 123;",
        ),
        ("b.ts", "let value: number = \"bad\";"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2305", "TS2305", "TS2322"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts", "a.ts", "b.ts"]);
}

#[test]
fn program_order_parser_error_before_module_resolution_error() {
    let diagnostics = program(&[
        ("a.ts", "import { User from \"./user\";"),
        ("b.ts", "import { User } from \"./missing\";"),
    ]);

    assert_eq!(
        codes(&diagnostics),
        vec!["typescript-rust::parser-error", "TS2307"]
    );
    assert_eq!(file_names(&diagnostics), vec!["a.ts", "b.ts"]);
}

#[test]
fn program_module_function_forward_reference_valid() {
    let diagnostics = program(&[(
        "a.ts",
        "export {};\nlet value: string = getName();\nfunction getName(): string { return \"Ada\"; }",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_export_function_forward_reference_valid() {
    let diagnostics = program(&[(
        "a.ts",
        "export function getName(): string { return \"Ada\"; }\nlet value: string = getName();",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_duplicate_function_same_module_file_ts2393() {
    let diagnostics = program(&[(
        "a.ts",
        "export {};\nfunction getValue(): string { return \"Ada\"; }\nfunction getValue(): number { return 1; }",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2393"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_module_duplicate_export_function_same_module_file_ts2393() {
    let diagnostics = program(&[(
        "a.ts",
        "export function getValue(): string { return \"Ada\"; }\nexport function getValue(): number { return 1; }",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2393"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_module_duplicate_type_alias_same_module_file_ts2300() {
    let diagnostics = program(&[(
        "a.ts",
        "export {};\ntype Name = string;\ntype Name = number;\nlet value: Name = \"Ada\";",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2300"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_module_duplicate_interface_same_module_file_ts2300() {
    let diagnostics = program(&[(
        "a.ts",
        "export {};\ninterface User { name: string; }\ninterface User { name: number; }",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2300"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_module_export_named_existing_no_diagnostic() {
    let diagnostics = program(&[("a.ts", "type User = string;\nexport { User };")]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_export_list_value_name_import_valid() {
    let diagnostics = program(&[
        ("a.ts", "const value: string = \"Ada\";\nexport { value };"),
        (
            "b.ts",
            "import { value } from \"./a\";\nlet copy: string = value;",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_export_list_type_name_import_valid() {
    let diagnostics = program(&[
        ("a.ts", "type User = { name: string };\nexport { User };"),
        (
            "b.ts",
            "import { User } from \"./a\";\nlet user: User = { name: \"Ada\" };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_export_type_list_exports_type_only() {
    let diagnostics = program(&[
        (
            "a.ts",
            "type User = { name: string };\nexport type { User };",
        ),
        (
            "b.ts",
            "import type { User } from \"./a\";\nlet user: User = { name: \"Ada\" };\nlet value = User;",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_module_export_named_missing_no_diagnostic_current_policy() {
    let diagnostics = program(&[("a.ts", "export { Missing };")]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_module_export_list_missing_local_no_cascade() {
    let diagnostics = program(&[("a.ts", "export { Missing }; let value: string = \"ok\";")]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_module_export_type_named_missing_no_diagnostic_current_policy() {
    let diagnostics = program(&[("a.ts", "export type { Missing };")]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_module_exported_private_type_dependency_valid() {
    let diagnostics = program(&[
        (
            "a.ts",
            "interface InternalUser { name: string; }\nexport type Box = { user: InternalUser };",
        ),
        (
            "b.ts",
            "import { Box } from \"./a\";\nlet box: Box = { user: { name: \"Ada\" } };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_module_exported_private_type_dependency_cycle_no_stack_overflow() {
    let diagnostics = program(&[
        (
            "a.ts",
            "interface A { next: B; }\ninterface B { next: A; }\nexport type Box = A;",
        ),
        (
            "b.ts",
            "import { Box } from \"./a\";\nlet box: Box = { next: { next: undefined } };",
        ),
    ]);

    assert!(!diagnostics.is_empty());
}

#[test]
fn program_module_export_named_does_not_make_global() {
    let diagnostics = program(&[
        ("a.ts", "type User = string;\nexport { User };"),
        ("b.ts", "let value: User = \"Ada\";"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn module_non_relative_named_import_ts2307() {
    let diagnostics = program(&[(
        "index.ts",
        "import { User } from \"pkg\";\nlet user: User = { name: \"Ada\" };",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
    assert_eq!(file_names(&diagnostics), vec!["index.ts"]);
}

#[test]
fn module_non_relative_type_import_ts2307() {
    let diagnostics = program(&[(
        "index.ts",
        "import type { StoreApi } from \"pkg\";\nlet store: StoreApi = { getState: 123 };",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
    assert_eq!(file_names(&diagnostics), vec!["index.ts"]);
}

#[test]
fn module_non_relative_side_effect_import_ts2307() {
    let diagnostics = program(&[("index.ts", "import \"pkg\";\nlet ok: string = \"ok\";")]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
    assert_eq!(file_names(&diagnostics), vec!["index.ts"]);
}

#[test]
fn module_non_relative_no_cascade_type_usage() {
    let diagnostics = program(&[(
        "index.ts",
        "import { User } from \"pkg\";\nlet user: User = { name: 123 };",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
}

#[test]
fn module_non_relative_no_cascade_value_usage() {
    let diagnostics = program(&[(
        "index.ts",
        "import { createStore } from \"zustand/vanilla\";\nlet store = createStore();",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
}

#[test]
fn module_non_relative_no_cascade_call_usage() {
    let diagnostics = program(&[(
        "index.ts",
        "import { createStore } from \"zustand/vanilla\";\ncreateStore();",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
}

#[test]
fn module_default_import_parser_safe_or_pinned() {
    let diagnostics = program(&[
        (
            "user.ts",
            "export default function getName(): string { return \"Ada\"; }",
        ),
        (
            "index.ts",
            "import DefaultThing from \"./user\";\nlet name: string = DefaultThing();",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn module_default_import_missing_default_export() {
    let diagnostics = program(&[
        ("user.ts", "export const getName: string = \"Ada\";"),
        (
            "index.ts",
            "import getName from \"./user\";\nlet name = getName;",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2305"]);
}

#[test]
fn module_default_import_missing_module() {
    let diagnostics = program(&[(
        "index.ts",
        "import getName from \"./missing\";\nlet name = getName;",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
}

#[test]
fn module_default_import_does_not_bind_type() {
    let diagnostics = program(&[
        ("user.ts", "export default 123;"),
        (
            "index.ts",
            "import DefaultThing from \"./user\";\nlet value: DefaultThing = 123;",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
}

#[test]
fn module_default_import_non_relative_unsupported() {
    let diagnostics = program(&[(
        "index.ts",
        "import DefaultThing from \"react\";\nlet value = DefaultThing;",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
}

#[test]
fn module_default_import_mixed_named_parser_safe_or_pinned() {
    let diagnostics = program(&[(
        "index.ts",
        "import DefaultThing, { named } from \"./thing\";",
    )]);

    assert_eq!(
        codes(&diagnostics),
        vec!["typescript-rust::unsupported-module-syntax"]
    );
}

#[test]
fn module_default_import_single_file_still_unresolved_or_unsupported() {
    let diagnostics = check_source(
        "import DefaultThing from \"./thing\";\nlet value = DefaultThing;",
        "index.ts",
    );

    assert_eq!(
        codes(&diagnostics),
        vec!["typescript-rust::unsupported-module-syntax", "TS2304"]
    );
}

#[test]
fn module_default_import_no_cascade_value_usage() {
    let diagnostics = program(&[
        ("user.ts", "export const getName: string = \"Ada\";"),
        ("index.ts", "import getName from \"./user\";\ngetName;"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2305"]);
}

#[test]
fn module_namespace_import_parser_safe_or_pinned() {
    let diagnostics = program(&[
        (
            "user.ts",
            "export function getName(): string { return \"Ada\"; }",
        ),
        (
            "index.ts",
            "import * as ns from \"./user\";\nlet name: string = ns.getName();",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn module_namespace_import_variable_property_valid() {
    let diagnostics = program(&[
        ("user.ts", "export const version: number = 1;"),
        (
            "index.ts",
            "import * as ns from \"./user\";\nlet version: number = ns.version;",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn module_namespace_import_default_property_valid_or_pinned() {
    let diagnostics = program(&[
        ("user.ts", "export default 123;"),
        (
            "index.ts",
            "import * as ns from \"./user\";\nlet version: number = ns.default;",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn module_namespace_import_missing_property_ts2339() {
    let diagnostics = program(&[
        (
            "user.ts",
            "export function getName(): string { return \"Ada\"; }",
        ),
        (
            "index.ts",
            "import * as ns from \"./user\";\nlet value = ns.missing;",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2339"]);
}

#[test]
fn module_namespace_import_does_not_bind_named_value() {
    let diagnostics = program(&[
        (
            "user.ts",
            "export function getName(): string { return \"Ada\"; }",
        ),
        (
            "index.ts",
            "import * as ns from \"./user\";\nlet value = getName;",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
}

#[test]
fn module_namespace_import_does_not_bind_named_type() {
    let diagnostics = program(&[
        ("user.ts", "export interface User { name: string; }"),
        (
            "index.ts",
            "import * as ns from \"./user\";\nlet value: User = { name: \"Ada\" };",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
}

#[test]
fn module_namespace_import_non_relative_unsupported() {
    let diagnostics = program(&[(
        "index.ts",
        "import * as ns from \"react\";\nlet value = ns;",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
}

#[test]
fn module_mixed_default_named_import_parser_safe() {
    let diagnostics = program(&[(
        "index.ts",
        "import DefaultThing, { named } from \"./thing\";",
    )]);

    assert_eq!(
        codes(&diagnostics),
        vec!["typescript-rust::unsupported-module-syntax"]
    );
}

#[test]
fn module_export_default_expression_parser_safe() {
    let diagnostics = program(&[
        ("user.ts", "export default 123;"),
        (
            "index.ts",
            "import value from \"./user\";\nlet name: number = value;",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn module_export_default_function_parser_safe() {
    let diagnostics = program(&[
        (
            "user.ts",
            "export default function makeThing() { return \"Ada\"; }",
        ),
        (
            "index.ts",
            "import value from \"./user\";\nlet name: string = value();",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn module_export_default_expression_string_import_valid() {
    let diagnostics = program(&[
        ("user.ts", "export default \"Ada\";"),
        (
            "index.ts",
            "import value from \"./user\";\nlet name: string = value;",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn module_export_default_duplicate_pinned() {
    let diagnostics = program(&[("index.ts", "export default 123;\nexport default 456;")]);

    assert_eq!(
        codes(&diagnostics),
        vec!["typescript-rust::duplicate-default-export"]
    );
}

#[test]
fn module_export_default_class_unsupported_no_panic() {
    let diagnostics = program(&[("index.ts", "export default class Foo {}")]);

    assert_eq!(
        codes(&diagnostics),
        vec!["typescript-rust::unsupported-module-syntax"]
    );
}

#[test]
fn module_export_default_function_local_name_policy_pinned() {
    let diagnostics = program(&[(
        "index.ts",
        "export default function getName(): string { return \"Ada\"; }\nlet name: string = getName();",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn module_export_default_function_import_valid() {
    let diagnostics = program(&[
        (
            "user.ts",
            "export default function getName(): string { return \"Ada\"; }",
        ),
        (
            "index.ts",
            "import getName from \"./user\";\nlet name: string = getName();",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn module_export_default_expression_number_import_valid() {
    let diagnostics = program(&[
        ("user.ts", "export default 123;"),
        (
            "index.ts",
            "import value from \"./user\";\nlet name: number = value;",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn module_export_default_expression_boolean_import_valid() {
    let diagnostics = program(&[
        ("user.ts", "export default true;"),
        (
            "index.ts",
            "import value from \"./user\";\nlet name: boolean = value;",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn module_re_export_named_parser_safe_or_pinned() {
    let diagnostics = program(&[
        ("foo.ts", "export interface Foo { name: string; }"),
        ("index.ts", "export { Foo } from \"./foo\";"),
        (
            "app.ts",
            "import { Foo } from \"./index\";\nlet value: Foo = { name: \"Ada\" };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn module_re_export_named_function_valid() {
    let diagnostics = program(&[
        (
            "foo.ts",
            "export function getName(): string { return \"Ada\"; }",
        ),
        ("index.ts", "export { getName } from \"./foo\";"),
        (
            "app.ts",
            "import { getName } from \"./index\";\nlet name: string = getName();",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn module_re_export_named_variable_valid() {
    let diagnostics = program(&[
        ("foo.ts", "export const version: number = 1;"),
        ("index.ts", "export { version } from \"./foo\";"),
        (
            "app.ts",
            "import { version } from \"./index\";\nlet value: number = version;",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn module_re_export_named_alias_valid() {
    let diagnostics = program(&[
        ("foo.ts", "export interface Foo { name: string; }"),
        ("index.ts", "export { Foo as FooModel } from \"./foo\";"),
        (
            "app.ts",
            "import { FooModel } from \"./index\";\nlet value: FooModel = { name: \"Ada\" };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn module_re_export_named_missing_member() {
    let diagnostics = program(&[
        ("foo.ts", "export const version: number = 1;"),
        ("index.ts", "export { Foo } from \"./foo\";"),
        (
            "app.ts",
            "import { Foo } from \"./index\";\nlet value: Foo = { name: \"Ada\" };",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2305"]);
}

#[test]
fn module_re_export_named_missing_module() {
    let diagnostics = program(&[("index.ts", "export { User } from \"./missing\";")]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
}

#[test]
fn module_re_export_named_no_cascade_consumer() {
    let diagnostics = program(&[
        ("foo.ts", "export const version: number = 1;"),
        ("index.ts", "export { Foo } from \"./foo\";"),
        (
            "app.ts",
            "import { Foo } from \"./index\";\nlet value: Foo = 123;",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2305"]);
}

#[test]
fn module_re_export_type_named_parser_safe_or_pinned() {
    let diagnostics = program(&[
        ("foo.ts", "export interface Foo { name: string; }"),
        ("index.ts", "export type { Foo } from \"./foo\";"),
        (
            "app.ts",
            "import type { Foo } from \"./index\";\nlet value: Foo = { name: \"Ada\" };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn module_re_export_type_named_alias_valid() {
    let diagnostics = program(&[
        ("foo.ts", "export interface Foo { name: string; }"),
        (
            "index.ts",
            "export type { Foo as FooModel } from \"./foo\";",
        ),
        (
            "app.ts",
            "import type { FooModel } from \"./index\";\nlet value: FooModel = { name: \"Ada\" };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn module_re_export_type_named_value_only_missing_type() {
    let diagnostics = program(&[
        ("foo.ts", "export const Foo: string = \"Ada\";"),
        ("index.ts", "export type { Foo } from \"./foo\";"),
        (
            "app.ts",
            "import type { Foo } from \"./index\";\nlet value: Foo = { name: \"Ada\" };",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2305"]);
}

#[test]
fn module_re_export_type_named_does_not_export_value() {
    let diagnostics = program(&[
        ("foo.ts", "export interface Foo { name: string; }"),
        ("index.ts", "export type { Foo } from \"./foo\";"),
        (
            "app.ts",
            "import { Foo } from \"./index\";\nlet value = Foo;",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
}

#[test]
fn module_re_export_star_parser_safe_or_pinned() {
    let diagnostics = program(&[
        (
            "foo.ts",
            "export function makeThing(): string { return \"Ada\"; }",
        ),
        ("index.ts", "export * from \"./foo\";"),
        (
            "app.ts",
            "import { makeThing } from \"./index\";\nlet value: string = makeThing();",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn module_re_export_star_missing_module() {
    let diagnostics = program(&[("index.ts", "export * from \"./missing\";")]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
}

#[test]
fn module_re_export_star_does_not_export_default() {
    let diagnostics = program(&[
        (
            "foo.ts",
            "export default function getName(): string { return \"Ada\"; }\nexport const version: number = 1;",
        ),
        ("index.ts", "export * from \"./foo\";"),
        (
            "app.ts",
            "import getName from \"./index\";\nlet value = getName;",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2305"]);
}

#[test]
fn module_re_export_star_conflict_policy_pinned() {
    let diagnostics = program(&[
        ("a.ts", "export const name: string = \"Ada\";"),
        ("b.ts", "export const name: number = 1;"),
        ("index.ts", "export * from \"./a\";\nexport * from \"./b\";"),
        (
            "app.ts",
            "import { name } from \"./index\";\nlet value: string = name;",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn module_re_export_star_local_explicit_wins() {
    let diagnostics = program(&[
        ("other.ts", "export const name: number = 1;"),
        (
            "index.ts",
            "export const name: string = \"Ada\";\nexport * from \"./other\";",
        ),
        (
            "app.ts",
            "import { name } from \"./index\";\nlet value: string = name;",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn module_re_export_star_type_local_explicit_wins() {
    let diagnostics = program(&[
        ("other.ts", "export interface User { other: number; }"),
        (
            "index.ts",
            "export interface User { name: string; }\nexport * from \"./other\";",
        ),
        (
            "app.ts",
            "import { User } from \"./index\";\nlet user: User = { name: \"Ada\" };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn module_re_export_star_missing_module_no_consumer_cascade() {
    let diagnostics = program(&[
        ("index.ts", "export * from \"./missing\";"),
        (
            "app.ts",
            "import { User } from \"./index\";\nlet value = User;",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
}

#[test]
fn module_re_export_star_as_parser_safe_or_pinned() {
    let diagnostics = program(&[("index.ts", "export * as Foo from \"./foo\";")]);

    assert_eq!(
        codes(&diagnostics),
        vec!["typescript-rust::unsupported-module-syntax"]
    );
}

#[test]
fn module_unsupported_syntax_single_diagnostic_no_cascade() {
    let diagnostics = program(&[(
        "index.ts",
        "import DefaultThing from \"./thing\";\nlet ok: string = \"ok\";",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
    assert_eq!(file_names(&diagnostics), vec!["index.ts"]);
}

#[test]
fn program_order_re_export_error_before_importer_statement() {
    let diagnostics = program(&[
        ("user.ts", "export const version: number = 1;"),
        ("index.ts", "export { Foo } from \"./user\";"),
        ("app.ts", "let value: string = 123;"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2305", "TS2322"]);
    assert_eq!(file_names(&diagnostics), vec!["index.ts", "app.ts"]);
}

#[test]
fn program_order_default_export_duplicate_before_statement() {
    let diagnostics = program(&[(
        "index.ts",
        "export default 123;\nexport default 456;\nlet value: string = 123;",
    )]);

    assert_eq!(
        codes(&diagnostics),
        vec!["typescript-rust::duplicate-default-export", "TS2322"]
    );
}

#[test]
fn program_order_namespace_import_error_before_statement() {
    let diagnostics = program(&[(
        "index.ts",
        "import * as ns from \"./missing\";\nlet value: string = 123;",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2307", "TS2322"]);
}

#[test]
fn program_order_star_re_export_missing_module_before_statement() {
    let diagnostics = program(&[(
        "index.ts",
        "export * from \"./missing\";\nlet value: string = 123;",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2307", "TS2322"]);
}
