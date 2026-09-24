use surge_ts_checker::{
    CheckerOptions,
    check_source,
};

use super::*;

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
            resolve_json_module: true,
            allow_js: false,
            check_js: None,
            jsx_configured: false,
            jsx_factory_names: Default::default(),
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
            resolve_json_module: true,
            allow_js: false,
            check_js: None,
            jsx_configured: false,
            jsx_factory_names: Default::default(),
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
            resolve_json_module: true,
            allow_js: false,
            check_js: None,
            jsx_configured: false,
            jsx_factory_names: Default::default(),
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
fn ambient_module_default_import_is_synthetic() {
    let diagnostics = program(&[
        ("src/index.ts", "import value from \"pkg-default\";"),
        (
            "types/pkg-default.d.ts",
            "declare module \"pkg-default\" { export const value: string; }",
        ),
    ]);

    // tsc: a declaration file's module can always be imported through a
    // synthetic default.
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
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
            resolve_json_module: true,
            allow_js: false,
            check_js: None,
            jsx_configured: false,
            jsx_factory_names: Default::default(),
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
            resolve_json_module: true,
            allow_js: false,
            check_js: None,
            jsx_configured: false,
            jsx_factory_names: Default::default(),
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
            resolve_json_module: true,
            allow_js: false,
            check_js: None,
            jsx_configured: false,
            jsx_factory_names: Default::default(),
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
fn ambient_module_duplicate_default_export_reports_both() {
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

    assert_eq!(
        codes(&diagnostics),
        vec!["TS2528", "TS2714", "TS2528", "TS2714"]
    );
}

#[test]
fn ambient_module_duplicate_type_export_conflict_reports_ts2717() {
    // Reopened ambient module blocks merge their exported interfaces; the
    // conflicting redeclaration of `name` is TS2717.
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

    assert_eq!(codes(&diagnostics), vec!["TS2717"]);
}

#[test]
fn ambient_global_duplicate_const_reports_both() {
    let diagnostics = program(&[
        ("src/index.ts", "let ok: string = value;"),
        ("types/a.d.ts", "declare const value: string;"),
        ("types/b.d.ts", "declare const value: number;"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2451", "TS2451"]);
    assert_eq!(file_names(&diagnostics), vec!["types/a.d.ts", "types/b.d.ts"]);
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
fn intersection_distributes_a_union_operand_over_the_object_merge() {
    // `(A | B) & C` is `(A & C) | (B & C)`. The object merge only reads
    // `Type::Object` operands, so an undistributed union contributed nothing and
    // every member of A/B was reported as an excess property.
    let source = concat!(
        "type U = { data: number; error: undefined } | { data: undefined; error: string };\n",
        "type I = { request: string };\n",
        "export const c: U & I = { data: undefined, error: 'x', request: 'r' };\n",
    );
    let diagnostics = check_source(source, "a.ts");
    assert!(
        codes(&diagnostics).is_empty(),
        "got {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn intersection_with_a_wide_union_operand_stays_open_instead_of_distributing() {
    // Past the distribution arity bound the merge is kept single, but the surface
    // must stay open — a closed merge would report every union-arm member as an
    // excess property.
    let source = concat!(
        "type W = { a: 1 } | { b: 1 } | { c: 1 } | { d: 1 } | { e: 1 }\n",
        "  | { f: 1 } | { g: 1 } | { h: 1 } | { i: 1 } | { j: 1 };\n",
        "type I = { request: string };\n",
        "export const w: W & I = { a: 1, request: 'r' };\n",
    );
    let diagnostics = check_source(source, "a.ts");
    assert!(
        codes(&diagnostics).is_empty(),
        "got {:?}",
        codes(&diagnostics)
    );
}

#[test]
fn intersection_distribution_keeps_the_missing_required_property_report() {
    // Distribution must not silently widen the target: every arm still requires
    // the non-union operand's members.
    let source = concat!(
        "type U = { data: number } | { error: string };\n",
        "type I = { request: string };\n",
        "export const c: U & I = { data: 1 };\n",
    );
    let diagnostics = check_source(source, "a.ts");
    assert_eq!(codes(&diagnostics), vec!["TS2322"], "{:?}", diagnostics);
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
fn ambient_module_namespace_class_export_assignment_is_importable() {
    let diagnostics = program(&[
        (
            "ambient.d.ts",
            "declare module \"mystream\" {\n  namespace Stream {\n    class Readable { ended: boolean }\n  }\n  export = Stream;\n}",
        ),
        (
            "index.ts",
            "import type { Readable } from \"mystream\";\nlet value: Readable = 1 as any;",
        ),
    ]);

    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn ambient_module_block_import_resolves_in_function_signature() {
    // The imported block sits in a later file, so it is not registered yet
    // when the importing block's signatures are first seen.
    let diagnostics = program(&[
        (
            "types/fsp.d.ts",
            "declare module \"m:fsp\" {\n    import { PathLike } from \"m:fs\";\n    function access(path: PathLike): void;\n}",
        ),
        (
            "types/fs.d.ts",
            "declare module \"m:fs\" {\n    type PathLike = string;\n}",
        ),
        (
            "src/index.ts",
            "import { access } from \"m:fsp\";\naccess(\"a\");\naccess(1);",
        ),
    ]);
    assert_eq!(codes(&diagnostics), vec!["TS2345"]);
    assert_eq!(diagnostics[0].file_name, "src/index.ts");
}

#[test]
fn ambient_module_block_unresolved_signature_name_still_reports() {
    let diagnostics = program(&[
        (
            "types/fsp.d.ts",
            "declare module \"m:fsp\" {\n    import { PathLike } from \"m:fs\";\n    function access(path: PathLike): void;\n    function missing(path: NotDeclared): void;\n}",
        ),
        (
            "types/fs.d.ts",
            "declare module \"m:fs\" {\n    type PathLike = string;\n}",
        ),
        ("src/index.ts", "export {};"),
    ]);
    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
    assert!(diagnostics[0].message.contains("NotDeclared"));
}
