use surge_ts_checker::{
    CheckerOptions, DiagnosticProfile, SourceFileInput, check_program, check_program_with_options,
    check_source, check_source_with_options,
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

fn program_with_options(
    files: &[(&str, &str)],
    options: CheckerOptions,
) -> Vec<surge_ts_diagnostics::Diagnostic> {
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

fn native_program(files: &[(&str, &str)]) -> Vec<surge_ts_diagnostics::Diagnostic> {
    let mut options = CheckerOptions::default();
    options.diagnostic_profile = DiagnosticProfile::Native;
    program_with_options(files, options)
}

fn dependency_program(
    declaration_file: &str,
    declaration_source: &str,
    consumer_source: &str,
) -> Vec<surge_ts_diagnostics::Diagnostic> {
    let mut options = CheckerOptions::default();
    options
        .resolved_modules
        .insert("dep".to_string(), declaration_file.to_string());
    program_with_options(
        &[
            (declaration_file, declaration_source),
            ("src/index.ts", consumer_source),
        ],
        options,
    )
}

#[test]
fn dependency_dts_export_generic_alias_lazy() {
    let diagnostics = dependency_program(
        "node_modules/dep/index.d.ts",
        "type Box<T> = { value: T }; declare const item: Box<string>; export { item };",
        "import { item } from 'dep'; const value: string = item.value;",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn dependency_dts_export_interface_lazy() {
    let diagnostics = dependency_program(
        "node_modules/dep/index.d.ts",
        "interface Item { value: string } declare const item: Item; export { item };",
        "import { item } from 'dep'; const value: string = item.value;",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn dependency_dts_export_conditional_lazy() {
    let diagnostics = dependency_program(
        "node_modules/dep/index.d.ts",
        "type Select<T> = T extends string ? { text: T } : { count: number }; declare const item: Select<string>; export { item };",
        "import { item } from 'dep'; const value: string = item.text;",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn dependency_dts_export_mapped_lazy() {
    let diagnostics = dependency_program(
        "node_modules/dep/index.d.ts",
        "type Copy<T> = { [K in keyof T]: T[K] }; declare const item: Copy<{ value: string }>; export { item };",
        "import { item } from 'dep'; const value: string = item.value;",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn dependency_dts_export_indexed_access_lazy() {
    let diagnostics = dependency_program(
        "node_modules/dep/index.d.ts",
        "interface Item { value: string } declare const item: Item['value']; export { item };",
        "import { item } from 'dep'; const value: string = item;",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn dependency_dts_export_reference_intersection_lazy() {
    let diagnostics = dependency_program(
        "node_modules/dep/index.d.ts",
        "interface Left { left: string } interface Right { right: number } declare const item: Left & Right; export { item };",
        "import { item } from 'dep'; const left: string = item.left; const right: number = item.right;",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn dependency_dts_export_typeof_imported_value_lazy() {
    let mut options = CheckerOptions::default();
    options
        .resolved_modules
        .insert("dep".to_string(), "node_modules/dep/index.d.ts".to_string());
    let diagnostics = program_with_options(
        &[
            (
                "node_modules/dep/primitive.d.ts",
                "declare const primitive: { value: string }; export { primitive };",
            ),
            (
                "node_modules/dep/index.d.ts",
                "import { primitive } from './primitive'; declare const item: typeof primitive; export { item };",
            ),
            (
                "src/index.ts",
                "import { item } from 'dep'; const value: string = item.value;",
            ),
        ],
        options,
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn dependency_dts_reexport_chain_lazy() {
    let mut options = CheckerOptions::default();
    options
        .resolved_modules
        .insert("dep".to_string(), "node_modules/dep/index.d.ts".to_string());
    let diagnostics = program_with_options(
        &[
            (
                "node_modules/dep/base.d.ts",
                "interface Item { value: string } declare const item: Item; export { item };",
            ),
            (
                "node_modules/dep/middle.d.ts",
                "export { item } from './base';",
            ),
            (
                "node_modules/dep/index.d.ts",
                "export { item } from './middle';",
            ),
            (
                "src/index.ts",
                "import { item } from 'dep'; const value: string = item.value;",
            ),
        ],
        options,
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn dependency_dts_export_star_lazy() {
    let mut options = CheckerOptions::default();
    options
        .resolved_modules
        .insert("dep".to_string(), "node_modules/dep/index.d.ts".to_string());
    let diagnostics = program_with_options(
        &[
            (
                "node_modules/dep/base.d.ts",
                "interface Item { value: string } declare const item: Item; export { item };",
            ),
            ("node_modules/dep/index.d.ts", "export * from './base';"),
            (
                "src/index.ts",
                "import { item } from 'dep'; const value: string = item.value;",
            ),
        ],
        options,
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn dependency_dts_degraded_resolution_not_cached() {
    let diagnostics = dependency_program(
        "node_modules/dep/index.d.ts",
        "type Broken<T> = Missing<T>; declare const first: Broken<string>; declare const second: Broken<number>; export { first, second };",
        "import { first, second } from 'dep'; first.anything; second.anything;",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
}

#[test]
fn dependency_dts_property_access_forces_once() {
    let diagnostics = dependency_program(
        "node_modules/dep/index.d.ts",
        "type Item = { value: string }; declare const item: Item; export { item };",
        "import { item } from 'dep'; const a: string = item.value; const b: string = item.value;",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn dependency_dts_assignability_forces_once() {
    let diagnostics = dependency_program(
        "node_modules/dep/index.d.ts",
        "type Item = { value: string }; declare const item: Item; export { item };",
        "import { item } from 'dep'; const a: { value: string } = item; const b: { value: string } = item;",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn dependency_dts_display_does_not_force() {
    let diagnostics = dependency_program(
        "node_modules/dep/index.d.ts",
        "type Box<T> = { value: T }; declare const item: Box<string>; export { item };",
        "import { item } from 'dep'; void item;",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn dependency_dts_module_dedup_does_not_force() {
    let diagnostics = dependency_program(
        "node_modules/dep/index.d.ts",
        "type Large<T> = { a: T; b: T; c: T; d: T; e: T; f: T }; declare const first: Large<string>; declare const second: Large<string>; export { first, second };",
        "import { first, second } from 'dep'; void first; void second;",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn d_mts_lazy_surface() {
    let diagnostics = dependency_program(
        "node_modules/dep/index.d.mts",
        "type Item = { value: string }; declare const item: Item; export { item };",
        "import { item } from 'dep'; const value: string = item.value;",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn d_cts_lazy_surface() {
    let diagnostics = dependency_program(
        "node_modules/dep/index.d.cts",
        "type Item = { value: string }; declare const item: Item; export { item };",
        "import { item } from 'dep'; const value: string = item.value;",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn dependency_dts_same_type_many_importers() {
    let mut files = vec![SourceFileInput {
        file_name: "node_modules/dep/base.d.ts".to_string(),
        source_text:
            "export interface Shared { value: string; a: number; b: number; c: number; d: number }"
                .to_string(),
    }];
    let mut consumer = String::new();
    let mut options = CheckerOptions::default();
    for index in 0..100 {
        let file_name = format!("node_modules/dep/module-{index}.d.ts");
        files.push(SourceFileInput {
            file_name: file_name.clone(),
            source_text: format!(
                "import {{ Shared }} from './base'; declare const value{index}: Shared; export {{ value{index} }};"
            ),
        });
        let specifier = format!("dep/module-{index}");
        options
            .resolved_modules
            .insert(specifier.clone(), file_name);
        consumer.push_str(&format!(
            "import {{ value{index} }} from '{specifier}'; const result{index}: string = value{index}.value;\n"
        ));
    }
    files.push(SourceFileInput {
        file_name: "src/index.ts".to_string(),
        source_text: consumer,
    });

    let diagnostics = check_program_with_options(files, options);
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn dependency_dts_function_parameter_materializes_for_call() {
    let diagnostics = dependency_program(
        "node_modules/dep/index.d.ts",
        "export interface Input { value: string } export declare function consume(input: Input): void;",
        "import { consume } from 'dep'; consume({ value: 1 });",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn dependency_dts_function_return_materializes_for_inspection() {
    let diagnostics = dependency_program(
        "node_modules/dep/index.d.ts",
        "export interface Output { value: string } export declare function create(): Output;",
        "import { create } from 'dep'; const value: string = create().value;",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn dependency_dts_generic_function_keeps_call_site_substitution() {
    let diagnostics = dependency_program(
        "node_modules/dep/index.d.ts",
        "export declare function identity<T extends { value: string } = { value: string }>(input: T): T;",
        "import { identity } from 'dep'; const value: string = identity({ value: 'ok' }).value;",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn dependency_dts_structural_generic_signature_keeps_call_site_substitution() {
    let diagnostics = dependency_program(
        "node_modules/dep/index.d.ts",
        "export declare function transform<T extends string>(input: { value: T }): { output: T };",
        "import { transform } from 'dep'; const output: 'ok' = transform({ value: 'ok' }).output; transform({ value: 1 });",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn d_mts_function_signature_materializes_for_call() {
    let diagnostics = dependency_program(
        "node_modules/dep/index.d.mts",
        "export declare function consume(input: { value: string }): void;",
        "import { consume } from 'dep'; consume({ value: 1 });",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn d_cts_function_signature_materializes_for_call() {
    let diagnostics = dependency_program(
        "node_modules/dep/index.d.cts",
        "export declare function consume(input: { value: string }): void;",
        "import { consume } from 'dep'; consume({ value: 1 });",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn dependency_dts_function_reexport_keeps_lazy_signature() {
    let mut options = CheckerOptions::default();
    options
        .resolved_modules
        .insert("dep".to_string(), "node_modules/dep/index.d.ts".to_string());
    let diagnostics = program_with_options(
        &[
            (
                "node_modules/dep/base.d.ts",
                "export interface Output { value: string } export declare function create(): Output;",
            ),
            (
                "node_modules/dep/index.d.ts",
                "export { create } from './base';",
            ),
            (
                "src/index.ts",
                "import { create } from 'dep'; const value: string = create().value;",
            ),
        ],
        options,
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn dependency_dts_function_signature_display_does_not_change_semantics() {
    let diagnostics = dependency_program(
        "node_modules/dep/index.d.ts",
        "export type Large<T> = { a: T; b: T; c: T; d: T }; export declare function create(input: Large<string>): Large<string>;",
        "import { create } from 'dep'; void create;",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
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
    let diagnostics = native_program(&[("a.ts", "let value: string | = \"ok\";")]);

    assert!(!diagnostics.is_empty());
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_api_generated_default_libs_visible() {
    let diagnostics = program(&[(
        "example.ts",
        "const transport: AuthenticatorTransport = \"usb\"; const n = Math.max(1, 2);",
    )]);

    assert!(diagnostics.is_empty());
}

#[test]
fn program_api_generated_default_lib_array_global_from_dts() {
    // The generated `.d.ts` fallback (parsed, not a Rust snapshot table) must
    // provide the named `Array`/`ReadonlyArray` globals and their `.length`/
    // `.map`/`.find` members, mirroring the physical-lib path.
    let diagnostics = program(&[(
        "example.ts",
        "const values: Array<number> = [1, 2, 3];\n\
         const readonlyValues: ReadonlyArray<number> = values;\n\
         const size: number = values.length;\n\
         const doubled: number[] = values.map((value) => value * 2);\n\
         const found: number | undefined = values.find((value) => value > 1);\n\
         void readonlyValues; void size; void doubled; void found;",
    )]);

    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn program_unknown_property_access_reports_ts18046() {
    let diagnostics = program(&[("example.ts", "const json: unknown = {}; json.result;")]);

    assert_eq!(codes(&diagnostics), vec!["TS18046"]);
    assert!(
        diagnostics[0]
            .message
            .contains("'json' is of type 'unknown'.")
    );
}

#[test]
fn program_node_fetch_json_result_reports_ts18046() {
    let mut options = CheckerOptions::default();
    options.resolved_modules.insert(
        "node-fetch".to_string(),
        "node_modules/node-fetch/@types/index.d.ts".to_string(),
    );

    let diagnostics = program_with_options(
        &[
            (
                "globals.d.ts",
                "interface Response { json(): Promise<any>; }",
            ),
            (
                "node_modules/node-fetch/@types/index.d.ts",
                "export type HeadersInit = Record<string, string>;\n\
                 export type BodyInit = string;\n\
                 export interface RequestInit { body?: BodyInit; headers?: HeadersInit; method?: string; }\n\
                 export type RequestInfo = string | Request;\n\
                 declare class BodyMixin {\n\
                   constructor(body?: BodyInit, options?: { size?: number });\n\
                   readonly body: NodeJS.ReadableStream | null;\n\
                   json(): Promise<unknown>;\n\
                 }\n\
                 export class Request extends BodyMixin {}\n\
                 export class Response extends BodyMixin {}\n\
                 export default function fetch(url: URL | RequestInfo, init?: RequestInit): Promise<Response>;",
            ),
            (
                "src/index.ts",
                "import fetch from 'node-fetch';\n\
                 fetch('https://mds3.fido.tools/getEndpoints', {\n\
                   method: 'POST',\n\
                   body: JSON.stringify({ endpoint: 'https://example.com' }),\n\
                   headers: { 'Content-Type': 'application/json' },\n\
                 })\n\
                   .then((resp) => resp.json())\n\
                   .then((json) => { const mdsServers: string[] = json.result; });",
            ),
        ],
        options,
    );

    assert_eq!(codes(&diagnostics), vec!["TS18046"], "{diagnostics:#?}");
}

#[test]
fn program_api_no_lib_hides_generated_default_libs() {
    let diagnostics = program_with_options(
        &[(
            "example.ts",
            "const transport: AuthenticatorTransport = \"usb\"; const n = Math.max(1, 2);",
        )],
        CheckerOptions {
            diagnostic_profile: Default::default(),
            resolved_modules: Default::default(),
            resolved_modules_by_importer: Default::default(),
            stub_external_modules: false,
            no_implicit_any: false,
            no_implicit_returns: false,
            no_fallthrough_cases_in_switch: false,
            no_implicit_override: false,
            no_property_access_from_index_signature: false,
            no_unused_locals: false,
            no_unused_parameters: false,
            no_lib: true,
            skip_lib_check: false,
            jsx_automatic_runtime: false,
            types: Vec::new(),
        },
    );

    assert_eq!(codes(&diagnostics), vec!["TS2304", "TS2304"]);
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
fn program_merge_interface_across_files_conflict_ts2717() {
    // Global interfaces with the same name merge across files; a conflicting
    // property type is reported once as TS2717 on the later declaration.
    let diagnostics = program(&[
        ("a.ts", "interface User { name: string; }"),
        ("b.ts", "interface User { name: number; }"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2717"]);
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

    assert_eq!(codes(&diagnostics), vec!["TS2393", "TS2393"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts", "a.ts"]);
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
        ("a.ts", "let greeting = \"Ada\";"),
        ("b.ts", "let value: string = greeting;"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_top_level_const_not_shared_or_policy_pinned() {
    let diagnostics = program(&[
        ("a.ts", "const greeting = \"Ada\";"),
        ("b.ts", "let value: string = greeting;"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_file_local_variable_does_not_leak() {
    let diagnostics = program(&[
        ("a.ts", "let greeting = \"Ada\";"),
        ("b.ts", "function f(): string { return greeting; }"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert_eq!(file_names(&diagnostics), vec!["b.ts"]);
}

#[test]
fn program_type_only_value_usage_reports_ts2693_no_cascade() {
    let diagnostics = program(&[
        ("a.ts", "type Name = string;"),
        (
            "b.ts",
            "let value: string; value = Name; let other: string = value;",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2693"]);
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
fn single_file_builtins_visible() {
    let source = r#"
        console.log("ok");
        const a: Array<string> = ["a"];
        const n: number = Math.max(1, 2);
    "#;

    let options = CheckerOptions {
        diagnostic_profile: Default::default(),
        no_lib: false,
        skip_lib_check: false,
        jsx_automatic_runtime: false,
        types: Vec::new(),
        ..Default::default()
    };

    let diagnostics = check_source_with_options(source, "test.ts", options);
    assert_eq!(
        diagnostics.len(),
        0,
        "Expected 0 diagnostics, got: {:#?}",
        diagnostics
    );
}

#[test]
fn single_file_no_lib_hides_builtins() {
    let source = r#"
        console.log("ok");
        const a: Array<string> = ["a"];
        const n: number = Math.max(1, 2);
    "#;

    let options = CheckerOptions {
        diagnostic_profile: Default::default(),
        no_lib: true,
        skip_lib_check: false,
        jsx_automatic_runtime: false,
        types: Vec::new(),
        ..Default::default()
    };

    let diagnostics = check_source_with_options(source, "test.ts", options);
    assert_eq!(codes(&diagnostics), vec!["TS2304", "TS2304", "TS2304"]);
}

#[test]
fn program_api_single_file_no_implicit_any_matches_check_source_with_options() {
    let source = "function f(value): string { return \"ok\"; }";
    let program_diagnostics = program_with_options(
        &[("example.ts", source)],
        CheckerOptions {
            diagnostic_profile: Default::default(),
            resolved_modules: Default::default(),
            resolved_modules_by_importer: Default::default(),
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
    let single_file_diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            diagnostic_profile: Default::default(),
            resolved_modules: Default::default(),
            resolved_modules_by_importer: Default::default(),
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

    assert_eq!(codes(&program_diagnostics), codes(&single_file_diagnostics));
    assert_eq!(
        file_names(&program_diagnostics),
        file_names(&single_file_diagnostics)
    );
}

fn no_implicit_returns_options(no_implicit_returns: bool) -> CheckerOptions {
    CheckerOptions {
        no_implicit_returns,
        ..Default::default()
    }
}

#[test]
fn no_implicit_returns_reports_ts7030_on_partial_value_return() {
    let source = "export function a(x: number) { if (x > 0) return 1; }";
    let diagnostics = check_source_with_options(source, "a.ts", no_implicit_returns_options(true));
    assert_eq!(codes(&diagnostics), vec!["TS7030"]);
}

#[test]
fn no_implicit_returns_reports_ts7030_on_arrow_block_body() {
    let source = "export const f = (x: number) => { if (x > 0) return 1; };";
    let diagnostics = check_source_with_options(source, "a.ts", no_implicit_returns_options(true));
    assert_eq!(codes(&diagnostics), vec!["TS7030"]);
}

#[test]
fn no_implicit_returns_silent_when_flag_off() {
    let source = "export function a(x: number) { if (x > 0) return 1; }";
    let diagnostics = check_source_with_options(source, "a.ts", no_implicit_returns_options(false));
    assert!(
        codes(&diagnostics).is_empty(),
        "got {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_implicit_returns_silent_when_all_paths_return() {
    let source = "export function b(x: number) { if (x > 0) return 1; return 2; }";
    let diagnostics = check_source_with_options(source, "b.ts", no_implicit_returns_options(true));
    assert!(
        codes(&diagnostics).is_empty(),
        "got {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_implicit_returns_silent_when_all_paths_exit_without_value() {
    // Every path exits explicitly (`return 1` / bare `return;`), so there is no
    // implicit fall-through — tsc emits nothing here even under noImplicitReturns.
    let source = "export function c(x: number) { if (x > 0) return 1; return; }";
    let diagnostics = check_source_with_options(source, "c.ts", no_implicit_returns_options(true));
    assert!(
        codes(&diagnostics).is_empty(),
        "got {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_implicit_returns_silent_on_infinite_loop() {
    // `while (true)` with no `break` never falls through, so the end is
    // unreachable and tsc emits no TS7030. (surge's separate always-truthy note
    // for `while (true)` is out of scope here, so assert only TS7030's absence.)
    let source = "export function g(x: number) { while (true) { if (x > 0) return 1; } }";
    let diagnostics = check_source_with_options(source, "g.ts", no_implicit_returns_options(true));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS7030"),
        "unexpected TS7030: {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_implicit_returns_silent_on_throw_only_body() {
    // A function that only throws (no `return <value>`) infers a `void` return
    // type; tsc skips TS7030. A `throw` must not count as a value return.
    let source = "export function p(x: number) { if (x > 0) { throw x; } }";
    let diagnostics = check_source_with_options(source, "p.ts", no_implicit_returns_options(true));
    assert_eq!(codes(&diagnostics), Vec::<String>::new());
}

#[test]
fn no_implicit_returns_silent_on_exhaustive_switch_with_default() {
    // Every clause returns and a `default` makes the switch exhaustive, so no
    // path falls through — tsc emits no TS7030.
    let source =
        "export function s(x: number) { switch (x) { case 1: return 'a'; default: return 'b'; } }";
    let diagnostics = check_source_with_options(source, "s.ts", no_implicit_returns_options(true));
    assert_eq!(codes(&diagnostics), Vec::<String>::new());
}

#[test]
fn no_implicit_returns_reports_switch_without_default() {
    // No `default`: the discriminant may match no clause and fall through, so
    // tsc reports TS7030.
    let source =
        "export function s(x: number) { switch (x) { case 1: return 'a'; case 2: return 'b'; } }";
    let diagnostics = check_source_with_options(source, "s.ts", no_implicit_returns_options(true));
    assert_eq!(codes(&diagnostics), vec!["TS7030"]);
}

#[test]
fn no_implicit_returns_does_not_fire_on_constructor() {
    // tsc never applies noImplicitReturns to constructors (they implicitly
    // return `this`). A constructor with a conditional `return` value must not
    // produce TS7030.
    let source =
        "export class C { constructor(x: number) { if (x > 0) { return; } this.y = x; } y = 0; }";
    let diagnostics = check_source_with_options(source, "c.ts", no_implicit_returns_options(true));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS7030"),
        "unexpected TS7030: {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_implicit_returns_silent_on_try_return_catch_throw() {
    // Every path exits: the `try` returns, the `catch` throws. The construct
    // never falls through, so tsc emits no TS7030 (regression: surge's Try flow
    // summary used to hardcode `guarantees_exit = false`).
    let source = "export function h(x: number) { try { return x; } catch (e) { throw e; } }";
    let diagnostics = check_source_with_options(source, "h.ts", no_implicit_returns_options(true));
    assert_eq!(codes(&diagnostics), Vec::<String>::new());
}

#[test]
fn no_implicit_returns_does_not_affect_annotated_missing_return() {
    // Annotated return type still routes through TS2366, independent of the flag.
    let source = "export function e(x: number): number { if (x > 0) return 1; }";
    let diagnostics = check_source_with_options(source, "e.ts", no_implicit_returns_options(true));
    assert_eq!(codes(&diagnostics), vec!["TS2366"]);
}

fn no_fallthrough_options(no_fallthrough_cases_in_switch: bool) -> CheckerOptions {
    CheckerOptions {
        no_fallthrough_cases_in_switch,
        ..Default::default()
    }
}

#[test]
fn no_fallthrough_reports_ts7029_on_reachable_clause_end() {
    let source = "export function a(x: number) { switch (x) { case 1: x; case 2: return; default: return; } }";
    let diagnostics = check_source_with_options(source, "a.ts", no_fallthrough_options(true));
    assert_eq!(codes(&diagnostics), vec!["TS7029"]);
}

#[test]
fn no_fallthrough_allows_empty_stacked_labels() {
    let source =
        "export function a(x: number) { switch (x) { case 1: case 2: return; default: return; } }";
    let diagnostics = check_source_with_options(source, "a.ts", no_fallthrough_options(true));
    assert_eq!(codes(&diagnostics), Vec::<String>::new());
}

#[test]
fn no_fallthrough_allows_terminated_clauses() {
    let source = "export function a(x: number) { switch (x) { case 1: return; case 2: throw x; default: break; } }";
    let diagnostics = check_source_with_options(source, "a.ts", no_fallthrough_options(true));
    assert_eq!(codes(&diagnostics), Vec::<String>::new());
}

#[test]
fn no_fallthrough_silent_when_flag_off() {
    let source = "export function a(x: number) { switch (x) { case 1: x; case 2: return; default: return; } }";
    let diagnostics = check_source_with_options(source, "a.ts", no_fallthrough_options(false));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS7029"),
        "got {:?}",
        codes(&diagnostics)
    );
}

fn no_implicit_override_options(no_implicit_override: bool) -> CheckerOptions {
    CheckerOptions {
        no_implicit_override,
        ..Default::default()
    }
}

#[test]
fn no_implicit_override_reports_ts4114_on_missing_override() {
    let source = "class Base { greet(): string { return 'a'; } } class Derived extends Base { greet(): string { return 'b'; } }";
    let diagnostics = check_source_with_options(source, "a.ts", no_implicit_override_options(true));
    assert_eq!(codes(&diagnostics), vec!["TS4114"]);
}

#[test]
fn no_implicit_override_silent_with_override_modifier() {
    let source = "class Base { greet(): string { return 'a'; } } class Derived extends Base { override greet(): string { return 'b'; } }";
    let diagnostics = check_source_with_options(source, "a.ts", no_implicit_override_options(true));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS4114"),
        "got {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_implicit_override_silent_on_new_member() {
    let source = "class Base { greet(): string { return 'a'; } } class Derived extends Base { extra(): number { return 1; } }";
    let diagnostics = check_source_with_options(source, "a.ts", no_implicit_override_options(true));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS4114"),
        "got {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_implicit_override_silent_on_abstract_member_implementation() {
    // Implementing an abstract base member does not require `override`.
    let source = "abstract class Base { abstract run(): number; } class Impl extends Base { run(): number { return 1; } }";
    let diagnostics = check_source_with_options(source, "a.ts", no_implicit_override_options(true));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS4114"),
        "got {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_implicit_override_reports_transitively() {
    let source = "class A { f(): number { return 1; } } class B extends A {} class C extends B { f(): number { return 2; } }";
    let diagnostics = check_source_with_options(source, "a.ts", no_implicit_override_options(true));
    assert_eq!(codes(&diagnostics), vec!["TS4114"]);
}

#[test]
fn no_implicit_override_silent_when_flag_off() {
    let source = "class Base { greet(): string { return 'a'; } } class Derived extends Base { greet(): string { return 'b'; } }";
    let diagnostics =
        check_source_with_options(source, "a.ts", no_implicit_override_options(false));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS4114"),
        "got {:?}",
        codes(&diagnostics)
    );
}

fn no_property_access_index_options(
    no_property_access_from_index_signature: bool,
) -> CheckerOptions {
    CheckerOptions {
        no_property_access_from_index_signature,
        ..Default::default()
    }
}

#[test]
fn no_property_access_from_index_signature_reports_ts4111_on_dot_access() {
    let source = "interface D { [k: string]: number; } declare const d: D; const a = d.foo;";
    let diagnostics =
        check_source_with_options(source, "a.ts", no_property_access_index_options(true));
    assert_eq!(codes(&diagnostics), vec!["TS4111"]);
}

#[test]
fn no_property_access_from_index_signature_allows_declared_property() {
    let source = "interface D { [k: string]: number; declared: number; } declare const d: D; const a = d.declared;";
    let diagnostics =
        check_source_with_options(source, "a.ts", no_property_access_index_options(true));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS4111"),
        "got {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_property_access_from_index_signature_allows_bracket_access() {
    let source = "interface D { [k: string]: number; } declare const d: D; const a = d[\"foo\"];";
    let diagnostics =
        check_source_with_options(source, "a.ts", no_property_access_index_options(true));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS4111"),
        "got {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_property_access_from_index_signature_silent_without_index_signature() {
    let source = "interface P { x: number; } declare const p: P; const a = p.x;";
    let diagnostics =
        check_source_with_options(source, "a.ts", no_property_access_index_options(true));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS4111"),
        "got {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn no_property_access_from_index_signature_silent_when_flag_off() {
    let source = "interface D { [k: string]: number; } declare const d: D; const a = d.foo;";
    let diagnostics =
        check_source_with_options(source, "a.ts", no_property_access_index_options(false));
    assert!(
        !codes(&diagnostics).iter().any(|code| code == "TS4111"),
        "got {:?}",
        codes(&diagnostics)
    );
}

fn ts6133_program_codes(source: &str, no_unused_locals: bool) -> Vec<String> {
    let options = CheckerOptions {
        no_unused_locals,
        ..Default::default()
    };
    let diagnostics = program_with_options(&[("a.ts", source)], options);
    codes(&diagnostics)
        .into_iter()
        .filter(|code| code == "TS6133")
        .collect()
}

#[test]
fn no_unused_locals_reports_unused_const() {
    let source = "export {};\nconst unused = 1;\n";
    assert_eq!(ts6133_program_codes(source, true), vec!["TS6133"]);
}

#[test]
fn no_unused_locals_reports_unused_function() {
    let source = "export {};\nfunction unused(): number { return 1; }\n";
    assert_eq!(ts6133_program_codes(source, true), vec!["TS6133"]);
}

#[test]
fn no_unused_locals_exempts_unused_class() {
    // tsc does not report unused top-level classes under noUnusedLocals.
    let source = "export {};\nclass Unused {}\n";
    assert!(ts6133_program_codes(source, true).is_empty());
}

#[test]
fn no_unused_locals_exempts_used_and_exported() {
    let source = "const used = 1;\nexport const reexported = 2;\nexport const x = used;\n";
    assert!(ts6133_program_codes(source, true).is_empty());
}

#[test]
fn no_unused_locals_ignores_scripts() {
    // No import/export: a script, whose top-level bindings are globals, not locals.
    let source = "const topLevel = 1;\n";
    assert!(ts6133_program_codes(source, true).is_empty());
}

#[test]
fn no_unused_locals_silent_when_flag_off() {
    let source = "export {};\nconst unused = 1;\n";
    assert!(ts6133_program_codes(source, false).is_empty());
}

#[test]
fn no_unused_locals_reports_function_local() {
    let source = "export function f(): number { const unused = 1; const used = 2; return used; }";
    assert_eq!(ts6133_program_codes(source, true), vec!["TS6133"]);
}

#[test]
fn no_unused_locals_counts_local_read_in_nested_block_and_closure() {
    let source = "export function f(): number { const a = 1; const b = 2; if (a > 0) { return a; } return [b].map(x => x)[0]; }";
    assert!(ts6133_program_codes(source, true).is_empty());
}

fn no_unused_parameters_options(no_unused_parameters: bool) -> CheckerOptions {
    CheckerOptions {
        no_unused_parameters,
        ..Default::default()
    }
}

fn ts6133_codes(source: &str, on: bool) -> Vec<String> {
    let diagnostics = check_source_with_options(source, "a.ts", no_unused_parameters_options(on));
    codes(&diagnostics)
        .into_iter()
        .filter(|code| code == "TS6133")
        .collect()
}

#[test]
fn no_unused_parameters_reports_ts6133() {
    let source = "export function f(a: number, b: number): number { return b; }";
    assert_eq!(ts6133_codes(source, true), vec!["TS6133"]);
}

#[test]
fn no_unused_parameters_exempts_underscore_prefix() {
    let source = "export function f(_a: number, b: number): number { return b; }";
    assert!(ts6133_codes(source, true).is_empty());
}

#[test]
fn no_unused_parameters_counts_read_in_nested_function() {
    // The parameter is read only inside a nested function declaration — the oxc
    // read-walk must see it, so no TS6133.
    let source =
        "export function f(p: number): void { function inner(): number { return p; } inner(); }";
    assert!(ts6133_codes(source, true).is_empty());
}

#[test]
fn no_unused_parameters_counts_read_in_template_literal() {
    let source = "export function f(token: string): string { return `Bearer ${token}`; }";
    assert!(ts6133_codes(source, true).is_empty());
}

#[test]
fn no_unused_parameters_counts_read_in_spread() {
    let source = "export function f(p: number[]): number[] { return [...p]; }";
    assert!(ts6133_codes(source, true).is_empty());
}

#[test]
fn no_unused_parameters_skips_overload_signatures() {
    // The bodyless overload signature's parameters must not be flagged; only the
    // implementation is checked (and here `a` is used).
    let source = "export function f(a: string): string;\nexport function f(a: number): string;\nexport function f(a: string | number): string { return String(a); }";
    assert!(ts6133_codes(source, true).is_empty());
}

#[test]
fn no_unused_parameters_silent_when_flag_off() {
    let source = "export function f(a: number, b: number): number { return b; }";
    assert!(ts6133_codes(source, false).is_empty());
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
            diagnostic_profile: DiagnosticProfile::Native,
            resolved_modules: Default::default(),
            resolved_modules_by_importer: Default::default(),
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

    assert_eq!(
        codes(&diagnostics),
        vec!["surge::parser-error", "TS2300", "TS7006", "TS2322"]
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
        ("a.ts", "let greeting = \"Ada\";"),
        ("b.ts", "let value: string = greeting;"),
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
    let diagnostics = native_program(&[
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
            diagnostic_profile: Default::default(),
            resolved_modules: Default::default(),
            resolved_modules_by_importer: Default::default(),
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
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_module_export_function_binding_pattern_no_implicit_any() {
    let diagnostics = program_with_options(
        &[(
            "a.ts",
            "export function f({ id: userId }) { return userId; }",
        )],
        CheckerOptions {
            diagnostic_profile: Default::default(),
            resolved_modules: Default::default(),
            resolved_modules_by_importer: Default::default(),
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

    assert_eq!(codes(&diagnostics), vec!["TS7031"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts"]);
}

#[test]
fn program_module_arrow_function_binding_pattern_no_implicit_any() {
    let diagnostics = program_with_options(
        &[("a.ts", "const fn = ({ id: userId }) => userId;")],
        CheckerOptions {
            diagnostic_profile: Default::default(),
            resolved_modules: Default::default(),
            resolved_modules_by_importer: Default::default(),
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

    assert_eq!(codes(&diagnostics), vec!["TS7031"]);
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

    assert_eq!(codes(&diagnostics), vec!["TS2451", "TS2451"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts", "a.ts"]);
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

    assert_eq!(codes(&diagnostics), vec!["TS2693"]);
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

    assert_eq!(codes(&diagnostics), vec!["TS2693"]);
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

    assert_eq!(codes(&diagnostics), vec!["TS2693"]);
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
    let diagnostics = native_program(&[
        ("a.ts", "import { User from \"./user\";"),
        ("b.ts", "import { User } from \"./missing\";"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["surge::parser-error", "TS2307"]);
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

    assert_eq!(codes(&diagnostics), vec!["TS2393", "TS2393"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts", "a.ts"]);
}

#[test]
fn program_module_duplicate_export_function_same_module_file_ts2393() {
    let diagnostics = program(&[(
        "a.ts",
        "export function getValue(): string { return \"Ada\"; }\nexport function getValue(): number { return 1; }",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2393", "TS2393"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts", "a.ts"]);
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
fn program_module_merge_interface_same_module_file_conflict_ts2717() {
    // Interfaces with the same name in one module file merge; a conflicting
    // property type is reported once as TS2717 rather than a duplicate-identifier.
    let diagnostics = program(&[(
        "a.ts",
        "export {};\ninterface User { name: string; }\ninterface User { name: number; }",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2717"]);
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

    assert_eq!(codes(&diagnostics), vec!["TS2693"]);
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
    let diagnostics = native_program(&[
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
fn ts2882_side_effect_import_emits_ts2882() {
    let diagnostics = program(&[("index.ts", "import \"pkg\";\nlet ok: string = \"ok\";")]);

    assert_eq!(codes(&diagnostics), vec!["TS2882"]);
    assert_eq!(file_names(&diagnostics), vec!["index.ts"]);
}

#[test]
fn ts2882_side_effect_import_does_not_emit_ts2307() {
    let diagnostics = program(&[("index.ts", "import \"pkg\";\nlet ok: string = \"ok\";")]);

    assert!(!codes(&diagnostics).contains(&"TS2307".to_string()));
}

#[test]
fn package_imports_other_package_imports_remain_ts2307() {
    let diagnostics = program(&[(
        "index.ts",
        "import React from \"react\";\nimport type { StoreApi } from \"zustand\";\nexport * from \"zustand/middleware\";",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2307", "TS2307", "TS2307"]);
}

#[test]
fn package_imports_stub_external_modules_ts2882_policy_pinned() {
    let mut options = CheckerOptions::default();
    options.stub_external_modules = true;

    let diagnostics = program_with_options(
        &[("index.ts", "import \"pkg\";\nlet ok: string = \"ok\";")],
        options,
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn package_imports_stub_external_modules_keeps_relative_ts2882() {
    let mut options = CheckerOptions::default();
    options.stub_external_modules = true;

    let diagnostics = program_with_options(
        &[(
            "index.ts",
            "import \"./missing\";\nlet ok: string = \"ok\";",
        )],
        options,
    );

    assert_eq!(codes(&diagnostics), vec!["TS2882"]);
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

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
}

#[test]
fn module_default_import_single_file_still_unresolved_or_unsupported() {
    let diagnostics = check_source(
        "import DefaultThing from \"./thing\";\nlet value = DefaultThing;",
        "index.ts",
    );

    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
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

/// A namespace member reached through a namespace import keeps its namespace
/// qualifier (`ns.Inner.Member`). Flattening it to `ns.Member` in the alias
/// table left the real name unresolvable, so the type silently opened and the
/// member error below was never reported.
#[test]
fn module_namespace_import_qualified_member_resolves_through_namespace() {
    let diagnostics = program(&[
        (
            "core.ts",
            "export namespace Inner { export interface Leaf { depth: number } }",
        ),
        (
            "index.ts",
            "import * as ns from \"./core\";\ndeclare const leaf: ns.Inner.Leaf;\nlet wrong: string = leaf.depth;",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

/// The flattened `ns.Member` spelling is not a real name for a namespace
/// member; resolving it would hide a genuine error behind an open type.
#[test]
fn module_namespace_import_does_not_flatten_qualified_member() {
    let diagnostics = program(&[
        (
            "core.ts",
            "export namespace Inner { export interface Leaf { depth: number } }",
        ),
        (
            "index.ts",
            "import * as ns from \"./core\";\ndeclare const leaf: ns.Leaf;\nlet value = leaf.depth;",
        ),
    ]);

    assert!(
        !codes(&diagnostics).contains(&"TS2322".to_string()),
        "unexpected diagnostics: {:?}",
        codes(&diagnostics)
    );
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

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
}

#[test]
fn mixed_default_named_relative_valid() {
    let diagnostics = program(&[
        (
            "thing.ts",
            "export default function makeThing(): string { return \"Ada\"; }\nexport function helper(): number { return 1; }",
        ),
        (
            "index.ts",
            "import DefaultThing, { helper } from \"./thing\";\nlet name: string = DefaultThing();\nlet count: number = helper();",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn mixed_default_named_relative_missing_default() {
    let diagnostics = program(&[
        ("thing.ts", "export function helper(): number { return 1; }"),
        (
            "index.ts",
            "import DefaultThing, { helper } from \"./thing\";\nlet count: number = helper();",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2305"]);
}

#[test]
fn mixed_default_named_relative_missing_named() {
    let diagnostics = program(&[
        (
            "thing.ts",
            "export default function makeThing(): string { return \"Ada\"; }",
        ),
        (
            "index.ts",
            "import DefaultThing, { helper } from \"./thing\";\nlet name: string = DefaultThing();",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2614"]);
}

#[test]
fn mixed_default_type_named_relative_valid() {
    let diagnostics = program(&[
        (
            "thing.ts",
            "export default function makeThing(): string { return \"Ada\"; }\nexport interface User { name: string; }",
        ),
        (
            "index.ts",
            "import DefaultThing, { type User } from \"./thing\";\nlet user: User = { name: DefaultThing() };",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn mixed_default_named_relative_renamed_valid() {
    let diagnostics = program(&[
        (
            "thing.ts",
            "export default function makeThing(): string { return \"Ada\"; }\nexport function helper(): number { return 1; }",
        ),
        (
            "index.ts",
            "import DefaultThing, { helper as h } from \"./thing\";\nlet name: string = DefaultThing();\nlet count: number = h();",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn mixed_default_named_relative_no_cascade() {
    let diagnostics = program(&[
        ("thing.ts", "export function helper(): number { return 1; }"),
        (
            "index.ts",
            "import DefaultThing, { helper } from \"./thing\";\nlet count: number = helper();\nlet made = DefaultThing();",
        ),
    ]);

    // The default export is missing (TS2305), but the named `helper` binds and
    // the unknown default binding must not cascade into TS2304 on `DefaultThing()`.
    assert_eq!(codes(&diagnostics), vec!["TS2305"]);
}

#[test]
fn mixed_default_named_relative_missing_module_no_cascade() {
    let diagnostics = program(&[(
        "index.ts",
        "import DefaultThing, { helper } from \"./missing\";\nlet count = helper();\nlet made = DefaultThing();",
    )]);

    // The module itself is unresolved (TS2307); both the default and named
    // bindings fall back to unknown so usages must not cascade into TS2304.
    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
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
    let diagnostics = native_program(&[("index.ts", "export default 123;\nexport default 456;")]);

    assert_eq!(codes(&diagnostics), vec!["surge::duplicate-default-export"]);
}

#[test]
fn module_export_default_class_unsupported_no_panic() {
    let diagnostics = program(&[("index.ts", "export default class Foo {}")]);

    assert!(diagnostics.is_empty());
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
fn module_re_export_named_default_valid() {
    let diagnostics = program(&[
        (
            "foo.ts",
            "export default function getName(): string { return \"Ada\"; }",
        ),
        (
            "index.ts",
            "export { default as DefaultThing } from \"./foo\";",
        ),
        (
            "app.ts",
            "import { DefaultThing } from \"./index\";\nlet name: string = DefaultThing();",
        ),
    ]);

    assert!(diagnostics.is_empty());
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

    assert_eq!(codes(&diagnostics), vec!["TS2693"]);
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
        ("a.ts", "export const greeting: string = \"Ada\";"),
        ("b.ts", "export const greeting: number = 1;"),
        ("index.ts", "export * from \"./a\";\nexport * from \"./b\";"),
        (
            "app.ts",
            "import { greeting } from \"./index\";\nlet value: string = greeting;",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn module_re_export_star_local_explicit_wins() {
    let diagnostics = program(&[
        ("other.ts", "export const greeting: number = 1;"),
        (
            "index.ts",
            "export const greeting: string = \"Ada\";\nexport * from \"./other\";",
        ),
        (
            "app.ts",
            "import { greeting } from \"./index\";\nlet value: string = greeting;",
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
    let diagnostics = program(&[
        ("foo.ts", "export const value: number = 1;"),
        ("index.ts", "export * as Foo from \"./foo\";"),
        (
            "app.ts",
            "import { Foo } from \"./index\";\nlet value: number = Foo.value;",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn module_re_export_star_as_missing_module_reports_ts2307() {
    let diagnostics = program(&[("index.ts", "export * as Foo from \"./foo\";")]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
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
    let diagnostics = native_program(&[(
        "index.ts",
        "export default 123;\nexport default 456;\nlet value: string = 123;",
    )]);

    assert_eq!(
        codes(&diagnostics),
        vec!["surge::duplicate-default-export", "TS2322"]
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

#[test]
fn single_file_external_named_import_reports_ts2307_no_cascade() {
    let source = r#"
        import { useState } from "react";
        let state = useState();
    "#;
    let options = CheckerOptions::default();
    let diagnostics = check_source_with_options(source, "test.ts", options);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code.to_string(), "TS2307");
}

#[test]
fn single_file_external_named_import_stub_mode_suppresses_ts2307() {
    let source = r#"
        import { useState } from "react";
        let state = useState();
    "#;
    let mut options = CheckerOptions::default();
    options.stub_external_modules = true;
    let diagnostics = check_source_with_options(source, "test.ts", options);
    println!("{:?}", diagnostics);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn single_file_external_type_only_import_reports_ts2307_no_cascade() {
    let source = r#"
        import type { Store } from "zustand";
        let x: Store = null as any;
    "#;
    let options = CheckerOptions::default();
    let diagnostics = check_source_with_options(source, "test.ts", options);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code.to_string(), "TS2307");
}

#[test]
fn single_file_external_type_only_import_stub_mode_suppresses_ts2307() {
    let source = r#"
        import type { Store } from "zustand";
        let x: Store = null as any;
    "#;
    let mut options = CheckerOptions::default();
    options.stub_external_modules = true;
    let diagnostics = check_source_with_options(source, "test.ts", options);
    println!("{:?}", diagnostics);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn single_file_external_default_import_reports_ts2307_no_cascade() {
    let source = r#"
        import React from "react";
        let r = React;
    "#;
    let options = CheckerOptions::default();
    let diagnostics = check_source_with_options(source, "test.ts", options);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code.to_string(), "TS2307");
}

#[test]
fn single_file_external_default_import_stub_mode_suppresses_ts2307() {
    let source = r#"
        import React from "react";
        let r = React;
    "#;
    let mut options = CheckerOptions::default();
    options.stub_external_modules = true;
    let diagnostics = check_source_with_options(source, "test.ts", options);
    println!("{:?}", diagnostics);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn single_file_external_namespace_import_reports_ts2307_no_cascade() {
    let source = r#"
        import * as Zustand from "zustand";
        let store = Zustand;
    "#;
    let options = CheckerOptions::default();
    let diagnostics = check_source_with_options(source, "test.ts", options);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code.to_string(), "TS2307");
}

#[test]
fn single_file_external_namespace_import_stub_mode_suppresses_ts2307() {
    let source = r#"
        import * as Zustand from "zustand";
        let store = Zustand;
    "#;
    let mut options = CheckerOptions::default();
    options.stub_external_modules = true;
    let diagnostics = check_source_with_options(source, "test.ts", options);
    println!("{:?}", diagnostics);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn single_file_external_namespace_property_access_no_cascade() {
    let source = r#"
        import * as Zustand from "zustand";
        let store = Zustand.createStore;
    "#;
    let options = CheckerOptions::default();
    let diagnostics = check_source_with_options(source, "test.ts", options);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(diagnostics[0].code.to_string(), "TS2307");
}

#[test]
fn program_external_namespace_property_access_no_cascade() {
    let files = vec![SourceFileInput {
        file_name: "test.ts".to_string(),
        source_text: r#"
            import * as Zustand from "zustand";
            let store = Zustand.createStore;
        "#
        .to_string(),
    }];
    let options = CheckerOptions::default();
    let result = check_program_with_options(files, options);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].code.to_string(), "TS2307");
}

#[test]
fn namespace_generic_member_shadows_nongeneric_ambient_global() {
    // A namespace member's generic interface (`Ev<T>`) must shadow a same-named
    // non-generic ambient global (`interface Ev`) when referenced from a sibling
    // member. Otherwise a handler-alias chain (`Handler<T> = Fn<Ev<T>>`) resolves
    // `Ev` to the arity-0 global, applying `<T>` degrades it to a non-function, and
    // a callback contextually typed by `Handler` falsely reports TS7006 — the root
    // cause of the React `onClick={(e) => …}` / `render={({ field }) => …}` over-reports.
    let mut options = CheckerOptions::default();
    options.no_implicit_any = true;
    let diagnostics = program_with_options(
        &[(
            "src/index.ts",
            "declare interface Ev { a: number; }\n\
             declare namespace NS {\n\
               interface Base<T> { x: T }\n\
               interface Ev<T> extends Base<T> { y: number }\n\
               type Fn<E extends Base<any>> = (e: E) => void;\n\
               type Handler<T> = Fn<Ev<T>>;\n\
             }\n\
             declare function on(cb: NS.Handler<number>): void;\n\
             on((e) => e.x);",
        )],
        options,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
}

#[test]
fn imported_namespace_member_alias_resolves_siblings_under_original_prefix() {
    // `import { Handler }` renames the namespace member to its bare local form, but
    // its body still references siblings (`Fn`, `Ev`) that only resolve under the
    // member's original `NS.` prefix. The prefix is recovered from `declared_name`
    // (the qualified source name), not the bare binding. Regression for React's
    // `import { MouseEventHandler } from "react"` falsely reporting TS7006 on the
    // contextually-typed callback parameter.
    let mut options = CheckerOptions::default();
    options.no_implicit_any = true;
    options
        .resolved_modules
        .insert("lib".to_string(), "node_modules/lib/index.d.ts".to_string());
    let diagnostics = program_with_options(
        &[
            (
                "node_modules/lib/index.d.ts",
                "export = NS;\n\
                 declare namespace NS {\n\
                   interface Ev<T> { x: T }\n\
                   type Fn<E> = (e: E) => void;\n\
                   type Handler<T> = Fn<Ev<T>>;\n\
                 }",
            ),
            (
                "src/index.ts",
                "import { Handler } from 'lib';\n\
                 const h: Handler<number> = (e) => e.x;",
            ),
        ],
        options,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
}

#[test]
fn program_stub_external_modules_keeps_relative_missing_module_ts2307() {
    let files = vec![SourceFileInput {
        file_name: "test.ts".to_string(),
        source_text: r#"
            import { X } from "./missing";
        "#
        .to_string(),
    }];
    let mut options = CheckerOptions::default();
    options.stub_external_modules = true;
    let result = check_program_with_options(files, options);
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].code.to_string(), "TS2307");
}

#[test]
fn ambient_module_resolves_before_package_stub_default() {
    let diagnostics = program(&[
        ("src/index.ts", "import { foo } from \"pkg\";"),
        (
            "types/pkg.d.ts",
            "declare module \"pkg\" { export const foo: number; }",
        ),
    ]);
    println!("{:?}", diagnostics);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn ambient_module_resolves_before_package_stub_with_stub_external_modules() {
    let diagnostics = program_with_options(
        &[
            ("src/index.ts", "import { foo } from \"pkg\";"),
            (
                "types/pkg.d.ts",
                "declare module \"pkg\" { export const foo: number; }",
            ),
        ],
        CheckerOptions {
            diagnostic_profile: Default::default(),
            resolved_modules: Default::default(),
            resolved_modules_by_importer: Default::default(),
            stub_external_modules: true,
            ..Default::default()
        },
    );
    println!("{:?}", diagnostics);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn ambient_module_missing_export_ts2305_not_ts2307() {
    let diagnostics = program(&[
        ("src/index.ts", "import { missing } from \"pkg\";"),
        (
            "types/pkg.d.ts",
            "declare module \"pkg\" { export const foo: number; }",
        ),
    ]);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(codes(&diagnostics)[0], "TS2305");
}

#[test]
fn ambient_module_missing_export_ts2305_not_ts2307_with_stub_external_modules() {
    let diagnostics = program_with_options(
        &[
            ("src/index.ts", "import { missing } from \"pkg\";"),
            (
                "types/pkg.d.ts",
                "declare module \"pkg\" { export const foo: number; }",
            ),
        ],
        CheckerOptions {
            diagnostic_profile: Default::default(),
            resolved_modules: Default::default(),
            resolved_modules_by_importer: Default::default(),
            stub_external_modules: true,
            ..Default::default()
        },
    );
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(codes(&diagnostics)[0], "TS2305");
}

#[test]
fn ambient_module_exact_specifier_only() {
    let diagnostics = program(&[
        ("src/index.ts", "import { foo } from \"pkg/subpath\";"),
        (
            "types/pkg.d.ts",
            "declare module \"pkg\" { export const foo: number; }",
        ),
    ]);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(codes(&diagnostics)[0], "TS2307");
}

#[test]
fn ambient_module_unknown_specifier_fallback_ts2307() {
    let diagnostics = program(&[
        ("src/index.ts", "import { missing } from \"missing-pkg\";"),
        (
            "types/pkg.d.ts",
            "declare module \"pkg\" { export const foo: number; }",
        ),
    ]);
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(codes(&diagnostics)[0], "TS2307");
}

#[test]
fn ambient_module_unknown_specifier_stub_external_modules_suppresses_ts2307() {
    let diagnostics = program_with_options(
        &[
            ("src/index.ts", "import { missing } from \"missing-pkg\";"),
            (
                "types/pkg.d.ts",
                "declare module \"pkg\" { export const foo: number; }",
            ),
        ],
        CheckerOptions {
            diagnostic_profile: Default::default(),
            resolved_modules: Default::default(),
            resolved_modules_by_importer: Default::default(),
            stub_external_modules: true,
            ..Default::default()
        },
    );
    println!("{:?}", diagnostics);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn ambient_module_default_export_value_valid() {
    let diagnostics = program(&[
        (
            "src/index.ts",
            "import value from \"pkg-default\"; let ok: string = value;",
        ),
        (
            "types/pkg-default.d.ts",
            "declare module \"pkg-default\" { export const value: string; export default value; }",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn ambient_module_default_export_value_mismatch() {
    let diagnostics = program(&[
        (
            "src/index.ts",
            "import value from \"pkg-default\"; let bad: number = value;",
        ),
        (
            "types/pkg-default.d.ts",
            "declare module \"pkg-default\" { export const value: string; export default value; }",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

#[test]
fn ambient_module_default_import_missing_default_ts2305() {
    let diagnostics = program(&[
        ("src/index.ts", "import value from \"pkg-default\";"),
        (
            "types/pkg-default.d.ts",
            "declare module \"pkg-default\" { export const value: string; }",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2305"]);
}

#[test]
fn ambient_module_default_import_missing_module_fallback_ts2307() {
    let diagnostics = program(&[("src/index.ts", "import value from \"missing-pkg\";")]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
}

#[test]
fn ambient_module_default_import_missing_module_stub_external_suppresses_ts2307() {
    let diagnostics = program_with_options(
        &[("src/index.ts", "import value from \"missing-pkg\";")],
        CheckerOptions {
            diagnostic_profile: Default::default(),
            resolved_modules: Default::default(),
            resolved_modules_by_importer: Default::default(),
            stub_external_modules: true,
            ..Default::default()
        },
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn ambient_module_default_export_function_valid_or_pinned() {
    let diagnostics = program(&[
        (
            "src/index.ts",
            "import getName from \"pkg-default-function\"; let name: string = getName();",
        ),
        (
            "types/pkg-default-function.d.ts",
            "declare module \"pkg-default-function\" { export default function getName(): string; }",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn ambient_module_namespace_import_value_property_valid() {
    let diagnostics = program(&[
        (
            "src/index.ts",
            "import * as pkg from \"pkg-ns\"; let ok: string = pkg.value; let name: string = pkg.getName();",
        ),
        (
            "types/pkg-ns.d.ts",
            "declare module \"pkg-ns\" { export const value: string; export function getName(): string; export interface User { name: string; } }",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn ambient_module_namespace_import_missing_property_ts2339() {
    let diagnostics = program(&[
        (
            "src/index.ts",
            "import * as pkg from \"pkg-ns\"; let missing = pkg.missing;",
        ),
        (
            "types/pkg-ns.d.ts",
            "declare module \"pkg-ns\" { export const value: string; export function getName(): string; }",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2339"]);
}

#[test]
fn ambient_module_namespace_import_type_export_not_value_property() {
    let diagnostics = program(&[
        (
            "src/index.ts",
            "import * as pkg from \"pkg-ns\"; let user = pkg.User;",
        ),
        (
            "types/pkg-ns.d.ts",
            "declare module \"pkg-ns\" { export interface User { name: string; } }",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2339"]);
}

#[test]
fn ambient_module_namespace_import_default_property_valid_or_pinned() {
    let diagnostics = program(&[
        (
            "src/index.ts",
            "import * as pkg from \"pkg-default\"; let ok: string = pkg.default;",
        ),
        (
            "types/pkg-default.d.ts",
            "declare module \"pkg-default\" { export const value: string; export default value; }",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn ambient_module_namespace_import_unknown_module_fallback_ts2307() {
    let diagnostics = program(&[("src/index.ts", "import * as pkg from \"missing-pkg\";")]);

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
}

#[test]
fn ambient_module_namespace_import_unknown_module_stub_external_suppresses_ts2307() {
    let diagnostics = program_with_options(
        &[("src/index.ts", "import * as pkg from \"missing-pkg\";")],
        CheckerOptions {
            diagnostic_profile: Default::default(),
            resolved_modules: Default::default(),
            resolved_modules_by_importer: Default::default(),
            stub_external_modules: true,
            ..Default::default()
        },
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn ambient_module_missing_named_export_no_assignment_cascade() {
    let diagnostics = program(&[
        (
            "src/index.ts",
            "import { missing } from \"pkg\"; let x: number = missing;",
        ),
        (
            "types/pkg.d.ts",
            "declare module \"pkg\" { export const value: string; }",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2305"]);
}

#[test]
fn ambient_module_missing_named_export_no_call_cascade() {
    let diagnostics = program(&[
        (
            "src/index.ts",
            "import { missing } from \"pkg\"; missing();",
        ),
        (
            "types/pkg.d.ts",
            "declare module \"pkg\" { export const value: string; }",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2305"]);
}

#[test]
fn ambient_module_missing_named_export_no_property_cascade() {
    let diagnostics = program(&[
        (
            "src/index.ts",
            "import { missing } from \"pkg\"; missing.property;",
        ),
        (
            "types/pkg.d.ts",
            "declare module \"pkg\" { export const value: string; }",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2305"]);
}

#[test]
fn ambient_module_missing_type_export_no_cascade() {
    let diagnostics = program(&[
        (
            "src/index.ts",
            "import type { missing } from \"pkg\"; type X = missing;",
        ),
        (
            "types/pkg.d.ts",
            "declare module \"pkg\" { export const value: string; }",
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2305"]);
}

#[test]
fn ambient_module_named_re_export_value_valid() {
    let diagnostics = program(&[
        (
            "src/index.ts",
            "import { User, value } from \"barrel-pkg\"; let user: User = { name: value };",
        ),
        (
            "types/ambient.d.ts",
            r#"
            declare module "source-pkg" {
                export interface User { name: string; }
                export const value: string;
            }

            declare module "barrel-pkg" {
                export { User, value } from "source-pkg";
            }
            "#,
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn ambient_module_type_only_re_export_valid() {
    let diagnostics = program(&[
        (
            "src/index.ts",
            "import type { User } from \"barrel-type-pkg\"; let user: User = { name: \"Ada\" };",
        ),
        (
            "types/ambient.d.ts",
            r#"
            declare module "source-pkg" {
                export interface User { name: string; }
            }

            declare module "barrel-type-pkg" {
                export type { User } from "source-pkg";
            }
            "#,
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn ambient_module_star_re_export_valid_or_pinned() {
    let diagnostics = program(&[
        (
            "src/index.ts",
            "import { User, value } from \"barrel-star-pkg\"; let user: User = { name: value };",
        ),
        (
            "types/ambient.d.ts",
            r#"
            declare module "source-pkg" {
                export interface User { name: string; }
                export const value: string;
            }

            declare module "barrel-star-pkg" {
                export * from "source-pkg";
            }
            "#,
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn ambient_module_re_export_missing_member_ts2305() {
    let diagnostics = program(&[
        ("src/index.ts", "import { missing } from \"barrel-pkg\";"),
        (
            "types/ambient.d.ts",
            r#"
            declare module "source-pkg" {
                export const value: string;
            }

            declare module "barrel-pkg" {
                export { missing } from "source-pkg";
            }
            "#,
        ),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2305"]);
}

#[test]
fn ambient_module_re_export_unknown_source_stub_external_modules_behavior() {
    let diagnostics = program_with_options(
        &[
            ("src/index.ts", "import { User } from \"barrel-pkg\";"),
            (
                "types/ambient.d.ts",
                r#"
                declare module "barrel-pkg" {
                    export { User } from "missing-pkg";
                }
                "#,
            ),
        ],
        CheckerOptions {
            diagnostic_profile: Default::default(),
            resolved_modules: Default::default(),
            resolved_modules_by_importer: Default::default(),
            stub_external_modules: true,
            ..Default::default()
        },
    );

    assert!(diagnostics.is_empty());
}

#[test]
fn ambient_module_duplicate_declarations_merge_policy() {
    let diagnostics = program(&[
        (
            "src/index.ts",
            "import { a, b } from \"merge-pkg\"; let okA: string = a; let okB: number = b;",
        ),
        (
            "types/ambient.d.ts",
            r#"
            declare module "merge-pkg" {
                export const a: string;
            }

            declare module "merge-pkg" {
                export const b: number;
            }
            "#,
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn ambient_module_duplicate_default_export_policy_pinned() {
    let diagnostics = program(&[
        (
            "src/index.ts",
            "import value from \"dup-default-pkg\"; let ok: string = value;",
        ),
        (
            "types/ambient.d.ts",
            r#"
            declare module "dup-default-pkg" {
                export default "first";
            }

            declare module "dup-default-pkg" {
                export default 123;
            }
            "#,
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn ambient_module_duplicate_type_export_policy_pinned() {
    // Reopened ambient module blocks merge their exported interfaces; on a
    // conflicting property the first declaration wins and no diagnostic is
    // surfaced for the ambient declaration file (pinned, low-cascade policy).
    let diagnostics = program(&[
        (
            "src/index.ts",
            "import type { User } from \"dup-type-pkg\"; let ok: User = { name: \"Ada\" };",
        ),
        (
            "types/ambient.d.ts",
            r#"
            declare module "dup-type-pkg" {
                export interface User { name: string; }
            }

            declare module "dup-type-pkg" {
                export interface User { name: number; }
            }
            "#,
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn ambient_global_duplicate_const_policy_pinned() {
    let diagnostics = program(&[
        ("src/index.ts", "let ok: string = value;"),
        ("types/a.d.ts", "declare const value: string;"),
        ("types/b.d.ts", "declare const value: number;"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn ambient_global_duplicate_function_policy_pinned() {
    let diagnostics = program(&[
        ("src/index.ts", "let ok: string = getName();"),
        ("types/a.d.ts", "declare function getName(): string;"),
        ("types/b.d.ts", "declare function getName(): number;"),
    ]);

    // tsc merges the two ambient `declare function getName` declarations as an
    // overload set (NOT a duplicate implementation): it reports no TS2393 and
    // instead a TS2322 at the call site once the `number` overload is selected.
    // surge no longer emits the false TS2393 here. It does not yet build a true
    // overload set (it keeps the first signature, so `getName()` stays `string`
    // and the TS2322 is under-reported) — a separate overload-merging limitation,
    // tracked distinctly from the duplicate-implementation policy this pins.
    assert!(
        !codes(&diagnostics).contains(&"TS2393".to_string()),
        "ambient function overloads must not be flagged as duplicate implementations: {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn ambient_generic_function_constrained_indexed_access_no_ts2536() {
    // An ambient `declare function` whose signature indexes a concrete type by a
    // constrained type parameter (`K extends keyof EventMap` → `EventMap[K]`, as
    // the lib `addEventListener` does) must not emit a false TS2536. The ambient
    // collection path resolves this signature authoritatively (no body follows),
    // so it must do so under the function's own type-parameter scope. Single-file
    // checking always had that scope; this pins the project/ambient path.
    let diagnostics = program(&[
        ("src/index.ts", "export const x = 1;"),
        (
            "types/dom.d.ts",
            "interface BaseMap { click: number; }\n\
             interface EventMap extends BaseMap { focus: string; }\n\
             declare function on<K extends keyof EventMap>(type: K, listener: (this: object, ev: EventMap[K]) => any): void;\n\
             declare function on(type: string, listener: () => void): void;",
        ),
    ]);

    assert!(
        diagnostics.is_empty(),
        "constrained indexed access in an ambient generic signature must not cascade: {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn ambient_overload_lowering_preserves_contextual_source_order() {
    let diagnostics = program(&[
        (
            "src/index.ts",
            "on(\"click\", (event) => { const value: number = event; });",
        ),
        (
            "types/dom.d.ts",
            "interface EventMap { click: number; }\n\
             declare function on<K extends keyof EventMap>(type: K, listener: (event: EventMap[K]) => void): void;\n\
             declare function on(type: string, listener: (() => void) | object): void;",
        ),
    ]);

    assert!(
        diagnostics.is_empty(),
        "the generic ambient overload must contextually type the callback: {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn ambient_global_typeof_global_this_intersection_resolves_to_left() {
    // `declare const w: Win & typeof globalThis` (the lib shape of `window`/`self`)
    // resolves `typeof globalThis` before the global object symbol is installed.
    // Treating that miss as a clean `unknown` plus the `T & unknown ⇒ T`
    // simplification keeps `w` typed as `Win`, so member access is checked against
    // it — `w.bar` is `string`. The earlier behaviour emitted a (suppressed) TS2304
    // and poisoned `w` to `unknown`, silently dropping the member check; an eager
    // re-merge instead corrupted the shared `Win` apparent type.
    let diagnostics = program(&[
        (
            "src/index.ts",
            "export const ok: string = w.bar;\nexport const bad: number = w.bar;",
        ),
        (
            "types/globals.d.ts",
            "interface Win { bar: string; }\ndeclare const w: Win & typeof globalThis;",
        ),
    ]);

    assert_eq!(
        codes(&diagnostics),
        vec!["TS2322"],
        "w.bar must resolve to Win.bar (string): only the number assignment errors: {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn ambient_global_user_source_shadow_or_duplicate_policy_pinned() {
    let diagnostics = program(&[
        (
            "src/index.ts",
            "let user: User = { name: \"Ada\" }; let value: string = User;",
        ),
        (
            "types/globals.d.ts",
            "declare interface User { name: string; } declare const User: string;",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn declaration_file_does_not_run_statement_body_checks() {
    let diagnostics = program(&[
        ("src/index.ts", "let x: number = 1;"),
        ("types/globals.d.ts", "const missingInit: number;"),
    ]);
    println!("{:?}", diagnostics);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn declaration_file_ambient_globals_are_visible_in_program() {
    let diagnostics = program(&[
        (
            "src/index.ts",
            "let id: ID = \"ok\"; let user: User = { name: \"Ada\" };",
        ),
        (
            "types/globals.d.ts",
            "declare type ID = string; declare interface User { name: string; }",
        ),
    ]);
    assert!(diagnostics.is_empty());
}

#[test]
fn declaration_file_type_alias_no_statement_check() {
    let diagnostics = program(&[
        ("src/index.ts", "let ok: Name = \"Ada\";"),
        ("types/globals.d.ts", "declare type Name = string;"),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn declaration_file_interface_no_statement_check() {
    let diagnostics = program(&[
        ("src/index.ts", "let ok: User = { name: \"Ada\" };"),
        (
            "types/globals.d.ts",
            "declare interface User { name: string; }",
        ),
    ]);

    assert!(diagnostics.is_empty());
}

#[test]
fn declaration_file_declare_function_no_body_valid() {
    let diagnostics = program(&[("types/globals.d.ts", "declare function foo(): number;")]);
    println!("{:?}", diagnostics);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn declaration_file_declare_const_no_initializer_valid() {
    let diagnostics = program(&[("types/globals.d.ts", "declare const foo: number;")]);
    println!("{:?}", diagnostics);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn declaration_file_declare_class_valid() {
    // `declare class` is now supported: it contributes a global value/type and
    // should not produce an unsupported-declaration diagnostic.
    let diagnostics = native_program(&[("types/globals.d.ts", "declare class Foo {}")]);
    assert_eq!(diagnostics.len(), 0);
}

#[test]
fn class_new_expression_checks_constructor_args_and_returns_instance() {
    let diagnostics = program(&[(
        "example.ts",
        "class User { id: string; constructor(id: string) { this.id = id; } }\n\
         const ok = new User(\"a\");\n\
         const okId: string = ok.id;\n\
         const bad = new User(123);",
    )]);
    assert_eq!(codes(&diagnostics), vec!["TS2345"]);
}

#[test]
fn class_constructor_parameter_property_declares_instance_member() {
    let diagnostics = program(&[(
        "example.ts",
        "class C {\n\
         constructor(private readonly buf: string) {}\n\
         method(): string { return this.buf; }\n\
         }\n\
         const c = new C(\"a\");\n\
         const s: string = c.method();",
    )]);
    assert_eq!(codes(&diagnostics), Vec::<String>::new());
}

#[test]
fn class_constructor_plain_parameter_is_not_a_member() {
    let diagnostics = program(&[(
        "example.ts",
        "class C {\n\
         constructor(buf: string) {}\n\
         method(): string { return this.buf; }\n\
         }",
    )]);
    assert_eq!(codes(&diagnostics), vec!["TS2339"]);
}

#[test]
fn class_instance_access_of_static_member_reports_ts2576() {
    let diagnostics = program(&[(
        "example.ts",
        "class User { id: string; static version: string; constructor(id: string) { this.id = id; } }\n\
         const user = new User(\"a\");\n\
         user.version;",
    )]);
    assert_eq!(codes(&diagnostics), vec!["TS2576"]);
}

#[test]
fn class_static_access_of_instance_member_reports_ts2339() {
    let diagnostics = program(&[(
        "example.ts",
        "class User { id: string; static version: string; constructor(id: string) { this.id = id; } }\n\
         User.id;",
    )]);
    assert_eq!(codes(&diagnostics), vec!["TS2339"]);
}

#[test]
fn declaration_file_unsupported_enum_still_reports() {
    let diagnostics = native_program(&[("types/globals.d.ts", "declare enum E {}")]);
    assert_eq!(codes(&diagnostics), vec!["surge::unsupported-declaration"]);
}

#[test]
fn declaration_file_namespace_is_supported() {
    // Identifier-named namespaces are parsed so their members (e.g.
    // `JSX.IntrinsicElements`) can resolve; an empty namespace is inert, matching
    // tsc which reports nothing here.
    let diagnostics = native_program(&[("types/globals.d.ts", "declare namespace N {}")]);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

#[test]
fn declaration_file_global_augmentation_is_supported() {
    let diagnostics = native_program(&[("types/globals.d.ts", "declare global {}")]);
    assert!(diagnostics.is_empty());
}

#[test]
fn declaration_file_export_equals_unresolved_target_no_cascade() {
    // `export = identifier` is a supported declaration-lite form. An unresolved
    // target binds nothing and emits no diagnostic (no cascade), rather than the
    // old unsupported-declaration report.
    let diagnostics = native_program(&[("types/globals.d.ts", "export = Foo;")]);
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn export_equals_import_require_binds_value_and_property_call() {
    let diagnostics = native_program(&[
        (
            "pkg.d.ts",
            "declare const auth: { sign(input: string): string };\nexport = auth;",
        ),
        (
            "consumer.ts",
            "import auth = require(\"./pkg\");\nconst token: string = auth.sign(\"x\");",
        ),
    ]);
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn export_equals_import_require_property_call_argument_mismatch() {
    let diagnostics = native_program(&[
        (
            "pkg.d.ts",
            "declare const auth: { sign(input: string): string };\nexport = auth;",
        ),
        (
            "consumer.ts",
            "import auth = require(\"./pkg\");\nconst token: string = auth.sign(123);",
        ),
    ]);
    assert_eq!(codes(&diagnostics), vec!["TS2345"]);
}

#[test]
fn export_equals_unresolved_target_import_require_no_cascade() {
    // The exported identifier is undefined in the package surface; the consumer
    // binds an unknown value and must not cascade name/property errors.
    let diagnostics = native_program(&[
        ("pkg.d.ts", "export = missingValue;"),
        (
            "consumer.ts",
            "import api = require(\"./pkg\");\nconst result = api.whatever();",
        ),
    ]);
    assert!(
        !codes(&diagnostics)
            .iter()
            .any(|code| code == "TS2304" || code == "TS2339" || code == "TS2571"),
        "unexpected cascade: {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn declaration_file_import_equals_missing_module_reports_ts2307() {
    // `import x = require("specifier")` is supported; a missing module surfaces
    // the existing missing-module diagnostic instead of unsupported-declaration.
    let diagnostics = native_program(&[("types/globals.d.ts", "import Foo = require(\"foo\");")]);
    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
}

#[test]
fn declaration_file_unsupported_wildcard_module_still_reports() {
    let diagnostics = native_program(&[("types/globals.d.ts", "declare module \"*\" {}")]);
    assert_eq!(codes(&diagnostics), vec!["surge::unsupported-declaration"]);
}

#[test]
fn native_profile_suppresses_indexed_access_cascade() {
    let files = vec![SourceFileInput {
        file_name: "index.ts".to_string(),
        source_text: "
        interface User { name: string; }
        type UnresolvedKeyIndex = User[MissingKeyName];
        let _trigger: UnresolvedKeyIndex;
        "
        .to_string(),
    }];

    let mut options = CheckerOptions::default();
    // Default is Tsc profile
    let tsc_diagnostics = check_program_with_options(files.clone(), options.clone());
    assert_eq!(codes(&tsc_diagnostics), vec!["TS2304", "TS2538"]);

    options.diagnostic_profile = surge_ts_checker::DiagnosticProfile::Native;
    let native_diagnostics = check_program_with_options(files, options);
    assert_eq!(codes(&native_diagnostics), vec!["TS2304"]);
}

#[test]
fn indexed_access_unresolved_object_reports_only_missing_type() {
    let files = vec![SourceFileInput {
        file_name: "index.ts".to_string(),
        source_text: "
        type UnresolvedObjectIndex = MissingObject[\"x\"];
        let _trigger: UnresolvedObjectIndex;
        "
        .to_string(),
    }];

    let diagnostics = check_program_with_options(files, CheckerOptions::default());
    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
}

// `infer` capture inside a function-parameter position resolves to the matched
// argument: the excess-property error proves `R` is the concrete `{ id: string }`
// props object, not a degraded `unknown`/`any`. Regression guard for the
// function-pattern arm of conditional `infer` binding (the core of
// `React.ComponentProps<typeof FunctionComponent>`).
#[test]
fn infer_capture_through_inline_function_parameter() {
    let diagnostics = check_source(
        "type FirstParam<T> = T extends (props: infer P) => any ? P : never;\n\
         declare const comp: (props: { id: string }) => void;\n\
         type R = FirstParam<typeof comp>;\n\
         const bad: R = { nope: 1 };\n",
        "example.ts",
    );

    assert_eq!(codes(&diagnostics), vec!["TS2353"]);
}

// `infer` capture reached by expanding a generic alias whose body is a function
// (`Ctor<infer P>`), matching React's `JSXElementConstructor<infer Props>` shape.
#[test]
fn infer_capture_through_generic_function_alias() {
    let diagnostics = check_source(
        "type Ctor<P> = (props: P) => unknown;\n\
         type PropsOf<T> = T extends Ctor<infer P> ? P : never;\n\
         declare const comp: (props: { id: string }) => unknown;\n\
         type R = PropsOf<typeof comp>;\n\
         const bad: R = { nope: 1 };\n",
        "example.ts",
    );

    assert_eq!(codes(&diagnostics), vec!["TS2353"]);
}

// A constructor signature (`new (...) => T`) as a union member must not collapse
// the whole union to `unknown`. This mirrors React's
// `JSXElementConstructor<P> = ((props: P) => …) | (new (props: P) => …)`; the
// props type is recovered from the call-signature member.
#[test]
fn constructor_type_union_member_preserves_call_signature_infer() {
    let diagnostics = check_source(
        "type ElementCtor<P> = ((props: P) => string) | (new (props: P) => object);\n\
         type PropsOf<T> = T extends ElementCtor<infer P> ? P : never;\n\
         declare const widget: (props: { title: string }) => string;\n\
         type R = PropsOf<typeof widget>;\n\
         const bad: R = { other: 1 };\n",
        "example.ts",
    );

    assert_eq!(codes(&diagnostics), vec!["TS2353"]);
}

// `infer` capture against a callable object (an interface carrying a call
// signature, e.g. React's `ForwardRefExoticComponent<P>`) resolves through the
// object's call signature rather than degrading.
#[test]
fn infer_capture_through_callable_interface_signature() {
    let diagnostics = check_source(
        "interface Callable { (props: { id: string }): void; }\n\
         declare const callable: Callable;\n\
         type FirstParam<T> = T extends (props: infer P) => any ? P : never;\n\
         type R = FirstParam<typeof callable>;\n\
         const bad: R = { nope: 1 };\n",
        "example.ts",
    );

    assert_eq!(codes(&diagnostics), vec!["TS2353"]);
}

// An arrow argument whose parameter type is a callable object (an interface
// carrying a call signature, e.g. React's `ForwardRefRenderFunction`) is
// contextually typed by that call signature rather than left implicit-any: `v`
// resolves to `number`, so assigning it to `string` is the only diagnostic.
#[test]
fn callable_object_parameter_contextually_types_arrow() {
    let diagnostics = check_source(
        "interface Render { (x: number): void; }\n\
         declare function take(fn: Render): void;\n\
         take((v) => { const s: string = v; });\n",
        "example.ts",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

// A function value is assignable to a callable object target when it matches the
// target's call signature, and not when its parameter is incompatible — so only
// the mismatched call is rejected.
#[test]
fn function_assignable_to_callable_object_target() {
    let diagnostics = check_source(
        "interface Render { (x: number): void; }\n\
         declare function take(fn: Render): void;\n\
         const ok = (n: number): void => { void n; };\n\
         const bad = (s: string): void => { void s; };\n\
         take(ok);\n\
         take(bad);\n",
        "example.ts",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2345"]);
}

// A tuple-typed rest parameter (`...args: [name: string]`) — and a union of
// tuples, next's cookie-store overload shape — accepts each argument at its
// tuple position instead of comparing the whole tuple/union against every
// argument. Only the genuinely mismatched call reports.
#[test]
fn tuple_rest_parameter_accepts_positional_arguments() {
    let diagnostics = check_source(
        "declare function get(...args: [string] | [{ name: string }]): void;\n\
         get(\"NEXT_LOCALE\");\n\
         get({ name: \"lang\" });\n\
         get(123);\n",
        "example.ts",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2345"]);
}

// A spread of a nominally-typed source (`{ ...defaults, ...props }` where both
// are `Props`) contributes the reference's members instead of being skipped, so
// destructured names resolve rather than reporting TS2339 on `{}`.
#[test]
fn object_literal_spread_peels_nominal_reference() {
    let diagnostics = check_source(
        "interface Props { url: string; email: string }\n\
         const defaults: Props = { url: \"u\", email: \"e\" };\n\
         function render(props: Props = defaults): string {\n\
             const { url, email } = { ...defaults, ...props };\n\
             return url + email;\n\
         }\n",
        "example.ts",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// A callable object used as a JSX component (a `forwardRef`/`memo`-style exotic
// component, which is callable rather than a bare function) has its props checked
// through its call signature.
#[test]
fn jsx_callable_object_component_checks_props() {
    let diagnostics = check_source(
        "interface Btn { (props: { label: string }): null; }\n\
         declare const Button: Btn;\n\
         const bad = <Button label={123} />;\n",
        "example.tsx",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

// tsc never excess-checks hyphenated JSX attribute names (`data-slot`,
// `aria-*`), while a non-hyphenated unknown attribute still reports.
#[test]
fn jsx_hyphenated_attribute_is_not_excess_checked() {
    let diagnostics = check_source(
        "declare function Item(props: { label?: string }): null;\n\
         const ok = <Item data-slot=\"x\" aria-bogus=\"y\" />;\n\
         const bad = <Item dataslot=\"x\" />;\n",
        "example.tsx",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}

// A `{...spread}` whose type resolves to an object contributes its members, so
// a required prop carried by the spread is not reported missing (the shadcn
// wrapper idiom), while a spread without it still reports TS2741.
#[test]
fn jsx_spread_attributes_cover_required_props() {
    let diagnostics = check_source(
        "declare function Item(props: { label: string }): null;\n\
         declare const full: { label: string };\n\
         declare const partial: { id?: number };\n\
         const ok = <Item {...full} />;\n\
         const bad = <Item {...partial} />;\n",
        "example.tsx",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2741"]);
}

// Calling an imported generic with explicit type arguments re-resolves its
// declared parameter/return annotations; their names live in the declaring
// module's scope, not the caller's (react-hook-form's
// `useForm(props?: UseFormProps<…>): UseFormReturn<…>`), so the instantiation
// must run under the declaring file or the names report false TS2304s.
#[test]
fn imported_generic_call_resolves_signature_in_declaring_module_scope() {
    let diagnostics = program(&[
        (
            "lib.ts",
            "export interface Options<T> { seed: T }\n\
             export interface Box<T> { value: T }\n\
             export function make<T>(options?: Options<T>): Box<T> {\n\
                 return { value: (options as Options<T>).seed };\n\
             }\n",
        ),
        (
            "main.ts",
            "import { make } from \"./lib\";\n\
             const box = make<{ email: string }>();\n\
             const s: string = box.value.email;\n",
        ),
    ]);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

// An opaque spread (`any`, unresolved) folds the whole attributes object into
// `any` in tsc, so both the missing-required and excess checks stand down.
#[test]
fn jsx_opaque_spread_suppresses_presence_checks() {
    let diagnostics = check_source(
        "declare function Item(props: { label: string }): null;\n\
         declare const rest: any;\n\
         const ok = <Item {...rest} bonus={1} />;\n",
        "example.tsx",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

// `import * as z from "./external"; export { z }` (zod's index.d.ts shape)
// re-exports the namespace's type members, so a type-only named import of `z`
// can reference them as qualified names instead of reporting TS2305.
#[test]
fn namespace_import_reexport_exposes_member_types() {
    let diagnostics = program(&[
        (
            "external.ts",
            "export interface Payload { value: string }\n",
        ),
        (
            "barrel.ts",
            "import * as z from \"./external\";\nexport { z };\n",
        ),
        (
            "main.ts",
            "import type { z } from \"./barrel\";\n\
             const p: z.Payload = { value: \"ok\" };\n\
             export default p;\n",
        ),
    ]);
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
}

// A JSX expression types as ReactElement<any, any> in tsc, so it satisfies a
// structurally-declared element shape (the react-hook-form `render` callback
// return); an empty opaque stub would miss the required members.
#[test]
fn jsx_element_satisfies_react_element_shape() {
    let diagnostics = check_source(
        "declare function take(render: () => { type: string; props: unknown; key: string | null }): void;\n\
         take(() => <div />);\n",
        "example.tsx",
    );
    assert!(diagnostics.is_empty(), "{diagnostics:?}");
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
            resolved_modules_by_importer: Default::default(),
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

/// Regression coverage for the parallel check phase over script (non-module)
/// files. Every worker builds its per-file declaration table from the prebuilt
/// shared global+ambient table; rebuilding it inside workers previously
/// bump-allocated into the shared arena concurrently. With the arena freeze in
/// place, any reintroduced worker-side allocation panics deterministically,
/// so this test failing (or panicking) flags the race instead of silent UB.
#[test]
fn parallel_script_files_match_serial_diagnostics() {
    use surge_ts_checker::check_program_with_stats_and_jobs;

    let files: Vec<SourceFileInput> = (0..8)
        .map(|i| SourceFileInput {
            file_name: format!("script_{i}.ts"),
            source_text: format!(
                "const good_{i}: number = {i};\nconst bad_{i}: string = {i};\nfunction helper_{i}(x: number): number {{ return x + good_{i}; }}\nhelper_{i}(bad_{i});\n"
            ),
        })
        .collect();

    let serial = check_program_with_stats_and_jobs(files.clone(), CheckerOptions::default(), 1);
    let parallel = check_program_with_stats_and_jobs(files, CheckerOptions::default(), 8);

    assert!(
        !serial.diagnostics.is_empty(),
        "expected the script fixtures to produce diagnostics"
    );
    let render = |diags: &[surge_ts_diagnostics::Diagnostic]| {
        let mut rendered: Vec<String> = diags
            .iter()
            .map(|d| format!("{} {} {}", d.file_name, d.code, d.message))
            .collect();
        rendered.sort();
        rendered
    };
    assert_eq!(render(&serial.diagnostics), render(&parallel.diagnostics));
}

/// One fixture module per index: a generic interface exercised at several
/// instantiations, one deliberate TS2322, and a cross-file import chain so the
/// module binding/import paths (whose preliminary structures are dropped at
/// `preliminary_release`) are all live.
fn region_fixture_files(count: usize) -> Vec<SourceFileInput> {
    (0..count)
        .map(|i| {
            let import = if i == 0 {
                String::new()
            } else {
                format!("import {{ ok_{p} }} from \"./mod_{p}\";\n", p = i - 1)
            };
            let use_import = if i == 0 {
                String::new()
            } else {
                format!("export const chained_{i}: string = ok_{p}.value;\n", p = i - 1)
            };
            SourceFileInput {
                file_name: format!("mod_{i}.ts"),
                source_text: format!(
                    "{import}export interface RegionBox_{i}<T> {{ value: T; }}\n\
                     export type RegionPair_{i}<T> = {{ first: T; second: RegionBox_{i}<T> }};\n\
                     export const ok_{i}: RegionBox_{i}<string> = {{ value: \"ok\" }};\n\
                     export const bad_{i}: RegionBox_{i}<number> = {{ value: \"oops\" }};\n\
                     export function use_{i}(input: RegionPair_{i}<boolean>): boolean {{ return input.first; }}\n\
                     {use_import}"
                ),
            }
        })
        .collect()
}

fn rendered_sorted(diags: &[surge_ts_diagnostics::Diagnostic]) -> Vec<String> {
    let mut rendered: Vec<String> = diags
        .iter()
        .map(|d| format!("{} {} {:?} {}", d.file_name, d.code, d.span, d.message))
        .collect();
    rendered.sort();
    rendered
}

/// Region regression: a parallel worker context is reused across many files
/// (error files interleaved with clean ones), and `begin_file_check` resets the
/// file region between them. Any leak of one file's dedup keys or caches into
/// the next would make parallel output diverge from serial (which clones a
/// fresh context per file).
#[test]
fn parallel_worker_reuse_across_many_module_files_matches_serial() {
    use surge_ts_checker::check_program_with_stats_and_jobs;

    let files = region_fixture_files(24);
    let serial = check_program_with_stats_and_jobs(files.clone(), CheckerOptions::default(), 1);
    let parallel = check_program_with_stats_and_jobs(files, CheckerOptions::default(), 4);

    assert!(
        serial.diagnostics.len() >= 24,
        "expected one TS2322 per fixture file, got {}",
        serial.diagnostics.len()
    );
    assert_eq!(
        rendered_sorted(&serial.diagnostics),
        rendered_sorted(&parallel.diagnostics)
    );
}

/// The expected diagnostic surface of `region_fixture_files(6)`, asserted
/// identically by the default-cap and bounded-cap tests below: the generic
/// instantiation caches are recomputable memos, so any bucket cap must produce
/// byte-identical diagnostics (only time/memory may change).
fn assert_region_fixture_diagnostics(diags: &[surge_ts_diagnostics::Diagnostic]) {
    let ts2322: Vec<&surge_ts_diagnostics::Diagnostic> = diags
        .iter()
        .filter(|d| d.code.to_string() == "TS2322")
        .collect();
    assert_eq!(
        ts2322.len(),
        6,
        "expected exactly one TS2322 per fixture file: {:?}",
        rendered_sorted(diags)
    );
    for (i, diagnostic) in ts2322.iter().enumerate() {
        assert_eq!(diagnostic.file_name, format!("mod_{i}.ts"));
    }
}

#[test]
fn generic_cache_default_cap_expected_diagnostics() {
    let result = check_program(region_fixture_files(6));
    assert_region_fixture_diagnostics(&result);
}

/// Same fixture and same golden expectation as the default-cap test, but with
/// the per-declaration cache bucket cap forced to 1 (over-cap instantiations
/// recompute instead of caching). Also checks a second in-process run for
/// determinism under the bound. nextest runs each test in its own process, so
/// the env override cannot leak into other tests.
#[test]
fn generic_cache_bounded_cap_expected_diagnostics() {
    // Safety: set before any checker thread is spawned in this test process.
    unsafe { std::env::set_var("SURGE_GENERIC_CACHE_BUCKET_CAP", "1") };
    let first = check_program(region_fixture_files(6));
    assert_region_fixture_diagnostics(&first);
    let second = check_program(region_fixture_files(6));
    assert_eq!(rendered_sorted(&first), rendered_sorted(&second));
}

/// Nested distributive conditionals multiply union widths (20^5 = 3.2M branch
/// resolutions here). The per-root expansion budget must degrade the runaway
/// alias to `unknown` instead of hanging or exhausting memory; without it this
/// test does not terminate in any reasonable time.
#[test]
fn nested_distributive_conditional_blowup_degrades_instead_of_hanging() {
    let mut source = String::new();
    let members = (1..=20)
        .map(|index| index.to_string())
        .collect::<Vec<_>>()
        .join(" | ");
    source.push_str(&format!("type U = {members};\n"));
    source.push_str(
        "type Cross<A, B, C, D, E> = A extends any\n\
         ? B extends any\n\
         ? C extends any\n\
         ? D extends any\n\
         ? E extends any\n\
         ? [A, B, C, D, E]\n\
         : never : never : never : never : never;\n\
         type Boom = Cross<U, U, U, U, U>;\n\
         export const marker: number = 1;\n",
    );

    let diagnostics = check_source(&source, "blowup.ts");
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| !diagnostic.message.contains("marker")),
        "budget degradation must not produce diagnostics on unrelated code: {diagnostics:?}"
    );
}

