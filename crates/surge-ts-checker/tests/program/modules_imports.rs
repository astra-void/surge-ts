use surge_ts_checker::{
    CheckerOptions,
    check_source,
};

use super::*;

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
fn program_module_file_sees_script_global() {
    let diagnostics = program(&[
        ("a.ts", "interface User { name: string; }"),
        ("b.ts", "export {};\nlet user: User = { name: \"Ada\" };"),
    ]);

    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn program_module_file_sees_script_type_alias() {
    let diagnostics = program(&[
        ("a.ts", "type Name = string;"),
        ("b.ts", "export {};\nlet value: Name = \"Ada\";"),
    ]);

    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn program_module_file_sees_script_function() {
    let diagnostics = program(&[
        ("a.ts", "function getName(): string { return \"Ada\"; }"),
        ("b.ts", "export {};\nlet value: string = getName();"),
    ]);

    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn program_empty_export_module_still_sees_script_globals() {
    let diagnostics = program(&[
        ("a.ts", "interface User { name: string; }"),
        ("b.ts", "export {};\nlet user: User = { name: \"Ada\" };"),
    ]);

    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn program_side_effect_import_module_still_sees_script_globals() {
    let diagnostics = program(&[
        ("a.ts", "interface User { name: string; }"),
        ("setup.ts", "export {};"),
        (
            "b.ts",
            "import \"./setup\";\nlet user: User = { name: \"Ada\" };",
        ),
    ]);

    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
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
            use_unknown_in_catch_variables: false,
            diagnostic_profile: Default::default(),
            resolve_json_module: true,
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
            no_unused_locals: false,
            no_unused_parameters: false,
            allow_unreachable_code: false,
            report_unreachable_code: false,
            no_lib: false,
            skip_lib_check: false,
            jsx_automatic_runtime: false,
            jsx_classic_react: false,
            jsx_emit_none: false,
            allow_umd_global_access: false,
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
            use_unknown_in_catch_variables: false,
            diagnostic_profile: Default::default(),
            resolve_json_module: true,
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
            no_unused_locals: false,
            no_unused_parameters: false,
            allow_unreachable_code: false,
            report_unreachable_code: false,
            no_lib: false,
            skip_lib_check: false,
            jsx_automatic_runtime: false,
            jsx_classic_react: false,
            jsx_emit_none: false,
            allow_umd_global_access: false,
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
            use_unknown_in_catch_variables: false,
            diagnostic_profile: Default::default(),
            resolve_json_module: true,
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
            no_unused_locals: false,
            no_unused_parameters: false,
            allow_unreachable_code: false,
            report_unreachable_code: false,
            no_lib: false,
            skip_lib_check: false,
            jsx_automatic_runtime: false,
            jsx_classic_react: false,
            jsx_emit_none: false,
            allow_umd_global_access: false,
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

    assert_eq!(codes(&diagnostics), vec!["TS2632"]);
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

    assert_eq!(codes(&diagnostics), vec!["TS2749"]);
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

    assert_eq!(codes(&diagnostics), vec!["TS2749"]);
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

    assert_eq!(
        codes(&diagnostics),
        vec!["TS2323", "TS2393", "TS2323", "TS2393"]
    );
    assert_eq!(file_names(&diagnostics), vec!["a.ts", "a.ts", "a.ts", "a.ts"]);
}

#[test]
fn program_module_duplicate_type_alias_same_module_file_ts2300() {
    let diagnostics = program(&[(
        "a.ts",
        "export {};\ntype Name = string;\ntype Name = number;\nlet value: Name = \"Ada\";",
    )]);

    assert_eq!(codes(&diagnostics), vec!["TS2300", "TS2300"]);
    assert_eq!(file_names(&diagnostics), vec!["a.ts", "a.ts"]);
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

    assert_eq!(codes(&diagnostics), vec!["TS2613"]);
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

    assert_eq!(codes(&diagnostics), vec!["TS2749"]);
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

    assert_eq!(codes(&diagnostics), vec!["TS2307"]);
}

#[test]
fn module_default_import_no_cascade_value_usage() {
    let diagnostics = program(&[
        ("user.ts", "export const getName: string = \"Ada\";"),
        ("index.ts", "import getName from \"./user\";\ngetName;"),
    ]);

    assert_eq!(codes(&diagnostics), vec!["TS2613"]);
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

/// An `export =` namespace imported under a different alias binds its members
/// under that alias (`react.Forward` for `React.Forward`). The interface's own
/// `extends` clause names its siblings unqualified, so the sibling lookup has to
/// use the *declared* namespace prefix, not the alias — resolving it under the
/// alias missed the base and silently dropped its call signature (lucide's
/// `import * as react from "react"` view of `ForwardRefExoticComponent`).
#[test]
fn renamed_namespace_import_resolves_interface_heritage_siblings() {
    let diagnostics = program(&[
        (
            "core.d.ts",
            "declare namespace React {\n\
                 interface ExoticComponent { (props: number): string }\n\
                 interface Forward extends ExoticComponent { tag?: string }\n\
             }\n\
             export = React;\n",
        ),
        (
            "index.ts",
            "import * as react from \"./core\";\n\
             declare const c: react.Forward;\n\
             export const r: string = c(1);\n",
        ),
    ]);

    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
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
fn module_default_import_binds_class_type() {
    let diagnostics = program(&[
        (
            "dispatcher.ts",
            "declare class Dispatcher { id: string }\nexport default Dispatcher;",
        ),
        (
            "index.ts",
            "import Dispatcher from \"./dispatcher\";\nlet value: Dispatcher = 1 as any;",
        ),
    ]);

    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn module_default_import_re_export_carries_type() {
    let diagnostics = program(&[
        (
            "dispatcher.ts",
            "declare class Dispatcher { id: string }\nexport default Dispatcher;",
        ),
        (
            "mid.ts",
            "import Dispatcher from \"./dispatcher\";\nexport { Dispatcher };",
        ),
        (
            "index.ts",
            "import type { Dispatcher } from \"./mid\";\nlet value: Dispatcher = 1 as any;",
        ),
    ]);

    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}
