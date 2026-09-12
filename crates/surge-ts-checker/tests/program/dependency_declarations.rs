use surge_ts_checker::{
    CheckerOptions, SourceFileInput, check_program_with_options,
};

use super::*;

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

// Two instantiations of the same degraded alias resolve independently — a
// degraded resolution is never cached — and the unresolvable name is still
// reported exactly once. The declaration file is an ambient script (no
// top-level import/export): a *module* declaration file is module-scoped, so it
// is not lowered by the ambient-global passes at all.
#[test]
fn dependency_dts_degraded_resolution_not_cached() {
    let diagnostics = program(&[
        (
            "node_modules/dep/index.d.ts",
            "type Broken<T> = Missing<T>; declare const first: Broken<string>; declare const second: Broken<number>;",
        ),
        ("src/index.ts", "first.anything; second.anything;"),
    ]);
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

/// A dependency `.d.ts` resolves its own imported names when its declarations
/// are forced later (a lazy value annotation peels inside an environment
/// captured before the per-file scope map existed). Without the program-wide
/// scope fallback, `SVGProps` here missed and the annotation degraded.
#[test]
fn dependency_declaration_resolves_its_own_imports_when_forced_late() {
    let mut options = CheckerOptions::default();
    options
        .resolved_modules
        .insert("dep".to_string(), "node_modules/dep/index.d.ts".to_string());
    options.resolved_modules.insert(
        "shapes".to_string(),
        "node_modules/shapes/index.d.ts".to_string(),
    );
    let diagnostics = program_with_options(
        &[
            (
                "node_modules/shapes/index.d.ts",
                "export interface Shape { size: number }\n",
            ),
            (
                "node_modules/dep/index.d.ts",
                "import { Shape } from 'shapes';\n\
                 declare const widget: Shape;\n\
                 export { widget };\n",
            ),
            (
                "src/index.ts",
                "import { widget } from 'dep';\n\
                 export const ok: number = widget.size;\n\
                 export const bad: string = widget.size;\n",
            ),
        ],
        options,
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322"]);
}
