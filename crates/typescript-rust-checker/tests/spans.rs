use typescript_rust_checker::{
    CheckerOptions, SourceFileInput, check_program, check_source_with_options,
};
use typescript_rust_diagnostics::Diagnostic;

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
fn span_ts2304_identifier_expression() {
    let source = "let value = missing;";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2304", span(source, "missing"));
}

#[test]
fn span_ts2304_unknown_type_annotation_points_to_type_name() {
    let source = "let value: Missing = 1;";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2304", span(source, "Missing"));
}

#[test]
fn span_ts2304_unknown_type_alias_target() {
    let source = "type Name = Missing; let value: Name = 1;";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2304", span(source, "Missing"));
}

#[test]
fn span_ts2304_unknown_interface_property_type() {
    let source = "interface User { name: Missing; } let user: User = { name: \"Ada\" };";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2304", span(source, "Missing"));
}

#[test]
fn span_ts2304_unknown_function_parameter_type() {
    let source = "function f(value: Missing): void { }";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2304", span(source, "Missing"));
}

#[test]
fn span_ts2304_unknown_function_return_type() {
    let source = "function f(): Missing { }";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2304", span(source, "Missing"));
}

#[test]
fn span_ts2304_unknown_tuple_element_type() {
    let source = "type Pair = [Missing]; let value: Pair = [1];";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2304", span(source, "Missing"));
}

#[test]
fn span_ts2304_unknown_array_element_type() {
    let source = "type Values = Missing[]; let value: Values = [1];";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2304", span(source, "Missing"));
}

#[test]
fn span_ts7006_points_to_parameter_name() {
    let source = "function f(value): void { }";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: true,
        },
    );
    assert_single_span(source, diagnostics, "TS7006", span(source, "value"));
}

#[test]
fn span_ts7005_points_to_variable_name() {
    let source = "let value;";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: true,
        },
    );
    assert_single_span(source, diagnostics, "TS7005", span(source, "value"));
}

#[test]
fn span_ts2451_points_to_duplicate_variable_name() {
    let source = "let value = 1; let value = 2;";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2451", span_nth(source, "value", 1));
}

#[test]
fn span_ts2393_points_to_duplicate_function_name() {
    let source = "function greet(): void { } function greet(): void { }";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2393", span_nth(source, "greet", 1));
}

#[test]
fn span_ts2300_points_to_duplicate_type_name() {
    let source = "type Name = string; type Name = number;";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2300", span_nth(source, "Name", 1));
}

#[test]
fn span_ts2588_points_to_assignment_target() {
    let source = "const value = 1; value = 2;";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2588", span_nth(source, "value", 1));
}

#[test]
fn span_ts2322_variable_initializer() {
    let source = "let value: number = \"a\";";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2322", span(source, "\"a\""));
}

#[test]
fn span_ts2322_assignment_rhs() {
    let source = "let value: number = 1; value = \"a\";";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2322", span(source, "\"a\""));
}

#[test]
fn span_ts2322_return_expression() {
    let source = "function f(): number { return \"a\"; }";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2322", span(source, "\"a\""));
}

#[test]
fn span_ts2322_object_property_value() {
    let source = "let value: { name: string } = { name: 1 };";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2322", span(source, "1"));
}

#[test]
fn span_ts2322_array_element() {
    let source = "let value: number[] = [\"a\"];";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2322", span(source, "\"a\""));
}

#[test]
fn span_ts2322_tuple_element() {
    let source = "let value: [number, string] = [1, 2];";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2322", span(source, "2"));
}

#[test]
fn span_tuple_length_too_few_points_to_array_literal() {
    let source = "let value: [number, string] = [1];";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2322", span(source, "[1]"));
}

#[test]
fn span_tuple_length_too_many_points_to_extra_element() {
    let source = "let value: [number] = [1, 2];";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2322", span(source, "2"));
}

#[test]
fn span_ts2322_conditional_true_branch() {
    let source = "let value: number = true ? \"a\" : 1;";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2322", span(source, "\"a\""));
}

#[test]
fn span_ts2322_conditional_false_branch() {
    let source = "let value: number = true ? 1 : \"a\";";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2322", span(source, "\"a\""));
}

#[test]
fn span_ts2322_property_call_return_initializer() {
    let source = "let store: { getName: () => string }; let value: number = store.getName();";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(
        source,
        diagnostics,
        "TS2322",
        span(source, "store.getName()"),
    );
}

#[test]
fn span_ts2322_index_access_initializer() {
    let source = "let values: string[] = [\"a\"]; let value: number = values[0];";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2322", span(source, "values[0]"));
}

#[test]
fn span_ts2345_identifier_call_argument() {
    let source = "function greet(value: number): void { } greet(\"a\");";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2345", span(source, "\"a\""));
}

#[test]
fn span_ts2304_call_argument() {
    let source = "function greet(value: string): void { } greet(missing);";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2304", span(source, "missing"));
}

#[test]
fn span_ts2304_call_callee() {
    let source = "missing();";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2304", span(source, "missing"));
}

#[test]
fn span_ts2304_property_call_receiver() {
    let source = "missing.value();";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2304", span(source, "missing"));
}

#[test]
fn span_ts2304_index_receiver() {
    let source = "missing[0];";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2304", span(source, "missing"));
}

#[test]
fn span_ts2304_index_expression() {
    let source = "let values: string[] = [\"a\"]; values[missing];";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2304", span(source, "missing"));
}

#[test]
fn span_ts2345_contextual_object_argument_property_value() {
    let source = "function takesUser(value: { name: string }): void { } takesUser({ name: 1 });";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2322", span(source, "1"));
}

#[test]
fn span_ts2345_contextual_array_argument_element() {
    let source = "function takesValues(value: number[]): void { } takesValues([\"a\"]);";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2322", span(source, "\"a\""));
}

#[test]
fn span_ts2345_contextual_tuple_argument_element() {
    let source = "function takesTuple(value: [number, string]): void { } takesTuple([1, 2]);";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2322", span(source, "2"));
}

#[test]
fn span_ts2554_identifier_call_arity_points_to_callee() {
    let source = "function greet(value: number): void { } greet();";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2554", span(source, "greet()"));
}

#[test]
fn span_ts2554_property_call_arity_points_to_property_or_call() {
    let source = "let store: { getName: (value: number) => void }; store.getName();";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(
        source,
        diagnostics,
        "TS2554",
        span(source, "store.getName()"),
    );
}

#[test]
fn span_ts2349_identifier_non_callable_points_to_callee() {
    let source = "let value = 1; value();";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2349", span_nth(source, "value", 1));
}

#[test]
fn span_ts2349_property_non_callable_points_to_property() {
    let source = "let store: { value: number }; store.value();";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2349", span(source, "store.value()"));
}

#[test]
fn span_ts2339_property_access_missing_points_to_property_name() {
    let source = "let user: { name: string } = { name: \"Ada\" }; user.age;";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2339", span(source, "age"));
}

#[test]
fn span_ts2339_property_call_missing_points_to_property_name() {
    let source = "let user: { name: string } = { name: \"Ada\" }; user.age();";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2339", span(source, "age"));
}

#[test]
fn span_ts2339_primitive_receiver_property_name() {
    let source = "let value = 1; value.foo;";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2339", span(source, "foo"));
}

#[test]
fn span_ts2339_tuple_out_of_range_index() {
    let source = "let tuple: [string] = [\"a\"]; tuple[1];";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2339", span(source, "1"));
}

#[test]
fn span_ts2339_index_non_array_receiver() {
    let source = "let value = 1; value[0];";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2339", span_nth(source, "value", 1));
}

#[test]
fn span_ts2353_excess_property_name() {
    let source = "let user: { name: string } = { name: \"Ada\", age: 1 };";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2353", span(source, "age"));
}

#[test]
fn span_ts2741_missing_required_object_literal() {
    let source = "let user: { name: string; age: number } = {};";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2741", span(source, "{}"));
}

#[test]
fn span_object_literal_unresolved_property_value() {
    let source = "let user: { name: string } = { name: missing };";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2304", span(source, "missing"));
}

#[test]
fn span_ts2362_left_operand() {
    let source = "let value = \"a\" - 1;";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2362", span(source, "\"a\""));
}

#[test]
fn span_ts2363_right_operand() {
    let source = "let value = 1 - \"a\";";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2363", span(source, "\"a\""));
}

#[test]
fn span_ts2365_operator() {
    let source = "let value = \"a\" + true;";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2365", span(source, "\"a\" + true"));
}

#[test]
fn span_ts2367_equality_operator() {
    let source = "let value = \"a\" === 1;";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2367", span(source, "\"a\" === 1"));
}

#[test]
fn span_ts2356_unary_operand() {
    let source = "let value = -\"a\";";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2356", span(source, "\"a\""));
}

#[test]
fn span_ts2872_truthy_literal() {
    let source = "function f(): void { if (\"a\") { } }";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2872", span(source, "\"a\""));
}

#[test]
fn span_ts2873_falsy_literal() {
    let source = "function f(): void { if (0) { } }";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2873", span(source, "0"));
}

#[test]
fn span_ts2304_imported_type_usage() {
    let diagnostics = check_program(vec![
        SourceFileInput {
            file_name: "user.ts".to_string(),
            source_text: "export interface User { name: string; }".to_string(),
        },
        SourceFileInput {
            file_name: "index.ts".to_string(),
            source_text: "import { User } from \"./user\"; let user: User = { name: \"Ada\" };"
                .to_string(),
        },
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn span_import_alias_unresolved_points_to_local_alias_usage() {
    let diagnostics = check_program(vec![
        SourceFileInput {
            file_name: "user.ts".to_string(),
            source_text: "export interface User { name: string; }".to_string(),
        },
        SourceFileInput {
            file_name: "index.ts".to_string(),
            source_text: "import { User as LocalUser } from \"./user\"; let user: LocalUser = { name: \"Ada\" };".to_string(),
        },
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn span_ts2304_imported_value_usage() {
    let diagnostics = check_program(vec![
        SourceFileInput {
            file_name: "user.ts".to_string(),
            source_text: "export const value = 1;".to_string(),
        },
        SourceFileInput {
            file_name: "index.ts".to_string(),
            source_text: "import { value as localValue } from \"./user\"; localValue;".to_string(),
        },
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn span_module_missing_relative_points_to_module_specifier() {
    let diagnostics = check_program(vec![SourceFileInput {
        file_name: "index.ts".to_string(),
        source_text: "import { User } from \"./missing\"; let user: User = { name: \"Ada\" };"
            .to_string(),
    }]);

    assert_eq!(
        diagnostic_tuples(&diagnostics),
        vec![(
            "TS2307".to_string(),
            "index.ts".to_string(),
            Some(span(
                "import { User } from \"./missing\"; let user: User = { name: \"Ada\" };",
                "\"./missing\"",
            )),
        )]
    );
}

#[test]
fn span_module_missing_export_points_to_import_specifier() {
    let diagnostics = check_program(vec![
        SourceFileInput {
            file_name: "user.ts".to_string(),
            source_text: "export interface User { name: string; }".to_string(),
        },
        SourceFileInput {
            file_name: "index.ts".to_string(),
            source_text: "import { Missing } from \"./user\"; let value: Missing = \"x\";"
                .to_string(),
        },
    ]);

    assert_eq!(
        diagnostic_tuples(&diagnostics),
        vec![(
            "TS2305".to_string(),
            "index.ts".to_string(),
            Some(span(
                "import { Missing } from \"./user\"; let value: Missing = \"x\";",
                "Missing",
            )),
        )]
    );
}

#[test]
fn span_module_export_list_missing_local_points_to_export_name() {
    let diagnostics = check_program(vec![SourceFileInput {
        file_name: "index.ts".to_string(),
        source_text: "export { Missing };".to_string(),
    }]);

    assert_eq!(
        diagnostic_tuples(&diagnostics),
        vec![(
            "TS2304".to_string(),
            "index.ts".to_string(),
            Some(span("export { Missing };", "Missing")),
        )]
    );
}

#[test]
fn span_module_side_effect_missing_points_to_module_specifier() {
    let diagnostics = check_program(vec![SourceFileInput {
        file_name: "index.ts".to_string(),
        source_text: "import \"./missing\";".to_string(),
    }]);

    assert_eq!(
        diagnostic_tuples(&diagnostics),
        vec![(
            "TS2307".to_string(),
            "index.ts".to_string(),
            Some(span("import \"./missing\";", "\"./missing\"")),
        )]
    );
}

#[test]
fn span_module_type_only_import_value_usage_points_to_value_usage() {
    let diagnostics = check_program(vec![
        SourceFileInput {
            file_name: "user.ts".to_string(),
            source_text: "export type Name = string;".to_string(),
        },
        SourceFileInput {
            file_name: "index.ts".to_string(),
            source_text: "import type { Name } from \"./user\"; let value = Name;".to_string(),
        },
    ]);

    assert_eq!(
        diagnostic_tuples(&diagnostics),
        vec![(
            "TS2304".to_string(),
            "index.ts".to_string(),
            Some(span_nth(
                "import type { Name } from \"./user\"; let value = Name;",
                "Name",
                1
            )),
        )]
    );
}

#[test]
fn span_module_non_relative_points_to_module_specifier() {
    let source = "import { User } from \"pkg\";";
    let diagnostics = check_program(vec![typescript_rust_checker::SourceFileInput {
        file_name: "example.ts".to_string(),
        source_text: source.to_string(),
    }]);

    assert_single_span(source, diagnostics, "TS2307", span(source, "\"pkg\""));
}

#[test]
fn span_module_unsupported_syntax_points_to_syntax_or_pinned() {
    let source = "import DefaultThing from \"./thing\";";
    let diagnostics = check_program(vec![typescript_rust_checker::SourceFileInput {
        file_name: "example.ts".to_string(),
        source_text: source.to_string(),
    }]);

    assert_single_span(
        source,
        diagnostics,
        "typescript-rust::unsupported-module-syntax",
        span(source, source),
    );
}

#[test]
fn span_module_value_export_used_as_type_points_to_type_usage() {
    let diagnostics = check_program(vec![
        SourceFileInput {
            file_name: "user.ts".to_string(),
            source_text: "export const User: string = \"Ada\";".to_string(),
        },
        SourceFileInput {
            file_name: "index.ts".to_string(),
            source_text: "import { User } from \"./user\"; let value: User = \"Ada\";".to_string(),
        },
    ]);

    assert_eq!(
        diagnostic_tuples(&diagnostics),
        vec![(
            "TS2304".to_string(),
            "index.ts".to_string(),
            Some(span_nth(
                "import { User } from \"./user\"; let value: User = \"Ada\";",
                "User",
                1
            )),
        )]
    );
}

#[test]
fn span_module_imported_type_mismatch_points_to_consumer_initializer() {
    let diagnostics = check_program(vec![
        SourceFileInput {
            file_name: "user.ts".to_string(),
            source_text: "export interface User { name: string; }".to_string(),
        },
        SourceFileInput {
            file_name: "index.ts".to_string(),
            source_text: "import { User } from \"./user\"; let user: User = { name: 123 };"
                .to_string(),
        },
    ]);

    assert_eq!(
        diagnostic_tuples(&diagnostics),
        vec![(
            "TS2322".to_string(),
            "index.ts".to_string(),
            Some(span(
                "import { User } from \"./user\"; let user: User = { name: 123 };",
                "123",
            )),
        )]
    );
}

#[test]
fn span_module_exported_unknown_type_points_to_exporter_type_name() {
    let diagnostics = check_program(vec![SourceFileInput {
        file_name: "user.ts".to_string(),
        source_text: "export type Name = Missing; let value: Name = 1;".to_string(),
    }]);

    assert_eq!(
        diagnostic_tuples(&diagnostics),
        vec![(
            "TS2304".to_string(),
            "user.ts".to_string(),
            Some(span(
                "export type Name = Missing; let value: Name = 1;",
                "Missing"
            )),
        )]
    );
}

#[test]
fn span_module_import_alias_usage_mismatch_points_to_consumer_expression() {
    let diagnostics = check_program(vec![
        SourceFileInput {
            file_name: "user.ts".to_string(),
            source_text: "export interface User { name: string; }".to_string(),
        },
        SourceFileInput {
            file_name: "index.ts".to_string(),
            source_text:
                "import { User as LocalUser } from \"./user\"; let user: LocalUser = { name: 123 };"
                    .to_string(),
        },
    ]);

    assert_eq!(
        diagnostic_tuples(&diagnostics),
        vec![(
            "TS2322".to_string(),
            "index.ts".to_string(),
            Some(span(
                "import { User as LocalUser } from \"./user\"; let user: LocalUser = { name: 123 };",
                "123",
            )),
        )]
    );
}

#[test]
fn span_module_exported_unknown_type_points_to_type_name() {
    let source = "export type Name = Missing; let value: Name = 1;";
    let diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            no_implicit_any: false,
        },
    );
    assert_single_span(source, diagnostics, "TS2304", span(source, "Missing"));
}

#[test]
fn span_cross_file_consumer_error_points_to_consumer_file_expression() {
    let diagnostics = check_program(vec![
        SourceFileInput {
            file_name: "a.ts".to_string(),
            source_text: "type Name = string;".to_string(),
        },
        SourceFileInput {
            file_name: "b.ts".to_string(),
            source_text: "let value: Name = 1;".to_string(),
        },
    ]);

    assert_eq!(
        diagnostic_tuples(&diagnostics),
        vec![(
            "TS2322".to_string(),
            "b.ts".to_string(),
            Some(span("let value: Name = 1;", "1")),
        )]
    );
}

#[test]
fn span_cross_file_declaration_error_points_to_declaration_file_token() {
    let diagnostics = check_program(vec![
        SourceFileInput {
            file_name: "a.ts".to_string(),
            source_text: "type Name = string;".to_string(),
        },
        SourceFileInput {
            file_name: "b.ts".to_string(),
            source_text: "type Name = number;".to_string(),
        },
    ]);

    assert_eq!(
        diagnostic_tuples(&diagnostics),
        vec![(
            "TS2300".to_string(),
            "b.ts".to_string(),
            Some(span("type Name = number;", "Name")),
        )]
    );
}
