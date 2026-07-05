use surge_ts_checker::{
    CheckerOptions, DiagnosticProfile, SourceFileInput, check_program, check_program_with_options,
    check_source_with_options,
};

fn codes(diagnostics: &[surge_ts_diagnostics::Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.to_string())
        .collect()
}

fn file_names(diagnostics: &[surge_ts_diagnostics::Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.file_name.clone())
        .collect()
}

fn program(files: &[(&str, &str)]) -> Vec<surge_ts_diagnostics::Diagnostic> {
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

fn native_program(files: &[(&str, &str)]) -> Vec<surge_ts_diagnostics::Diagnostic> {
    let mut options = CheckerOptions::default();
    options.diagnostic_profile = DiagnosticProfile::Native;

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

fn source_with_options(
    source: &str,
    options: CheckerOptions,
) -> Vec<surge_ts_diagnostics::Diagnostic> {
    check_source_with_options(source, "example.ts", options)
}

#[test]
fn generic_type_alias_cross_file_valid() {
    let diagnostics = program(&[
        ("box.ts", "export type Box<T> = { value: T };"),
        (
            "index.ts",
            "import { Box } from \"./box\"; let box: Box<string> = { value: \"ok\" };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_type_alias_cross_file_mismatch() {
    let diagnostics = program(&[
        ("box.ts", "export type Box<T> = { value: T };"),
        (
            "index.ts",
            "import { Box } from \"./box\"; let box: Box<string> = { value: 123 };",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
    assert_eq!(file_names(&diagnostics), vec!["index.ts"]);
}

#[test]
fn generic_interface_cross_file_valid() {
    let diagnostics = program(&[
        ("box.ts", "export interface Box<T> { value: T; }"),
        (
            "index.ts",
            "import { Box } from \"./box\"; let box: Box<string> = { value: \"ok\" };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_interface_cross_file_mismatch() {
    let diagnostics = program(&[
        ("box.ts", "export interface Box<T> { value: T; }"),
        (
            "index.ts",
            "import { Box } from \"./box\"; let box: Box<string> = { value: 123 };",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
    assert_eq!(file_names(&diagnostics), vec!["index.ts"]);
}

#[test]
fn generic_type_alias_missing_argument_ts2314() {
    let diagnostics = program(&[
        ("box.ts", "type Box<T> = { value: T };"),
        ("index.ts", "let box: Box = { value: \"ok\" };"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2314"]);
    assert_eq!(file_names(&diagnostics), vec!["index.ts"]);
}

#[test]
fn generic_type_alias_too_many_arguments_ts2314() {
    let diagnostics = program(&[
        ("box.ts", "type Box<T> = { value: T };"),
        (
            "index.ts",
            "let box: Box<string, number> = { value: \"ok\" };",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2314"]);
    assert_eq!(file_names(&diagnostics), vec!["index.ts"]);
}

#[test]
fn generic_type_alias_unknown_type_argument_no_cascade() {
    let diagnostics = program(&[
        ("box.ts", "type Box<T> = { value: T };"),
        ("index.ts", "let box: Box<Missing> = { value: 123 };"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["index.ts"]);
}

#[test]
fn generic_nested_type_reference_valid() {
    let diagnostics = program(&[
        (
            "box.ts",
            "type Box<T> = { value: T }; type Outer<T> = { inner: Box<T> };",
        ),
        (
            "index.ts",
            "let outer: Outer<string> = { inner: { value: \"ok\" } };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_function_declaration_parser_safe() {
    let diagnostics = program(&[(
        "index.ts",
        "function identity<T>(value: T): T { return value; }",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_call_type_arguments_ignored_or_pinned() {
    let diagnostics = program(&[(
        "index.ts",
        "function makeString(): string { return \"ok\"; } let value: string = makeString<string>();",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_module_import_valid() {
    let diagnostics = program(&[
        ("box.ts", "export type Box<T> = { value: T };"),
        (
            "index.ts",
            "import { Box } from \"./box\"; let box: Box<string> = { value: \"ok\" };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_module_private_helper_type_importable_or_pinned() {
    let diagnostics = program(&[
        (
            "box.ts",
            "type Internal<T> = { value: T }; export type Box<T> = Internal<T>;",
        ),
        ("index.ts", "import { Internal } from \"./box\";"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_type_alias_number_valid() {
    let diagnostics = program(&[
        ("box.ts", "type Box<T> = { value: T };"),
        ("index.ts", "let box: Box<number> = { value: 123 };"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_type_alias_boolean_valid() {
    let diagnostics = program(&[
        ("box.ts", "type Box<T> = { value: T };"),
        ("index.ts", "let box: Box<boolean> = { value: true };"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_type_alias_literal_argument_valid() {
    let diagnostics = program(&[
        ("box.ts", "type Box<T> = { value: T };"),
        ("index.ts", "let box: Box<\"ok\"> = { value: \"ok\" };"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_type_alias_object_argument_valid() {
    let diagnostics = program(&[
        ("box.ts", "type Box<T> = { value: T };"),
        (
            "index.ts",
            "let box: Box<{ name: string }> = { value: { name: \"Ada\" } };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_type_alias_tuple_argument_valid() {
    let diagnostics = program(&[
        ("box.ts", "type Box<T> = { value: T };"),
        (
            "index.ts",
            "let box: Box<[string, number]> = { value: [\"Ada\", 1] };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_type_alias_array_argument_valid() {
    let diagnostics = program(&[
        ("box.ts", "type Box<T> = { value: T };"),
        ("index.ts", "let box: Box<string[]> = { value: [\"ok\"] };"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_type_alias_union_argument_valid() {
    let diagnostics = program(&[
        ("box.ts", "type Box<T> = { value: T };"),
        (
            "index.ts",
            "let box: Box<string | number> = { value: 123 };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_type_alias_function_argument_valid() {
    let diagnostics = program(&[
        ("box.ts", "type Box<T> = { value: T };"),
        (
            "index.ts",
            "function getValue(): string { return \"ok\"; } let box: Box<() => string> = { value: getValue };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_type_alias_nested_generic_valid() {
    let diagnostics = program(&[
        (
            "box.ts",
            "type Box<T> = { value: T }; type Outer<T> = { inner: Box<T> };",
        ),
        (
            "index.ts",
            "let outer: Outer<string> = { inner: { value: \"ok\" } };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_type_alias_alias_of_generic_valid() {
    let diagnostics = program(&[
        (
            "box.ts",
            "type Box<T> = { value: T }; type Alias<T> = Box<T>;",
        ),
        ("index.ts", "let box: Alias<string> = { value: \"ok\" };"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_type_alias_generic_reuses_type_parameter_multiple_positions() {
    let diagnostics = program(&[
        ("box.ts", "type Pair<T> = [T, T];"),
        ("index.ts", "let pair: Pair<string> = [\"Ada\", \"Grace\"];"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_type_alias_parameter_order_preserved() {
    let diagnostics = program(&[
        ("box.ts", "type Pair<A, B> = [A, B];"),
        ("index.ts", "let pair: Pair<string, number> = [\"Ada\", 1];"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_type_alias_private_helper_generic_valid() {
    let diagnostics = program(&[
        (
            "box.ts",
            "type Internal<T> = { value: T }; export type Box<T> = Internal<T>;",
        ),
        (
            "index.ts",
            "import { Box } from \"./box\"; let box: Box<string> = { value: \"ok\" };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_type_alias_private_helper_generic_not_importable() {
    let diagnostics = program(&[
        (
            "box.ts",
            "type Internal<T> = { value: T }; export type Box<T> = Internal<T>;",
        ),
        ("index.ts", "import { Internal } from \"./box\";"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_interface_property_valid() {
    let diagnostics = program(&[
        (
            "box.ts",
            "interface StoreApi<TState> { getState: () => TState; }",
        ),
        (
            "index.ts",
            "function getState(): string { return \"ok\"; } let store: StoreApi<string> = { getState: getState };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_interface_property_mismatch() {
    let diagnostics = program(&[
        (
            "box.ts",
            "interface StoreApi<TState> { getState: () => TState; }",
        ),
        (
            "index.ts",
            "function getState(): number { return 123; } let store: StoreApi<string> = { getState: getState };",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn generic_interface_multiple_parameters_valid() {
    let diagnostics = program(&[
        ("box.ts", "interface Pair<A, B> { first: A; second: B; }"),
        (
            "index.ts",
            "let pair: Pair<string, number> = { first: \"Ada\", second: 1 };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_interface_multiple_parameters_mismatch() {
    let diagnostics = program(&[
        ("box.ts", "interface Pair<A, B> { first: A; second: B; }"),
        (
            "index.ts",
            "let pair: Pair<string, number> = { first: 1, second: \"Ada\" };",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn generic_interface_nested_generic_property_valid() {
    let diagnostics = program(&[
        (
            "box.ts",
            "type Box<T> = { value: T }; interface Outer<T> { inner: Box<T>; }",
        ),
        (
            "index.ts",
            "let outer: Outer<string> = { inner: { value: \"ok\" } };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_interface_nested_generic_property_mismatch() {
    let diagnostics = program(&[
        (
            "box.ts",
            "type Box<T> = { value: T }; interface Outer<T> { inner: Box<T>; }",
        ),
        (
            "index.ts",
            "let outer: Outer<string> = { inner: { value: 123 } };",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn generic_interface_uses_generic_type_alias_valid() {
    let diagnostics = program(&[
        (
            "box.ts",
            "type Box<T> = { value: T }; interface UsesBox<T> { inner: Box<T>; }",
        ),
        (
            "index.ts",
            "let value: UsesBox<string> = { inner: { value: \"ok\" } };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_interface_uses_generic_type_alias_mismatch() {
    let diagnostics = program(&[
        (
            "box.ts",
            "type Box<T> = { value: T }; interface UsesBox<T> { inner: Box<T>; }",
        ),
        (
            "index.ts",
            "let value: UsesBox<string> = { inner: { value: 123 } };",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn generic_default_type_argument_valid() {
    let diagnostics = program(&[
        ("box.ts", "type Box<T = string> = { value: T };"),
        ("index.ts", "let box: Box = { value: \"ok\" };"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_default_type_argument_mismatch() {
    let diagnostics = program(&[
        ("box.ts", "type Box<T = string> = { value: T };"),
        ("index.ts", "let box: Box = { value: 123 };"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn generic_default_type_argument_references_previous_parameter_valid() {
    let diagnostics = program(&[
        ("box.ts", "type Pair<A, B = A> = [A, B];"),
        ("index.ts", "let pair: Pair<string> = [\"Ada\", \"Ada\"];"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_default_type_argument_unknown_no_cascade() {
    let diagnostics = program(&[
        ("box.ts", "type Box<T = Missing> = { value: T };"),
        ("index.ts", "let box: Box = { value: \"ok\" };"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
}

#[test]
fn generic_default_type_argument_partial_application_valid() {
    let diagnostics = program(&[
        ("box.ts", "type Pair<A, B = number> = [A, B];"),
        ("index.ts", "let pair: Pair<string> = [\"Ada\", 1];"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_constraint_parsed_not_enforced() {
    let diagnostics = program(&[
        ("box.ts", "type Named<T extends string> = { name: T };"),
        ("index.ts", "let value: Named<number> = { name: 1 };"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_constraint_unknown_type_no_cascade() {
    let diagnostics = program(&[
        ("box.ts", "type Named<T extends Missing> = { name: T };"),
        ("index.ts", "let value: Named<string> = { name: \"ok\" };"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_constraint_default_combination_valid() {
    let diagnostics = program(&[
        (
            "box.ts",
            "type Named<T extends string = \"ok\"> = { name: T };",
        ),
        ("index.ts", "let value: Named = { name: \"ok\" };"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_constraint_does_not_reject_out_of_constraint_yet() {
    let diagnostics = program(&[
        ("box.ts", "type Named<T extends string> = { name: T };"),
        ("index.ts", "let value: Named<number> = { name: 1 };"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_arity_missing_argument_alias() {
    let diagnostics = program(&[
        ("box.ts", "type Box<T> = { value: T };"),
        ("index.ts", "let box: Box = { value: \"ok\" };"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2314"]);
}

#[test]
fn generic_arity_missing_argument_interface() {
    let diagnostics = program(&[
        ("box.ts", "interface Box<T> { value: T; }"),
        ("index.ts", "let box: Box = { value: \"ok\" };"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2314"]);
}

#[test]
fn generic_arity_too_many_arguments_alias() {
    let diagnostics = program(&[
        ("box.ts", "type Box<T> = { value: T };"),
        (
            "index.ts",
            "let box: Box<string, number> = { value: \"ok\" };",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2314"]);
}

#[test]
fn generic_arity_too_many_arguments_interface() {
    let diagnostics = program(&[
        ("box.ts", "interface Box<T> { value: T; }"),
        (
            "index.ts",
            "let box: Box<string, number> = { value: \"ok\" };",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2314"]);
}

#[test]
fn generic_arity_partial_with_default_valid() {
    let diagnostics = program(&[
        ("box.ts", "type Pair<A, B = number> = [A, B];"),
        ("index.ts", "let pair: Pair<string> = [\"Ada\", 1];"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_arity_partial_without_default_error() {
    let diagnostics = program(&[
        ("box.ts", "type Pair<A, B> = [A, B];"),
        ("index.ts", "let pair: Pair<string> = [\"Ada\", 1];"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2314"]);
}

#[test]
fn generic_non_generic_type_with_type_arguments_alias() {
    let diagnostics = program(&[
        ("box.ts", "type Name = string;"),
        ("index.ts", "let value: Name<string> = \"ok\";"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2315"]);
}

#[test]
fn generic_non_generic_type_with_type_arguments_interface() {
    let diagnostics = program(&[
        ("box.ts", "interface Name { value: string; }"),
        ("index.ts", "let value: Name<string> = { value: \"ok\" };"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2315"]);
}

#[test]
fn generic_unknown_type_argument_no_assignment_cascade() {
    let diagnostics = program(&[
        ("box.ts", "type Box<T> = { value: T };"),
        ("index.ts", "let box: Box<Missing> = { value: 123 };"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
}

#[test]
fn generic_unknown_type_argument_no_property_cascade() {
    let diagnostics = program(&[
        ("box.ts", "type Box<T> = { value: T };"),
        (
            "index.ts",
            "let box: Box<Missing> = { value: 123 }; let value = box.value;",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
}

#[test]
fn generic_unknown_type_argument_no_call_argument_cascade() {
    let diagnostics = program(&[
        (
            "box.ts",
            "type Box<T> = { value: T }; function takesString(value: string): void { }",
        ),
        (
            "index.ts",
            "let box: Box<Missing> = { value: 123 }; takesString(box.value);",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
}

#[test]
fn generic_arity_error_no_assignment_cascade() {
    let diagnostics = program(&[
        ("box.ts", "type Box<T> = { value: T };"),
        (
            "index.ts",
            "let box: Box = { value: \"ok\" }; let value = box.value;",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2314"]);
}

#[test]
fn generic_arity_error_no_property_cascade() {
    let diagnostics = program(&[
        ("box.ts", "type Box<T> = { value: T };"),
        (
            "index.ts",
            "let box: Box = { value: \"ok\" }; let value = box.value;",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2314"]);
}

#[test]
fn generic_arity_error_no_call_argument_cascade() {
    let diagnostics = program(&[
        (
            "box.ts",
            "type Box<T> = { value: T }; function takesString(value: string): void { }",
        ),
        (
            "index.ts",
            "let box: Box = { value: \"ok\" }; takesString(box.value);",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2314"]);
}

#[test]
fn generic_default_unknown_no_assignment_cascade() {
    let diagnostics = program(&[
        ("box.ts", "type Box<T = Missing> = { value: T };"),
        ("index.ts", "let box: Box = { value: 123 };"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
}

#[test]
fn generic_constraint_unknown_no_assignment_cascade() {
    let diagnostics = program(&[
        ("box.ts", "type Box<T extends Missing> = { value: T };"),
        ("index.ts", "let box: Box<string> = { value: \"ok\" };"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_module_generic_type_alias_import_valid() {
    let diagnostics = program(&[
        ("box.ts", "export type Box<T> = { value: T };"),
        (
            "index.ts",
            "import { Box } from \"./box\"; let box: Box<string> = { value: \"ok\" };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_module_generic_type_alias_import_mismatch() {
    let diagnostics = program(&[
        ("box.ts", "export type Box<T> = { value: T };"),
        (
            "index.ts",
            "import { Box } from \"./box\"; let box: Box<string> = { value: 123 };",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn generic_module_generic_interface_import_valid() {
    let diagnostics = program(&[
        ("box.ts", "export interface Box<T> { value: T; }"),
        (
            "index.ts",
            "import { Box } from \"./box\"; let box: Box<string> = { value: \"ok\" };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_module_generic_interface_import_mismatch() {
    let diagnostics = program(&[
        ("box.ts", "export interface Box<T> { value: T; }"),
        (
            "index.ts",
            "import { Box } from \"./box\"; let box: Box<string> = { value: 123 };",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn generic_module_generic_alias_export_list_valid() {
    let diagnostics = program(&[
        ("box.ts", "type Box<T> = { value: T }; export { Box };"),
        (
            "index.ts",
            "import { Box } from \"./box\"; let box: Box<string> = { value: \"ok\" };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_module_generic_interface_export_list_valid() {
    let diagnostics = program(&[
        ("box.ts", "interface Box<T> { value: T; } export { Box };"),
        (
            "index.ts",
            "import { Box } from \"./box\"; let box: Box<string> = { value: \"ok\" };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_module_generic_alias_export_alias_valid() {
    let diagnostics = program(&[
        (
            "box.ts",
            "type Box<T> = { value: T }; export { Box as Alias };",
        ),
        (
            "index.ts",
            "import { Alias } from \"./box\"; let box: Alias<string> = { value: \"ok\" };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_module_generic_interface_export_alias_valid() {
    let diagnostics = program(&[
        (
            "box.ts",
            "interface Box<T> { value: T; } export { Box as Alias };",
        ),
        (
            "index.ts",
            "import { Alias } from \"./box\"; let box: Alias<string> = { value: \"ok\" };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_module_type_only_import_valid() {
    let diagnostics = program(&[
        ("box.ts", "export type Box<T> = { value: T };"),
        (
            "index.ts",
            "import type { Box } from \"./box\"; let box: Box<string> = { value: \"ok\" };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_module_type_only_import_value_usage_unresolved() {
    let diagnostics = program(&[
        ("box.ts", "export type Box<T> = { value: T };"),
        (
            "index.ts",
            "import type { Box } from \"./box\"; let value = Box;",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2693"]);
}

#[test]
fn generic_module_private_helper_alias_valid() {
    let diagnostics = program(&[
        (
            "box.ts",
            "type Internal<T> = { value: T }; export type Box<T> = Internal<T>;",
        ),
        (
            "index.ts",
            "import { Box } from \"./box\"; let box: Box<string> = { value: \"ok\" };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_module_private_helper_interface_valid() {
    let diagnostics = program(&[
        (
            "box.ts",
            "type Internal<T> = { value: T }; export interface Box<T> { value: Internal<T>; }",
        ),
        (
            "index.ts",
            "import { Box } from \"./box\"; let box: Box<string> = { value: { value: \"ok\" } };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_module_private_helper_not_importable() {
    let diagnostics = program(&[
        (
            "box.ts",
            "type Internal<T> = { value: T }; export type Box<T> = Internal<T>;",
        ),
        ("index.ts", "import { Internal } from \"./box\";"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_module_default_type_argument_across_module_valid() {
    let diagnostics = program(&[
        ("box.ts", "export type Box<T = string> = { value: T };"),
        (
            "index.ts",
            "import { Box } from \"./box\"; let box: Box = { value: \"ok\" };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_module_arity_error_across_module_no_cascade() {
    let diagnostics = program(&[
        ("box.ts", "export type Box<T> = { value: T };"),
        (
            "index.ts",
            "import { Box } from \"./box\"; let box: Box = { value: \"ok\" };",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2314"]);
}

#[test]
fn generic_module_unknown_type_argument_across_module_no_cascade() {
    let diagnostics = program(&[
        ("box.ts", "export type Box<T> = { value: T };"),
        (
            "index.ts",
            "import { Box } from \"./box\"; let box: Box<Missing> = { value: 123 };",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
}

#[test]
fn generic_duplicate_type_parameter_alias() {
    let diagnostics = native_program(&[("index.ts", "type Pair<T, T> = [T, T];")]);

    assert_eq!(codes(&diagnostics), vec!["surge::duplicate-type-parameter"]);
}

#[test]
fn generic_duplicate_type_parameter_interface() {
    let diagnostics = native_program(&[("index.ts", "interface Pair<T, T> { value: T; }")]);

    assert_eq!(codes(&diagnostics), vec!["surge::duplicate-type-parameter"]);
}

#[test]
fn generic_duplicate_type_parameter_function() {
    let diagnostics = native_program(&[(
        "index.ts",
        "function identity<T, T>(value: T): T { return value; }",
    )]);

    assert_eq!(codes(&diagnostics), vec!["surge::duplicate-type-parameter"]);
}

#[test]
fn generic_duplicate_type_parameter_no_cascade() {
    let diagnostics = native_program(&[("index.ts", "type Pair<T, T> = [T, T];")]);

    assert_eq!(codes(&diagnostics), vec!["surge::duplicate-type-parameter"]);
}

#[test]
fn generic_function_parameter_type_parameter_no_ts2304() {
    let diagnostics = program(&[(
        "index.ts",
        "function identity<T>(value: T): T { return value; }",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_function_return_type_parameter_no_ts2304() {
    let diagnostics = program(&[(
        "index.ts",
        "function identity<T>(value: T): T { return value; }",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_function_multiple_type_parameters_no_ts2304() {
    let diagnostics = program(&[(
        "index.ts",
        "function pair<A, B>(a: A, b: B): A { return a; }",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_function_type_parameter_default_parser_safe() {
    let diagnostics = program(&[(
        "index.ts",
        "function identity<T = string>(value: T): T { return value; }",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_function_type_parameter_constraint_parser_safe() {
    let diagnostics = program(&[(
        "index.ts",
        "function identity<T extends string>(value: T): T { return value; }",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_function_body_return_same_type_parameter_valid_or_unknown_pinned() {
    let diagnostics = program(&[(
        "index.ts",
        "function identity<T>(value: T): T { return value; }",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_function_body_return_mismatch_policy_pinned() {
    let diagnostics = program(&[(
        "index.ts",
        "function identity<T>(value: T): T { return 123; }",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_function_no_implicit_any_still_checks_unannotated_param() {
    let diagnostics = source_with_options(
        "function identity<T>(value): T { return value; }",
        CheckerOptions {
            diagnostic_profile: Default::default(),
            resolved_modules: Default::default(),
            stub_external_modules: false,
            no_implicit_any: true,
            no_implicit_returns: false,
            no_fallthrough_cases_in_switch: false,
            no_implicit_override: false,
            no_property_access_from_index_signature: false,
            no_unused_locals: false,
            no_unused_parameters: false,
            no_lib: false,
            skip_lib_check: false,
            jsx_automatic_runtime: false,
            types: Vec::new(),
        },
    );

    assert_eq!(codes(&diagnostics), vec!["TS7006"]);
}

#[test]
fn generic_function_call_type_args_parser_safe() {
    let diagnostics = program(&[(
        "index.ts",
        "function makeString(): string { return \"ok\"; } let value: string = makeString<string>();",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_function_call_type_args_ignored_policy_pinned() {
    let diagnostics = program(&[(
        "index.ts",
        "function makeString(): string { return \"ok\"; } let value: string = makeString<string>();",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_function_call_type_args_wrong_arity_policy_pinned() {
    let diagnostics = program(&[(
        "index.ts",
        "function makeString(): string { return \"ok\"; } let value: string = makeString<string, number>();",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn generic_function_import_export_parser_safe() {
    let diagnostics = program(&[
        (
            "box.ts",
            "export function identity<T>(value: T): T { return value; }",
        ),
        (
            "index.ts",
            "import { identity } from \"./box\"; let value = identity(\"ok\");",
        ),
    ]);

    assert!(diagnostics.is_empty());
}
