use surge_ts_checker::{
    CheckerOptions, SourceFileInput, check_program_with_options, check_source_with_options,
};

use super::*;

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

    // `export type { Foo }` over a value-only export is legal and republishes
    // the symbol, so the re-export itself is not TS2305. Using the value as a
    // type is the error; tsc 7.0.2 reports TS2749 there, which surge does not
    // implement yet and reports as TS2304.
    assert_eq!(codes(&diagnostics), vec!["TS2304"]);
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
