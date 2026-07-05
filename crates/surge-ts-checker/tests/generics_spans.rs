use surge_ts_checker::{
    CheckerOptions, DiagnosticProfile, SourceFileInput, check_program, check_program_with_options,
    check_source_with_options,
};
use surge_ts_diagnostics::Diagnostic;

fn diagnostic_tuples(diagnostics: &[Diagnostic]) -> Vec<(String, String, Option<(usize, usize)>)> {
    diagnostics
        .iter()
        .map(|diagnostic| {
            (
                diagnostic.code.to_string(),
                diagnostic.file_name.clone(),
                diagnostic.span.map(|span| (span.start, span.end)),
            )
        })
        .collect()
}

fn span(source: &str, needle: &str) -> (usize, usize) {
    let start = source
        .find(needle)
        .unwrap_or_else(|| panic!("missing needle {needle:?} in {source:?}"));
    (start, start + needle.len())
}

fn span_nth(source: &str, needle: &str, nth: usize) -> (usize, usize) {
    let start = source
        .match_indices(needle)
        .nth(nth)
        .map(|(start, _)| start)
        .unwrap_or_else(|| panic!("missing {nth} occurrence of {needle:?} in {source:?}"));
    (start, start + needle.len())
}

fn assert_single_span(
    source: &str,
    diagnostics: Vec<Diagnostic>,
    code: &str,
    expected_span: (usize, usize),
) {
    assert_eq!(
        diagnostic_tuples(&diagnostics),
        vec![(
            code.to_string(),
            "example.ts".to_string(),
            Some(expected_span)
        )],
        "source: {source}"
    );
}

#[test]
fn span_generic_arity_missing_points_to_type_reference_name() {
    let source = "type Box<T> = { value: T }; let box: Box = { value: \"ok\" };";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            diagnostic_profile: DiagnosticProfile::Native,
            resolved_modules: Default::default(),
            stub_external_modules: false,
            no_implicit_any: false,
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

    assert_eq!(
        diagnostic_tuples(&diagnostics),
        vec![(
            "TS2314".to_string(),
            "example.ts".to_string(),
            Some(span_nth(source, "Box", 1)),
        )]
    );
}

#[test]
fn span_generic_unknown_type_argument_points_to_type_argument() {
    let source = "type Box<T> = { value: T }; let box: Box<Missing> = { value: 123 };";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            diagnostic_profile: Default::default(),
            resolved_modules: Default::default(),
            stub_external_modules: false,
            no_implicit_any: false,
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

    assert_eq!(
        diagnostic_tuples(&diagnostics),
        vec![(
            "TS2304".to_string(),
            "example.ts".to_string(),
            Some(span(source, "Missing")),
        )]
    );
}

#[test]
fn span_generic_type_alias_mismatch_points_to_initializer_value() {
    let diagnostics = check_program(vec![
        SourceFileInput {
            file_name: "box.ts".to_string(),
            source_text: "export type Box<T> = { value: T };".to_string(),
        },
        SourceFileInput {
            file_name: "index.ts".to_string(),
            source_text: "import { Box } from \"./box\"; let box: Box<string> = { value: 123 };"
                .to_string(),
        },
    ]);

    assert_eq!(
        diagnostic_tuples(&diagnostics),
        vec![(
            "TS2322".to_string(),
            "index.ts".to_string(),
            Some(span(
                "import { Box } from \"./box\"; let box: Box<string> = { value: 123 };",
                "value",
            )),
        )]
    );
}

#[test]
fn span_generic_module_import_mismatch_points_to_consumer_initializer() {
    let diagnostics = check_program(vec![
        SourceFileInput {
            file_name: "box.ts".to_string(),
            source_text: "export interface Box<T> { value: T; }".to_string(),
        },
        SourceFileInput {
            file_name: "index.ts".to_string(),
            source_text: "import { Box } from \"./box\"; let box: Box<string> = { value: 123 };"
                .to_string(),
        },
    ]);

    assert_eq!(
        diagnostic_tuples(&diagnostics),
        vec![(
            "TS2322".to_string(),
            "index.ts".to_string(),
            Some(span(
                "import { Box } from \"./box\"; let box: Box<string> = { value: 123 };",
                "value",
            )),
        )]
    );
}

#[test]
fn span_generic_arity_too_many_points_to_type_reference_name() {
    let source = "type Box<T> = { value: T }; let box: Box<string, number> = { value: \"ok\" };";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            diagnostic_profile: Default::default(),
            resolved_modules: Default::default(),
            stub_external_modules: false,
            no_implicit_any: false,
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

    assert_single_span(source, diagnostics, "TS2314", span_nth(source, "Box", 1));
}

#[test]
fn span_generic_non_generic_type_args_points_to_type_reference_name() {
    let source = "type Name = string; let value: Name<string> = \"ok\";";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            diagnostic_profile: Default::default(),
            resolved_modules: Default::default(),
            stub_external_modules: false,
            no_implicit_any: false,
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

    assert_single_span(source, diagnostics, "TS2315", span_nth(source, "Name", 1));
}

#[test]
fn span_invalid_pick_alias_points_to_pick_reference_and_dedupes_usage() {
    let source = "interface User { id: number; name: string; active: boolean; } type InvalidPick = Pick<User, \"id\" | \"missing\">; let invalidPickUsage: InvalidPick;";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            diagnostic_profile: Default::default(),
            resolved_modules: Default::default(),
            stub_external_modules: false,
            no_implicit_any: false,
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

    assert_single_span(source, diagnostics, "TS2344", span_nth(source, "Pick", 1));
}

#[test]
fn span_generic_default_unknown_points_to_default_type_name() {
    let source = "type Box<T = Missing> = { value: T }; let box: Box = { value: 123 };";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            diagnostic_profile: Default::default(),
            resolved_modules: Default::default(),
            stub_external_modules: false,
            no_implicit_any: false,
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

    assert_single_span(source, diagnostics, "TS2304", span(source, "Missing"));
}

#[test]
fn span_generic_constraint_unknown_points_to_constraint_type_name() {
    let source =
        "type Box<T extends Missing> = { value: T }; let box: Box<string> = { value: \"ok\" };";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            diagnostic_profile: Default::default(),
            resolved_modules: Default::default(),
            stub_external_modules: false,
            no_implicit_any: false,
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

    assert!(diagnostics.is_empty());
}

#[test]
fn span_generic_duplicate_type_parameter_points_to_duplicate_name() {
    let source = "type Pair<T, T> = [T, T];";
    let diagnostics = check_program_with_options(
        vec![SourceFileInput {
            file_name: "example.ts".to_string(),
            source_text: source.to_string(),
        }],
        CheckerOptions {
            diagnostic_profile: DiagnosticProfile::Native,
            resolved_modules: Default::default(),
            stub_external_modules: false,
            no_implicit_any: false,
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

    assert_single_span(
        source,
        diagnostics,
        "surge::duplicate-type-parameter",
        span_nth(source, "T", 1),
    );
}

#[test]
fn span_generic_function_type_parameter_no_unresolved_span() {
    let source = "function identity<T>(value: T): T { return value; }";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            diagnostic_profile: Default::default(),
            resolved_modules: Default::default(),
            stub_external_modules: false,
            no_implicit_any: false,
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

    assert!(diagnostics.is_empty());
}
