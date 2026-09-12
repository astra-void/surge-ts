
use super::*;

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
fn declaration_file_enum_is_supported() {
    // Enums lower to a member-literal union plus an ambient value binding; an
    // empty one is inert, matching tsc which reports nothing here.
    let diagnostics = native_program(&[("types/globals.d.ts", "declare enum E {}")]);
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
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
fn declaration_file_wildcard_module_matches_importers() {
    // A wildcard pattern resolves any specifier it matches, so the CSS/asset
    // declarations app frameworks ship (`declare module "*.css"`) stop the
    // importing side from reporting an unresolved module.
    let diagnostics = native_program(&[
        (
            "types/globals.d.ts",
            "declare module \"*.css\" { const c: string; export default c; }",
        ),
        ("index.ts", "import \"./app.css\";\nexport const x = 1;"),
    ]);
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}
