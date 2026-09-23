use surge_ts_checker::{
    CheckerOptions, DiagnosticProfile, SourceFileInput, check_program,
    check_source, check_source_with_options,
};

use super::*;

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

// A default-exported promise-returning function is collapsed to its awaited
// value like every other `Promise<T>`, so a member read on the awaited result
// resolves instead of landing on a synthetic promise stand-in.
#[test]
fn program_default_exported_promise_collapses_to_its_value() {
    let mut options = CheckerOptions::default();
    options.resolved_modules.insert(
        "lib".to_string(),
        "node_modules/lib/index.d.ts".to_string(),
    );

    let diagnostics = program_with_options(
        &[
            (
                "node_modules/lib/index.d.ts",
                "export interface Res { ok: boolean; status: number }\n\
                 export default function grab(url: string): Promise<Res>;",
            ),
            (
                "src/index.ts",
                "import grab from 'lib';\n\
                 export async function read() {\n\
                     const res = await grab('u');\n\
                     return res.status;\n\
                 }\n\
                 export function chain() {\n\
                     return grab('u').then((res) => res.ok);\n\
                 }",
            ),
        ],
        options,
    );

    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
}

// The collapse must not invent members: a name the awaited value does not
// declare still reports.
#[test]
fn program_default_exported_promise_value_still_reports_a_missing_member() {
    let mut options = CheckerOptions::default();
    options.resolved_modules.insert(
        "lib".to_string(),
        "node_modules/lib/index.d.ts".to_string(),
    );

    let diagnostics = program_with_options(
        &[
            (
                "node_modules/lib/index.d.ts",
                "export interface Res { ok: boolean }\n\
                 export default function grab(url: string): Promise<Res>;",
            ),
            (
                "src/index.ts",
                "import grab from 'lib';\n\
                 export async function read() {\n\
                     const res = await grab('u');\n\
                     return res.missing;\n\
                 }",
            ),
        ],
        options,
    );

    assert_eq!(codes(&diagnostics), vec!["TS2339"], "{diagnostics:#?}");
}

// A `.json` module's exports come from the value it holds: the whole value as
// the default export and one named export per top-level property, both widened
// the way tsc widens them.
#[test]
fn json_module_exports_its_value() {
    let files = &[
        (
            "data.json",
            "{ \"version\": \"1.2.3\", \"retries\": 2, \"nested\": { \"on\": true } }",
        ),
        (
            "example.ts",
            "import info, { version, retries } from \"./data.json\";\n\
             export const v: string = version;\n\
             export const r: number = retries;\n\
             export const n: boolean = info.nested.on;\n",
        ),
    ];
    assert!(program(files).is_empty(), "{:?}", codes(&program(files)));
}

#[test]
fn json_module_value_type_is_checked() {
    let files = &[
        ("data.json", "{ \"version\": \"1.2.3\" }"),
        (
            "example.ts",
            "import { version } from \"./data.json\";\n\
             export const v: number = version;\n",
        ),
    ];
    assert_eq!(codes(&program(files)), vec!["TS2322"]);
}

// A `.json` file that does not parse is still a module. Reporting its importer
// as unresolved would be a worse answer than an unmodelled value, and surge
// does not report JSON syntax errors, so the value degrades and nothing
// cascades. tsc reports the syntax error on the JSON file itself.
#[test]
fn malformed_json_module_degrades_without_cascading() {
    let files = &[
        ("data.json", "{ 'version': 1 }"),
        (
            "example.ts",
            "import info, { version } from \"./data.json\";\n\
             export const v: number = version;\n\
             export const anything = info.whatever;\n",
        ),
    ];
    assert_eq!(codes(&program(files)), vec!["TS1327"]);
}

#[test]
fn program_api_no_lib_hides_generated_default_libs() {
    let diagnostics = program_with_options(
        &[(
            "example.ts",
            "const transport: AuthenticatorTransport = \"usb\"; const n = Math.max(1, 2);",
        )],
        CheckerOptions {
            use_unknown_in_catch_variables: false,
            diagnostic_profile: Default::default(),
            resolve_json_module: true,
            allow_js: false,
            jsx_configured: false,
            jsx_factory_names: Default::default(),
            resolved_modules: Default::default(),
            resolved_modules_by_importer: Default::default(),
            stub_external_modules: false,
            no_implicit_any: false,
            no_implicit_this: false,
            module_emit: Default::default(),
            use_define_for_class_fields: true,
            node_module_resolution: false,
            esm_module_files: Default::default(),
            strict_null_checks: true,
            strict_property_initialization: false,
            no_implicit_returns: false,
            no_fallthrough_cases_in_switch: false,
            no_implicit_override: false,
            no_property_access_from_index_signature: false,
            no_unchecked_indexed_access: false,
            allow_importing_ts_extensions: false,
            allow_arbitrary_extensions: false,
            no_unused_locals: false,
            no_unused_parameters: false,
            allow_unreachable_code: false,
            no_lib: true,
            skip_lib_check: false,
            jsx_automatic_runtime: false,
            jsx_classic_react: false,
            allow_umd_global_access: false,
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

    assert_eq!(codes(&diagnostics), vec!["TS2300", "TS2300"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts", "b.ts"]);
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

    assert_eq!(codes(&diagnostics), vec!["TS2300", "TS2300"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts", "b.ts"]);
}

#[test]
fn program_duplicate_interface_alias_across_files_ts2300() {
    let diagnostics = program(&[
        ("a.ts", "interface User { name: string; }"),
        ("b.ts", "type User = { name: string };"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2300", "TS2300"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts", "b.ts"]);
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

    assert_eq!(codes(&diagnostics), vec!["TS2393", "TS2393"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts", "b.ts"]);
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
fn program_top_level_let_is_shared_across_scripts() {
    let diagnostics = program(&[
        ("a.ts", "let greeting = \"Ada\";"),
        ("b.ts", "let value: string = greeting;"),
    ]);

    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn program_top_level_const_is_shared_across_scripts() {
    let diagnostics = program(&[
        ("a.ts", "const greeting = \"Ada\";"),
        ("b.ts", "let value: string = greeting;"),
    ]);

    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn program_script_variable_is_visible_in_another_scripts_function() {
    let diagnostics = program(&[
        ("a.ts", "let greeting = \"Ada\";"),
        ("b.ts", "function f(): string { return greeting; }"),
    ]);

    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
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
            resolve_json_module: true,
            allow_js: false,
            jsx_configured: false,
            jsx_factory_names: Default::default(),
        no_lib: false,
        skip_lib_check: false,
        jsx_automatic_runtime: false,
        jsx_classic_react: false,
        allow_umd_global_access: false,
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
            resolve_json_module: true,
            allow_js: false,
            jsx_configured: false,
            jsx_factory_names: Default::default(),
        no_lib: true,
        skip_lib_check: false,
        jsx_automatic_runtime: false,
        jsx_classic_react: false,
        allow_umd_global_access: false,
        types: Vec::new(),
        ..Default::default()
    };

    let diagnostics = check_source_with_options(source, "test.ts", options);
    assert_eq!(codes(&diagnostics), vec!["TS2584", "TS2304", "TS2304"]);
}

#[test]
fn program_api_single_file_no_implicit_any_matches_check_source_with_options() {
    let source = "function f(value): string { return \"ok\"; }";
    let program_diagnostics = program_with_options(
        &[("example.ts", source)],
        CheckerOptions {
            use_unknown_in_catch_variables: false,
            diagnostic_profile: Default::default(),
            resolve_json_module: true,
            allow_js: false,
            jsx_configured: false,
            jsx_factory_names: Default::default(),
            resolved_modules: Default::default(),
            resolved_modules_by_importer: Default::default(),
            stub_external_modules: false,
            no_implicit_any: true,
            no_implicit_this: true,
            module_emit: Default::default(),
            use_define_for_class_fields: true,
            node_module_resolution: false,
            esm_module_files: Default::default(),
            strict_null_checks: true,
            strict_property_initialization: false,
            no_implicit_returns: false,
            no_fallthrough_cases_in_switch: false,
            no_implicit_override: false,
            no_property_access_from_index_signature: false,
            no_unchecked_indexed_access: false,
            allow_importing_ts_extensions: false,
            allow_arbitrary_extensions: false,
            no_unused_locals: false,
            no_unused_parameters: false,
            allow_unreachable_code: false,
            no_lib: false,
            skip_lib_check: false,
            jsx_automatic_runtime: false,
            jsx_classic_react: false,
            allow_umd_global_access: false,
            types: Vec::new(),
        },
    );
    let single_file_diagnostics = check_source_with_options(
        source,
        "example.ts",
        CheckerOptions {
            use_unknown_in_catch_variables: false,
            diagnostic_profile: Default::default(),
            resolve_json_module: true,
            allow_js: false,
            jsx_configured: false,
            jsx_factory_names: Default::default(),
            resolved_modules: Default::default(),
            resolved_modules_by_importer: Default::default(),
            stub_external_modules: false,
            no_implicit_any: true,
            no_implicit_this: true,
            module_emit: Default::default(),
            use_define_for_class_fields: true,
            node_module_resolution: false,
            esm_module_files: Default::default(),
            strict_null_checks: true,
            strict_property_initialization: false,
            no_implicit_returns: false,
            no_fallthrough_cases_in_switch: false,
            no_implicit_override: false,
            no_property_access_from_index_signature: false,
            no_unchecked_indexed_access: false,
            allow_importing_ts_extensions: false,
            allow_arbitrary_extensions: false,
            no_unused_locals: false,
            no_unused_parameters: false,
            allow_unreachable_code: false,
            no_lib: false,
            skip_lib_check: false,
            jsx_automatic_runtime: false,
            jsx_classic_react: false,
            allow_umd_global_access: false,
            types: Vec::new(),
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
            use_unknown_in_catch_variables: false,
            diagnostic_profile: DiagnosticProfile::Native,
            resolve_json_module: true,
            allow_js: false,
            jsx_configured: false,
            jsx_factory_names: Default::default(),
            resolved_modules: Default::default(),
            resolved_modules_by_importer: Default::default(),
            stub_external_modules: false,
            no_implicit_any: true,
            no_implicit_this: true,
            module_emit: Default::default(),
            use_define_for_class_fields: true,
            node_module_resolution: false,
            esm_module_files: Default::default(),
            strict_null_checks: true,
            strict_property_initialization: false,
            no_implicit_returns: false,
            no_fallthrough_cases_in_switch: false,
            no_implicit_override: false,
            no_property_access_from_index_signature: false,
            no_unchecked_indexed_access: false,
            allow_importing_ts_extensions: false,
            allow_arbitrary_extensions: false,
            no_unused_locals: false,
            no_unused_parameters: false,
            allow_unreachable_code: false,
            no_lib: false,
            skip_lib_check: false,
            jsx_automatic_runtime: false,
            jsx_classic_react: false,
            allow_umd_global_access: false,
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
        ("a.ts", "let first: number = \"a\";"),
        ("b.ts", "let second: number = \"b\";"),
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

    assert_eq!(codes(&diagnostics), vec!["TS2393", "TS2393", "TS2322"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts", "b.ts", "b.ts"]);
}

#[test]
fn program_function_second_body_checked_against_own_signature() {
    let diagnostics = program(&[
        ("a.ts", "function getValue(): string { return \"Ada\"; }"),
        ("b.ts", "function getValue(): number { return \"Ada\"; }"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2393", "TS2393", "TS2322"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts", "b.ts", "b.ts"]);
}

#[test]
fn program_duplicate_function_call_site_is_not_reported() {
    let diagnostics = program(&[
        ("a.ts", "function getValue(): string { return \"Ada\"; }"),
        ("b.ts", "function getValue(): number { return \"Ada\"; }"),
        ("c.ts", "let value: number = getValue();"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2393", "TS2393", "TS2322"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts", "b.ts", "b.ts"]);
}

#[test]
fn program_top_level_variable_is_shared_across_scripts() {
    let diagnostics = program(&[
        ("a.ts", "let greeting = \"Ada\";"),
        ("b.ts", "let value: string = greeting;"),
    ]);

    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
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
        "let userName = \"Ada\"; function f(): string { return userName; }",
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
fn script_function_signature_typeof_reads_a_later_script_variable() {
    let diagnostics = program(&[
        (
            "a.ts",
            "function f(x: typeof later): void;\nfunction f(x: any) { }\nf({ foo: \"\" });\nf({ foo: 1 });\nfunction g(x: typeof nowhere) { }",
        ),
        ("b.ts", "var later: { foo: string } = { foo: \"\" };"),
    ]);
    let mut found = codes(&diagnostics);
    found.sort();
    assert_eq!(found, vec!["TS2304", "TS2322"]);
    assert!(
        diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code.to_string() == "TS2304" && diagnostic.message.contains("nowhere"))
    );
}
