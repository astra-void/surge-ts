use super::*;

#[test]
fn parse_object_type_literal_accepts_commas_semicolons_and_trailing_separator() {
    let cases = [
        "let user: { name: string; age: number } = { name: \"Ada\", age: 36 };",
        "let user: { name: string, age: number } = { name: \"Ada\", age: 36 };",
        "let user: { name: string; age: number; } = { name: \"Ada\", age: 36 };",
    ];

    for source in cases {
        let parsed = parse_source(source, "example.ts");
        assert!(
            parsed.parser_errors.is_empty(),
            "unexpected parser errors for {source}"
        );
        assert_eq!(parsed.statements.len(), 1);

        let ParsedStatement::VariableDeclaration(variable) = &parsed.statements[0] else {
            panic!("expected a variable declaration");
        };

        let Some(ParsedType::Object(object_type)) = variable.declared_type.as_ref() else {
            panic!("expected an object type annotation");
        };

        assert_eq!(object_type.properties.len(), 2);
        assert_eq!(object_type.properties[0].name, "name");
        assert_eq!(object_type.properties[1].name, "age");
    }
}

#[test]
fn parse_object_type_literal_keeps_optional_property_metadata() {
    let parsed = parse_source("let user: { name?: string } = {};", "example.ts");
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::VariableDeclaration(variable) = &parsed.statements[0] else {
        panic!("expected a variable declaration");
    };

    let Some(ParsedType::Object(object_type)) = variable.declared_type.as_ref() else {
        panic!("expected an object type annotation");
    };

    assert_eq!(object_type.properties.len(), 1);
    assert!(object_type.properties[0].optional);
    assert_eq!(object_type.properties[0].name, "name");
}

#[test]
fn parse_object_type_literal_accepts_mixed_optional_separators() {
    let cases = [
        (
            "let user: { name?: string; age: number } = { age: 1 };",
            vec![("name", true), ("age", false)],
        ),
        (
            "let user: { name?: string, age?: number } = {};",
            vec![("name", true), ("age", true)],
        ),
        (
            "let user: { name?: string; age?: number; } = {};",
            vec![("name", true), ("age", true)],
        ),
    ];

    for (source, expected_properties) in cases {
        let parsed = parse_source(source, "example.ts");
        assert!(
            parsed.parser_errors.is_empty(),
            "unexpected parser errors for {source}"
        );
        assert_eq!(parsed.statements.len(), 1);

        let ParsedStatement::VariableDeclaration(variable) = &parsed.statements[0] else {
            panic!("expected a variable declaration");
        };

        let Some(ParsedType::Object(object_type)) = variable.declared_type.as_ref() else {
            panic!("expected an object type annotation");
        };

        assert_eq!(object_type.properties.len(), expected_properties.len());

        for ((expected_name, expected_optional), property) in expected_properties
            .into_iter()
            .zip(object_type.properties.iter())
        {
            assert_eq!(property.name, expected_name);
            assert_eq!(property.optional, expected_optional);
        }
    }
}

#[test]
fn parse_union_type_literal_keeps_constituents_and_metadata() {
    let parsed = parse_source(
        "let value: { name?: string } | undefined = { name: \"Ada\" };",
        "example.ts",
    );
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::VariableDeclaration(variable) = &parsed.statements[0] else {
        panic!("expected a variable declaration");
    };

    let Some(ParsedType::Union(types)) = variable.declared_type.as_ref() else {
        panic!("expected a union type annotation");
    };

    assert_eq!(types.len(), 2);
    assert!(matches!(types[0], ParsedType::Object(_)));
    assert!(matches!(types[1], ParsedType::Undefined));

    let ParsedType::Object(object_type) = &types[0] else {
        panic!("expected an object constituent");
    };
    assert!(object_type.properties[0].optional);
}

#[test]
fn parse_union_types_accepts_multiple_constituents() {
    let parsed = parse_source(
        "let value: string | number | undefined = \"ok\";",
        "example.ts",
    );
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::VariableDeclaration(variable) = &parsed.statements[0] else {
        panic!("expected a variable declaration");
    };

    let Some(ParsedType::Union(types)) = variable.declared_type.as_ref() else {
        panic!("expected a union type annotation");
    };

    assert_eq!(types.len(), 3);
    assert!(matches!(types[0], ParsedType::String));
    assert!(matches!(types[1], ParsedType::Number));
    assert!(matches!(types[2], ParsedType::Undefined));
}

#[test]
fn parse_string_literal_type() {
    let parsed = parse_source("let value: \"ok\" = \"ok\";", "example.ts");
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::VariableDeclaration(variable) = &parsed.statements[0] else {
        panic!("expected a variable declaration");
    };

    let Some(ParsedType::StringLiteral(value)) = variable.declared_type.as_ref() else {
        panic!("expected a string literal type");
    };

    assert_eq!(value, "ok");
}

#[test]
fn parse_single_quoted_string_literal_type() {
    let parsed = parse_source("let value: 'ok' = 'ok';", "example.ts");
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::VariableDeclaration(variable) = &parsed.statements[0] else {
        panic!("expected a variable declaration");
    };

    let Some(ParsedType::StringLiteral(value)) = variable.declared_type.as_ref() else {
        panic!("expected a string literal type");
    };

    assert_eq!(value, "ok");
}

#[test]
fn parse_number_literal_type() {
    let parsed = parse_source("let value: 1 = 1;", "example.ts");
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::VariableDeclaration(variable) = &parsed.statements[0] else {
        panic!("expected a variable declaration");
    };

    let Some(ParsedType::NumberLiteral(value)) = variable.declared_type.as_ref() else {
        panic!("expected a number literal type");
    };

    assert_eq!(value, "1");
}

#[test]
fn parse_decimal_number_literal_type() {
    let parsed = parse_source("let value: 1.5 = 1.5;", "example.ts");
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::VariableDeclaration(variable) = &parsed.statements[0] else {
        panic!("expected a variable declaration");
    };

    let Some(ParsedType::NumberLiteral(value)) = variable.declared_type.as_ref() else {
        panic!("expected a decimal number literal type");
    };

    assert_eq!(value, "1.5");
}

#[test]
fn parse_boolean_literal_type() {
    let parsed = parse_source("let value: true = true;", "example.ts");
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::VariableDeclaration(variable) = &parsed.statements[0] else {
        panic!("expected a variable declaration");
    };

    let Some(ParsedType::BooleanLiteral(value)) = variable.declared_type.as_ref() else {
        panic!("expected a boolean literal type");
    };

    assert!(*value);
}

#[test]
fn parse_boolean_false_literal_type() {
    let parsed = parse_source("let value: false = false;", "example.ts");
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::VariableDeclaration(variable) = &parsed.statements[0] else {
        panic!("expected a variable declaration");
    };

    let Some(ParsedType::BooleanLiteral(value)) = variable.declared_type.as_ref() else {
        panic!("expected a boolean literal type");
    };

    assert!(!*value);
}

#[test]
fn parse_literal_union_type() {
    let parsed = parse_source(
        "type Status = \"idle\" | \"loading\" | \"done\";",
        "example.ts",
    );
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::TypeAliasDeclaration(alias) = &parsed.statements[0] else {
        panic!("expected a type alias declaration");
    };

    let ParsedType::Union(types) = &alias.ty else {
        panic!("expected a union alias");
    };

    assert!(matches!(types[0], ParsedType::StringLiteral(_)));
    assert!(matches!(types[1], ParsedType::StringLiteral(_)));
    assert!(matches!(types[2], ParsedType::StringLiteral(_)));
}

#[test]
fn parse_void_type() {
    let parsed = parse_source("let value: void = undefined;", "example.ts");
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::VariableDeclaration(variable) = &parsed.statements[0] else {
        panic!("expected a variable declaration");
    };

    assert!(matches!(
        variable.declared_type.as_ref(),
        Some(ParsedType::Void)
    ));
}

#[test]
fn parse_function_type_no_params() {
    let parsed = parse_source("type Fn = () => string;", "example.ts");
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::TypeAliasDeclaration(alias) = &parsed.statements[0] else {
        panic!("expected a type alias declaration");
    };

    let ParsedType::Function(function_type) = &alias.ty else {
        panic!("expected a function type alias");
    };

    assert!(function_type.parameters.is_empty());
    assert!(matches!(&*function_type.return_type, ParsedType::String));
}

#[test]
fn parse_function_type_one_param() {
    let parsed = parse_source("type Fn = (value: string) => number;", "example.ts");
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::TypeAliasDeclaration(alias) = &parsed.statements[0] else {
        panic!("expected a type alias declaration");
    };

    let ParsedType::Function(function_type) = &alias.ty else {
        panic!("expected a function type alias");
    };

    assert_eq!(function_type.parameters.len(), 1);
    assert_eq!(function_type.parameters[0].name.as_deref(), Some("value"));
    assert!(matches!(function_type.parameters[0].ty, ParsedType::String));
    assert!(matches!(&*function_type.return_type, ParsedType::Number));
}

#[test]
fn parse_function_type_multiple_params() {
    let parsed = parse_source(
        "type Fn = (value: string, count: number) => boolean;",
        "example.ts",
    );
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::TypeAliasDeclaration(alias) = &parsed.statements[0] else {
        panic!("expected a type alias declaration");
    };

    let ParsedType::Function(function_type) = &alias.ty else {
        panic!("expected a function type alias");
    };

    assert_eq!(function_type.parameters.len(), 2);
    assert_eq!(function_type.parameters[0].name.as_deref(), Some("value"));
    assert_eq!(function_type.parameters[1].name.as_deref(), Some("count"));
    assert!(matches!(&*function_type.return_type, ParsedType::Boolean));
}

#[test]
fn parse_function_type_literal_param() {
    let parsed = parse_source("type Fn = (value: \"idle\") => void;", "example.ts");
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::TypeAliasDeclaration(alias) = &parsed.statements[0] else {
        panic!("expected a type alias declaration");
    };

    let ParsedType::Function(function_type) = &alias.ty else {
        panic!("expected a function type alias");
    };

    let ParsedType::StringLiteral(value) = &function_type.parameters[0].ty else {
        panic!("expected a string literal parameter type");
    };

    assert_eq!(value, "idle");
    assert!(matches!(&*function_type.return_type, ParsedType::Void));
}

#[test]
fn parse_function_type_union_param() {
    let parsed = parse_source(
        "type Fn = (value: \"idle\" | \"done\") => void;",
        "example.ts",
    );
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::TypeAliasDeclaration(alias) = &parsed.statements[0] else {
        panic!("expected a type alias declaration");
    };

    let ParsedType::Function(function_type) = &alias.ty else {
        panic!("expected a function type alias");
    };

    let ParsedType::Union(types) = &function_type.parameters[0].ty else {
        panic!("expected a union parameter type");
    };

    assert_eq!(types.len(), 2);
}

#[test]
fn parse_function_type_nested_param() {
    let parsed = parse_source("type Fn = (callback: () => string) => void;", "example.ts");
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::TypeAliasDeclaration(alias) = &parsed.statements[0] else {
        panic!("expected a type alias declaration");
    };

    let ParsedType::Function(function_type) = &alias.ty else {
        panic!("expected a function type alias");
    };

    let ParsedType::Function(callback_type) = &function_type.parameters[0].ty else {
        panic!("expected a nested function type parameter");
    };

    assert!(callback_type.parameters.is_empty());
    assert!(matches!(&*callback_type.return_type, ParsedType::String));
}

#[test]
fn parse_function_type_function_return() {
    let parsed = parse_source("type Fn = (value: string) => () => string;", "example.ts");
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::TypeAliasDeclaration(alias) = &parsed.statements[0] else {
        panic!("expected a type alias declaration");
    };

    let ParsedType::Function(function_type) = &alias.ty else {
        panic!("expected a function type alias");
    };

    let ParsedType::Function(return_type) = &*function_type.return_type else {
        panic!("expected a nested return function type");
    };

    assert!(return_type.parameters.is_empty());
    assert!(matches!(&*return_type.return_type, ParsedType::String));
}

#[test]
fn parse_function_type_in_type_alias() {
    let parsed = parse_source("type Mapper = (value: string) => number;", "example.ts");
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::TypeAliasDeclaration(alias) = &parsed.statements[0] else {
        panic!("expected a type alias declaration");
    };

    assert!(matches!(alias.ty, ParsedType::Function(_)));
}

#[test]
fn parse_function_type_in_object_property() {
    let parsed = parse_source(
        "let store: { getState: () => string } = { getState: getState };",
        "example.ts",
    );
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::VariableDeclaration(variable) = &parsed.statements[0] else {
        panic!("expected a variable declaration");
    };

    let ParsedType::Object(object_type) = variable.declared_type.as_ref().unwrap() else {
        panic!("expected an object type");
    };

    assert!(matches!(
        object_type.properties[0].ty,
        ParsedType::Function(_)
    ));
}

#[test]
fn parse_function_type_in_interface_member() {
    let parsed = parse_source("interface Store { getState: () => string; }", "example.ts");
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::InterfaceDeclaration(interface) = &parsed.statements[0] else {
        panic!("expected an interface declaration");
    };

    assert!(matches!(interface.members[0].ty, ParsedType::Function(_)));
}

#[test]
fn parse_function_type_malformed_no_panic() {
    let parsed = parse_source("type Fn = (value: string) => ;", "example.ts");

    assert_eq!(parsed.file_name, "example.ts");
}

#[test]
fn parse_function_type_missing_arrow_no_panic() {
    let parsed = parse_source("type Fn = (value: string) string;", "example.ts");

    assert_eq!(parsed.file_name, "example.ts");
}

#[test]
fn parse_function_type_missing_return_no_panic() {
    let parsed = parse_source("type Fn = (value: string) =>", "example.ts");

    assert_eq!(parsed.file_name, "example.ts");
}

#[test]
fn parse_function_type_missing_parameter_type_no_panic() {
    let parsed = parse_source("type Fn = (value) => string;", "example.ts");

    assert_eq!(parsed.file_name, "example.ts");
}

#[test]
fn parse_function_type_optional_param_unsupported_no_panic() {
    let parsed = parse_source("type Fn = (value?: string) => void;", "example.ts");

    assert_eq!(parsed.file_name, "example.ts");
}

#[test]
fn parse_function_type_rest_param_unsupported_no_panic() {
    let parsed = parse_source("type Fn = (...args: string[]) => void;", "example.ts");

    assert_eq!(parsed.file_name, "example.ts");
}

#[test]
fn parse_function_type_generic_unsupported_no_panic() {
    let parsed = parse_source("type Fn = <T>(value: T) => T;", "example.ts");

    assert_eq!(parsed.file_name, "example.ts");
}

#[test]
fn parse_function_type_default_param_unsupported_no_panic() {
    let parsed = parse_source("type Fn = (value = \"ok\") => string;", "example.ts");

    assert_eq!(parsed.file_name, "example.ts");
}

#[test]
fn parse_function_type_this_param_unsupported_no_panic() {
    let parsed = parse_source("type Fn = (this: string) => void;", "example.ts");

    assert_eq!(parsed.file_name, "example.ts");
}

#[test]
fn parse_function_type_call_signature_unsupported_no_panic() {
    let parsed = parse_source("type Fn = { (): string };", "example.ts");

    assert_eq!(parsed.file_name, "example.ts");
}

#[test]
fn parse_function_type_construct_signature_unsupported_no_panic() {
    let parsed = parse_source("type Fn = { new (): string };", "example.ts");

    assert_eq!(parsed.file_name, "example.ts");
}

#[test]
fn parse_function_type_parenthesized_non_function_type_no_panic() {
    let parsed = parse_source("type Fn = (string);", "example.ts");

    assert_eq!(parsed.file_name, "example.ts");

    let ParsedStatement::TypeAliasDeclaration(alias) = &parsed.statements[0] else {
        panic!("expected a type alias declaration");
    };

    assert!(matches!(alias.ty, ParsedType::String));
}

#[test]
fn parse_literal_type_alias() {
    let parsed = parse_source("type Status = \"idle\" | 1 | true;", "example.ts");
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::TypeAliasDeclaration(alias) = &parsed.statements[0] else {
        panic!("expected a type alias declaration");
    };

    let ParsedType::Union(types) = &alias.ty else {
        panic!("expected a union alias");
    };

    assert!(matches!(types[0], ParsedType::StringLiteral(_)));
    assert!(matches!(types[1], ParsedType::NumberLiteral(_)));
    assert!(matches!(types[2], ParsedType::BooleanLiteral(true)));
}

#[test]
fn parse_literal_object_property_type() {
    let parsed = parse_source(
        "let event: { kind: \"click\" } = { kind: \"click\" };",
        "example.ts",
    );
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::VariableDeclaration(variable) = &parsed.statements[0] else {
        panic!("expected a variable declaration");
    };

    let Some(ParsedType::Object(object_type)) = variable.declared_type.as_ref() else {
        panic!("expected an object type annotation");
    };

    let ParsedType::StringLiteral(value) = &object_type.properties[0].ty else {
        panic!("expected a string literal property type");
    };

    assert_eq!(value, "click");
}

#[test]
fn parse_literal_interface_property_type() {
    let parsed = parse_source("interface ClickEvent { kind: \"click\"; }", "example.ts");
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::InterfaceDeclaration(interface) = &parsed.statements[0] else {
        panic!("expected an interface declaration");
    };

    let ParsedType::StringLiteral(value) = &interface.members[0].ty else {
        panic!("expected a string literal member type");
    };

    assert_eq!(value, "click");
}

#[test]
fn parse_malformed_union_type_literal_does_not_panic() {
    let parsed = parse_source("let value: string | = \"ok\";", "example.ts");

    assert_eq!(parsed.file_name, "example.ts");
    assert!(!parsed.parser_errors.is_empty());
}

#[test]
fn parse_negative_literal_type_unsupported_no_panic() {
    let parsed = parse_source("type Value = -1;", "example.ts");

    assert_eq!(parsed.file_name, "example.ts");
}

#[test]
fn parse_template_literal_type_unsupported_no_panic() {
    let parsed = parse_source("type Value = `x`;", "example.ts");

    assert_eq!(parsed.file_name, "example.ts");
}

#[test]
fn parse_bigint_literal_type_unsupported_no_panic() {
    let parsed = parse_source("type Value = 1n;", "example.ts");

    assert_eq!(parsed.file_name, "example.ts");
}

#[test]
fn parse_undefined_literal_expression() {
    let parsed = parse_source("let value = undefined;", "example.ts");
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::VariableDeclaration(variable) = &parsed.statements[0] else {
        panic!("expected a variable declaration");
    };

    assert!(matches!(
        variable.initializer.as_ref(),
        Some(ParsedExpression::UndefinedLiteral)
    ));
}

#[test]
fn parse_type_alias_primitive() {
    let parsed = parse_source("type Name = string;", "example.ts");
    assert!(parsed.parser_errors.is_empty());
    assert_eq!(parsed.statements.len(), 1);

    let ParsedStatement::TypeAliasDeclaration(alias) = &parsed.statements[0] else {
        panic!("expected a type alias declaration");
    };

    assert_eq!(alias.name, "Name");
    assert!(matches!(alias.ty, ParsedType::String));
}

#[test]
fn parse_type_alias_object() {
    let parsed = parse_source("type User = { name: string; age?: number };", "example.ts");
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::TypeAliasDeclaration(alias) = &parsed.statements[0] else {
        panic!("expected a type alias declaration");
    };

    let ParsedType::Object(object_type) = &alias.ty else {
        panic!("expected an object alias");
    };

    assert_eq!(object_type.properties.len(), 2);
    assert_eq!(object_type.properties[0].name, "name");
    assert_eq!(object_type.properties[1].name, "age");
}

#[test]
fn parse_type_alias_union() {
    let parsed = parse_source("type MaybeName = string | undefined;", "example.ts");
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::TypeAliasDeclaration(alias) = &parsed.statements[0] else {
        panic!("expected a type alias declaration");
    };

    let ParsedType::Union(types) = &alias.ty else {
        panic!("expected a union alias");
    };

    assert_eq!(types.len(), 2);
    assert!(matches!(types[0], ParsedType::String));
    assert!(matches!(types[1], ParsedType::Undefined));
}

#[test]
fn parse_type_alias_named_reference() {
    let parsed = parse_source(
        "type Name = string; let value: Name = \"ok\";",
        "example.ts",
    );
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::VariableDeclaration(variable) = &parsed.statements[1] else {
        panic!("expected a variable declaration");
    };

    let Some(ParsedType::Named(named)) = variable.declared_type.as_ref() else {
        panic!("expected a named type reference");
    };

    assert_eq!(named.name, "Name");
}

#[test]
fn parse_type_alias_named_reference_inside_object_property() {
    let parsed = parse_source(
        "type Name = string; type User = { name: Name };",
        "example.ts",
    );
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::TypeAliasDeclaration(alias) = &parsed.statements[1] else {
        panic!("expected a type alias declaration");
    };

    let ParsedType::Object(object_type) = &alias.ty else {
        panic!("expected an object alias");
    };

    let ParsedType::Named(named) = &object_type.properties[0].ty else {
        panic!("expected a named property type");
    };

    assert_eq!(named.name, "Name");
}

#[test]
fn parse_type_alias_named_reference_inside_union() {
    let parsed = parse_source(
        "type Name = string; type MaybeName = Name | undefined;",
        "example.ts",
    );
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::TypeAliasDeclaration(alias) = &parsed.statements[1] else {
        panic!("expected a type alias declaration");
    };

    let ParsedType::Union(types) = &alias.ty else {
        panic!("expected a union alias");
    };

    assert!(matches!(types[0], ParsedType::Named(_)));
}

#[test]
fn parse_type_alias_generic_unsupported_no_panic() {
    let parsed = parse_source("type Box<T> = T;", "example.ts");
    assert_eq!(parsed.file_name, "example.ts");
}

#[test]
fn parse_type_alias_malformed_no_panic() {
    let parsed = parse_source("type Name = ;", "example.ts");
    assert_eq!(parsed.file_name, "example.ts");
}

#[test]
fn parse_interface_required_property() {
    let parsed = parse_source("interface User { name: string; }", "example.ts");
    assert!(parsed.parser_errors.is_empty());
    assert_eq!(parsed.statements.len(), 1);

    let ParsedStatement::InterfaceDeclaration(interface) = &parsed.statements[0] else {
        panic!("expected an interface declaration");
    };

    assert_eq!(interface.name, "User");
    assert_eq!(interface.members.len(), 1);
    assert_eq!(interface.members[0].name, "name");
    assert!(!interface.members[0].optional);
    assert!(matches!(interface.members[0].ty, ParsedType::String));
}

#[test]
fn parse_interface_optional_property() {
    let parsed = parse_source("interface User { name?: string; }", "example.ts");
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::InterfaceDeclaration(interface) = &parsed.statements[0] else {
        panic!("expected an interface declaration");
    };

    assert_eq!(interface.members.len(), 1);
    assert!(interface.members[0].optional);
}

#[test]
fn parse_interface_multiple_properties_semicolon() {
    let parsed = parse_source(
        "interface User { name: string; age: number; }",
        "example.ts",
    );
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::InterfaceDeclaration(interface) = &parsed.statements[0] else {
        panic!("expected an interface declaration");
    };

    assert_eq!(interface.members.len(), 2);
    assert_eq!(interface.members[0].name, "name");
    assert_eq!(interface.members[1].name, "age");
}

#[test]
fn parse_interface_multiple_properties_comma() {
    let parsed = parse_source("interface User { name: string, age: number }", "example.ts");
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::InterfaceDeclaration(interface) = &parsed.statements[0] else {
        panic!("expected an interface declaration");
    };

    assert_eq!(interface.members.len(), 2);
}

#[test]
fn parse_interface_multiple_properties_newline_or_current_separator() {
    let parsed = parse_source(
        "interface User {\n  name: string\n  age: number\n}",
        "example.ts",
    );
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::InterfaceDeclaration(interface) = &parsed.statements[0] else {
        panic!("expected an interface declaration");
    };

    assert_eq!(interface.members.len(), 2);
}

#[test]
fn parse_interface_property_union_type() {
    let parsed = parse_source("interface User { name: string | undefined; }", "example.ts");
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::InterfaceDeclaration(interface) = &parsed.statements[0] else {
        panic!("expected an interface declaration");
    };

    assert!(matches!(interface.members[0].ty, ParsedType::Union(_)));
}

#[test]
fn parse_interface_property_named_type() {
    let parsed = parse_source(
        "type Name = string; interface User { name: Name; }",
        "example.ts",
    );
    assert!(parsed.parser_errors.is_empty());

    let ParsedStatement::InterfaceDeclaration(interface) = &parsed.statements[1] else {
        panic!("expected an interface declaration");
    };

    assert!(matches!(interface.members[0].ty, ParsedType::Named(_)));
}

#[test]
fn parse_interface_top_level_statement_order() {
    let parsed = parse_source(
        "let user: User = { name: \"Ada\" }; interface User { name: string; }",
        "example.ts",
    );
    assert!(parsed.parser_errors.is_empty());
    assert!(matches!(
        parsed.statements[0],
        ParsedStatement::VariableDeclaration(_)
    ));
    assert!(matches!(
        parsed.statements[1],
        ParsedStatement::InterfaceDeclaration(_)
    ));
}

#[test]
fn parse_interface_malformed_no_panic() {
    let parsed = parse_source("interface User { name }", "example.ts");
    assert_eq!(parsed.file_name, "example.ts");
}

#[test]
fn parse_interface_extends_unsupported_no_panic() {
    let parsed = parse_source(
        "interface User extends Base { name: string; }",
        "example.ts",
    );
    assert_eq!(parsed.file_name, "example.ts");
}

#[test]
fn parse_interface_generic_unsupported_no_panic() {
    let parsed = parse_source("interface Box<T> { value: T; }", "example.ts");
    assert_eq!(parsed.file_name, "example.ts");
    assert_eq!(parsed.statements.len(), 1);
}

#[test]
fn parse_interface_method_unsupported_no_panic() {
    let parsed = parse_source(
        "interface User { greet(): string; name: string; }",
        "example.ts",
    );
    assert_eq!(parsed.file_name, "example.ts");

    let ParsedStatement::InterfaceDeclaration(interface) = &parsed.statements[0] else {
        panic!("expected an interface declaration");
    };

    assert_eq!(interface.members.len(), 1);
    assert_eq!(interface.members[0].name, "name");
}

#[test]
fn parse_interface_call_signature_unsupported_no_panic() {
    let parsed = parse_source("interface Callable { (): string; }", "example.ts");
    assert_eq!(parsed.file_name, "example.ts");
}

#[test]
fn parse_interface_index_signature_unsupported_no_panic() {
    let parsed = parse_source(
        "interface MapLike { [key: string]: number; value: number; }",
        "example.ts",
    );
    assert_eq!(parsed.file_name, "example.ts");
}

#[test]
fn parse_interface_readonly_property_unsupported_no_panic() {
    let parsed = parse_source("interface User { readonly name: string; }", "example.ts");
    assert_eq!(parsed.file_name, "example.ts");
}

#[test]
fn parse_interface_missing_body_no_panic() {
    let parsed = parse_source("interface User", "example.ts");
    assert_eq!(parsed.file_name, "example.ts");
}

#[test]
fn parse_interface_missing_member_type_no_panic() {
    let parsed = parse_source("interface User { name; }", "example.ts");
    assert_eq!(parsed.file_name, "example.ts");
}

#[test]
fn parse_interface_malformed_member_no_panic() {
    let parsed = parse_source("interface User { name: ; }", "example.ts");
    assert_eq!(parsed.file_name, "example.ts");
}
