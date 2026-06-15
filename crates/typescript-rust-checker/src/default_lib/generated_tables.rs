use std::collections::BTreeMap;
use std::sync::OnceLock;

use typescript_rust_syntax::{
    ParsedConditionalType, ParsedFunctionType, ParsedFunctionTypeParameter, ParsedInterfaceMember,
    ParsedNamedType, ParsedType, ParsedTypeParameter,
};
use typescript_rust_types::{FunctionType, ObjectProperty, ObjectType, Type};

use crate::symbols::{
    FunctionSignatureInfo, SymbolInfo, SymbolKind, SymbolTable, TypeAliasInfo, TypeDeclarationInfo,
    TypeDeclarationTable,
};

use super::registry::GeneratedDefaultLibSnapshot;

pub(crate) fn core_snapshot() -> &'static GeneratedDefaultLibSnapshot {
    static SNAPSHOT: OnceLock<GeneratedDefaultLibSnapshot> = OnceLock::new();
    SNAPSHOT.get_or_init(build_core_snapshot)
}

pub(crate) fn dom_snapshot() -> &'static GeneratedDefaultLibSnapshot {
    static SNAPSHOT: OnceLock<GeneratedDefaultLibSnapshot> = OnceLock::new();
    SNAPSHOT.get_or_init(build_dom_snapshot)
}

fn build_core_snapshot() -> GeneratedDefaultLibSnapshot {
    let mut type_declarations = TypeDeclarationTable::new();
    let mut symbols = SymbolTable::new();

    insert_type_declaration(
        &mut type_declarations,
        "Array",
        interface_decl(
            "Array",
            "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
            vec![type_param("T", None, None)],
            vec![],
            vec![
                interface_member("length", ParsedType::Number, false),
                interface_member(
                    "map",
                    ParsedType::Function(ParsedFunctionType {
                        parameters: vec![fn_param(
                            ParsedType::Function(ParsedFunctionType {
                                parameters: vec![
                                    fn_param(ParsedType::Named(named("T", vec![])), false),
                                    fn_param(ParsedType::Number, false),
                                    fn_param(
                                        ParsedType::Array(Box::new(ParsedType::Named(named(
                                            "T",
                                            vec![],
                                        )))),
                                        false,
                                    ),
                                ],
                                return_type: Box::new(ParsedType::Named(named("U", vec![]))),
                                type_parameters: vec![type_param("U", None, None)],
                            }),
                            false,
                        )],
                        return_type: Box::new(ParsedType::Array(Box::new(ParsedType::Named(
                            named("U", vec![]),
                        )))),
                        type_parameters: vec![type_param("U", None, None)],
                    }),
                    false,
                ),
                interface_member(
                    "find",
                    ParsedType::Function(ParsedFunctionType {
                        parameters: vec![fn_param(
                            ParsedType::Function(ParsedFunctionType {
                                parameters: vec![
                                    fn_param(ParsedType::Named(named("T", vec![])), false),
                                    fn_param(ParsedType::Number, false),
                                    fn_param(
                                        ParsedType::Array(Box::new(ParsedType::Named(named(
                                            "T",
                                            vec![],
                                        )))),
                                        false,
                                    ),
                                ],
                                return_type: Box::new(ParsedType::Unknown),
                                type_parameters: vec![],
                            }),
                            false,
                        )],
                        return_type: Box::new(ParsedType::Union(vec![
                            ParsedType::Named(named("T", vec![])),
                            ParsedType::Undefined,
                        ])),
                        type_parameters: vec![],
                    }),
                    false,
                ),
                interface_member(
                    "join",
                    ParsedType::Function(ParsedFunctionType {
                        parameters: vec![fn_param(ParsedType::String, true)],
                        return_type: Box::new(ParsedType::String),
                        type_parameters: vec![],
                    }),
                    false,
                ),
                interface_member(
                    "includes",
                    ParsedType::Function(ParsedFunctionType {
                        parameters: vec![fn_param(ParsedType::Named(named("T", vec![])), false)],
                        return_type: Box::new(ParsedType::Boolean),
                        type_parameters: vec![],
                    }),
                    false,
                ),
                interface_member(
                    "push",
                    ParsedType::Function(ParsedFunctionType {
                        parameters: vec![fn_param(
                            ParsedType::Array(Box::new(ParsedType::Named(named("T", vec![])))),
                            false,
                        )],
                        return_type: Box::new(ParsedType::Number),
                        type_parameters: vec![],
                    }),
                    false,
                ),
            ],
        ),
    );

    insert_type_declaration(
        &mut type_declarations,
        "ReadonlyArray",
        interface_decl(
            "ReadonlyArray",
            "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
            vec![type_param("T", None, None)],
            vec![],
            vec![interface_member("length", ParsedType::Number, false)],
        ),
    );

    insert_type_declaration(
        &mut type_declarations,
        "ArrayConstructor",
        interface_decl(
            "ArrayConstructor",
            "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
            vec![],
            vec![],
            vec![interface_member(
                "from",
                ParsedType::Function(ParsedFunctionType {
                    parameters: vec![fn_param(ParsedType::Unknown, false)],
                    return_type: Box::new(ParsedType::Array(Box::new(ParsedType::Any))),
                    type_parameters: vec![],
                }),
                false,
            )],
        ),
    );

    insert_symbol(
        &mut symbols,
        "Array",
        object_value(
            vec![(
                "from",
                function_value(vec![Type::Unknown], array_value(Type::Any), false, 1),
                false,
            )],
            None,
        ),
        SymbolKind::Const,
        Some(function_signature(
            vec![],
            vec![Some(ParsedType::Unknown)],
            Some(ParsedType::Array(Box::new(ParsedType::Any))),
        )),
    );

    insert_type_declaration(
        &mut type_declarations,
        "Promise",
        interface_decl(
            "Promise",
            "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
            vec![type_param("T", None, None)],
            vec![],
            vec![],
        ),
    );

    insert_type_declaration(
        &mut type_declarations,
        "PromiseLike",
        interface_decl(
            "PromiseLike",
            "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
            vec![type_param("T", None, None)],
            vec![],
            vec![],
        ),
    );

    insert_type_declaration(
        &mut type_declarations,
        "PromiseConstructor",
        interface_decl(
            "PromiseConstructor",
            "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
            vec![],
            vec![],
            vec![
                interface_member(
                    "resolve",
                    ParsedType::Function(ParsedFunctionType {
                        parameters: vec![fn_param(ParsedType::Named(named("T", vec![])), false)],
                        return_type: Box::new(ParsedType::Named(named(
                            "Promise",
                            vec![ParsedType::Named(named("T", vec![]))],
                        ))),
                        type_parameters: vec![type_param("T", None, None)],
                    }),
                    false,
                ),
                interface_member(
                    "all",
                    ParsedType::Function(ParsedFunctionType {
                        parameters: vec![fn_param(
                            ParsedType::Array(Box::new(ParsedType::Named(named(
                                "Promise",
                                vec![ParsedType::Named(named("T", vec![]))],
                            )))),
                            false,
                        )],
                        return_type: Box::new(ParsedType::Named(named(
                            "Promise",
                            vec![ParsedType::Array(Box::new(ParsedType::Named(named(
                                "T",
                                vec![],
                            ))))],
                        ))),
                        type_parameters: vec![type_param("T", None, None)],
                    }),
                    false,
                ),
            ],
        ),
    );

    insert_symbol(
        &mut symbols,
        "Promise",
        object_value(
            vec![
                (
                    "resolve",
                    function_value(vec![Type::Any], object_value(vec![], None), false, 1),
                    false,
                ),
                (
                    "all",
                    function_value(
                        vec![Type::Array(Box::new(Type::Any))],
                        object_value(vec![], None),
                        false,
                        1,
                    ),
                    false,
                ),
            ],
            None,
        ),
        SymbolKind::Const,
        Some(function_signature(
            vec![],
            vec![Some(ParsedType::Any)],
            Some(ParsedType::Named(named("Promise", vec![ParsedType::Any]))),
        )),
    );

    insert_type_declaration(
        &mut type_declarations,
        "Map",
        interface_decl(
            "Map",
            "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
            vec![type_param("K", None, None), type_param("V", None, None)],
            vec![],
            vec![
                interface_member(
                    "get",
                    ParsedType::Function(ParsedFunctionType {
                        parameters: vec![fn_param(ParsedType::Named(named("K", vec![])), false)],
                        return_type: Box::new(ParsedType::Any),
                        type_parameters: vec![],
                    }),
                    false,
                ),
                interface_member(
                    "set",
                    ParsedType::Function(ParsedFunctionType {
                        parameters: vec![
                            fn_param(ParsedType::Named(named("K", vec![])), false),
                            fn_param(ParsedType::Named(named("V", vec![])), false),
                        ],
                        return_type: Box::new(ParsedType::Any),
                        type_parameters: vec![],
                    }),
                    false,
                ),
                interface_member(
                    "has",
                    ParsedType::Function(ParsedFunctionType {
                        parameters: vec![fn_param(ParsedType::Named(named("K", vec![])), false)],
                        return_type: Box::new(ParsedType::Boolean),
                        type_parameters: vec![],
                    }),
                    false,
                ),
                interface_member(
                    "delete",
                    ParsedType::Function(ParsedFunctionType {
                        parameters: vec![fn_param(ParsedType::Named(named("K", vec![])), false)],
                        return_type: Box::new(ParsedType::Boolean),
                        type_parameters: vec![],
                    }),
                    false,
                ),
                interface_member(
                    "clear",
                    ParsedType::Function(ParsedFunctionType {
                        parameters: vec![],
                        return_type: Box::new(ParsedType::Void),
                        type_parameters: vec![],
                    }),
                    false,
                ),
                interface_member("size", ParsedType::Number, false),
            ],
        ),
    );

    insert_symbol(
        &mut symbols,
        "Map",
        generated_map_value(),
        SymbolKind::Function,
        Some(function_signature(
            vec![type_param("K", None, None), type_param("V", None, None)],
            vec![],
            Some(ParsedType::Named(named(
                "Map",
                vec![
                    ParsedType::Named(named("K", vec![])),
                    ParsedType::Named(named("V", vec![])),
                ],
            ))),
        )),
    );

    insert_type_declaration(
        &mut type_declarations,
        "Uint8Array",
        interface_decl(
            "Uint8Array",
            "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
            vec![],
            vec![named("Array", vec![ParsedType::Number])],
            vec![],
        ),
    );

    insert_symbol(
        &mut symbols,
        "Uint8Array",
        array_value(Type::Number),
        SymbolKind::Function,
        Some(function_signature(
            vec![],
            vec![Some(ParsedType::Unknown)],
            Some(ParsedType::Named(named("Uint8Array", vec![]))),
        )),
    );

    insert_type_declaration(
        &mut type_declarations,
        "Date",
        type_alias_decl(
            "Date",
            "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
            vec![],
            ParsedType::Any,
        ),
    );

    insert_type_declaration(
        &mut type_declarations,
        "String",
        interface_decl(
            "String",
            "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
            vec![],
            vec![],
            vec![
                interface_member(
                    "replace",
                    ParsedType::Function(ParsedFunctionType {
                        parameters: vec![
                            fn_param(
                                ParsedType::Union(vec![
                                    ParsedType::String,
                                    ParsedType::Named(named("RegExp", vec![])),
                                ]),
                                false,
                            ),
                            fn_param(ParsedType::String, false),
                        ],
                        return_type: Box::new(ParsedType::String),
                        type_parameters: vec![],
                    }),
                    false,
                ),
                interface_member(
                    "split",
                    ParsedType::Function(ParsedFunctionType {
                        parameters: vec![fn_param(
                            ParsedType::Union(vec![
                                ParsedType::String,
                                ParsedType::Named(named("RegExp", vec![])),
                            ]),
                            false,
                        )],
                        return_type: Box::new(ParsedType::Array(Box::new(ParsedType::String))),
                        type_parameters: vec![],
                    }),
                    false,
                ),
                interface_member(
                    "slice",
                    ParsedType::Function(ParsedFunctionType {
                        parameters: vec![
                            fn_param(ParsedType::Number, true),
                            fn_param(ParsedType::Number, true),
                        ],
                        return_type: Box::new(ParsedType::String),
                        type_parameters: vec![],
                    }),
                    false,
                ),
                interface_member(
                    "toLowerCase",
                    ParsedType::Function(ParsedFunctionType {
                        parameters: vec![],
                        return_type: Box::new(ParsedType::String),
                        type_parameters: vec![],
                    }),
                    false,
                ),
                interface_member(
                    "toUpperCase",
                    ParsedType::Function(ParsedFunctionType {
                        parameters: vec![],
                        return_type: Box::new(ParsedType::String),
                        type_parameters: vec![],
                    }),
                    false,
                ),
                interface_member(
                    "padStart",
                    ParsedType::Function(ParsedFunctionType {
                        parameters: vec![
                            fn_param(ParsedType::Number, false),
                            fn_param(ParsedType::String, true),
                        ],
                        return_type: Box::new(ParsedType::String),
                        type_parameters: vec![],
                    }),
                    false,
                ),
                interface_member(
                    "charCodeAt",
                    ParsedType::Function(ParsedFunctionType {
                        parameters: vec![fn_param(ParsedType::Number, false)],
                        return_type: Box::new(ParsedType::Number),
                        type_parameters: vec![],
                    }),
                    false,
                ),
            ],
        ),
    );

    insert_type_declaration(
        &mut type_declarations,
        "Number",
        interface_decl(
            "Number",
            "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
            vec![],
            vec![],
            vec![interface_member(
                "toString",
                ParsedType::Function(ParsedFunctionType {
                    parameters: vec![fn_param(ParsedType::Number, true)],
                    return_type: Box::new(ParsedType::String),
                    type_parameters: vec![],
                }),
                false,
            )],
        ),
    );

    insert_type_declaration(
        &mut type_declarations,
        "Boolean",
        interface_decl(
            "Boolean",
            "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
            vec![],
            vec![],
            vec![],
        ),
    );

    insert_type_declaration(
        &mut type_declarations,
        "ObjectConstructor",
        interface_decl(
            "ObjectConstructor",
            "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
            vec![],
            vec![],
            vec![interface_member(
                "keys",
                ParsedType::Function(ParsedFunctionType {
                    parameters: vec![fn_param(ParsedType::Unknown, false)],
                    return_type: Box::new(ParsedType::Array(Box::new(ParsedType::String))),
                    type_parameters: vec![],
                }),
                false,
            )],
        ),
    );

    insert_symbol(
        &mut symbols,
        "Object",
        object_value(
            vec![(
                "keys",
                function_value(vec![Type::Unknown], array_value(Type::String), false, 1),
                false,
            )],
            None,
        ),
        SymbolKind::Const,
        Some(function_signature(
            vec![],
            vec![Some(ParsedType::Unknown)],
            Some(ParsedType::Array(Box::new(ParsedType::String))),
        )),
    );

    insert_symbol(
        &mut symbols,
        "Date",
        object_value(
            vec![("now", function_value(vec![], Type::Number, false, 0), false)],
            None,
        ),
        SymbolKind::Const,
        None,
    );

    insert_symbol(
        &mut symbols,
        "Math",
        object_value(
            vec![
                (
                    "floor",
                    function_value(vec![Type::Number], Type::Number, false, 1),
                    false,
                ),
                (
                    "max",
                    function_value(
                        vec![Type::Number, Type::Number, Type::Number, Type::Number],
                        Type::Number,
                        false,
                        1,
                    ),
                    false,
                ),
                (
                    "min",
                    function_value(
                        vec![Type::Number, Type::Number, Type::Number, Type::Number],
                        Type::Number,
                        false,
                        1,
                    ),
                    false,
                ),
                (
                    "round",
                    function_value(vec![Type::Number], Type::Number, false, 1),
                    false,
                ),
            ],
            None,
        ),
        SymbolKind::Const,
        None,
    );

    insert_symbol(
        &mut symbols,
        "JSON",
        object_value(
            vec![
                (
                    "stringify",
                    function_value(vec![Type::Unknown], Type::String, false, 1),
                    false,
                ),
                (
                    "parse",
                    function_value(vec![Type::String], Type::Unknown, false, 1),
                    false,
                ),
            ],
            None,
        ),
        SymbolKind::Const,
        None,
    );

    insert_symbol(
        &mut symbols,
        "decodeURIComponent",
        function_value(vec![Type::String], Type::String, false, 1),
        SymbolKind::Function,
        Some(function_signature(
            vec![],
            vec![Some(ParsedType::String)],
            Some(ParsedType::String),
        )),
    );

    insert_symbol(
        &mut symbols,
        "isNaN",
        function_value(vec![Type::Unknown], Type::Boolean, false, 1),
        SymbolKind::Function,
        Some(function_signature(
            vec![],
            vec![Some(ParsedType::Unknown)],
            Some(ParsedType::Boolean),
        )),
    );

    insert_symbol(
        &mut symbols,
        "Number",
        function_value(vec![Type::Unknown], Type::Number, false, 0),
        SymbolKind::Function,
        Some(function_signature(
            vec![],
            vec![Some(ParsedType::Unknown)],
            Some(ParsedType::Number),
        )),
    );

    insert_symbol(
        &mut symbols,
        "String",
        function_value(vec![Type::Unknown], Type::String, false, 0),
        SymbolKind::Function,
        Some(function_signature(
            vec![],
            vec![Some(ParsedType::Unknown)],
            Some(ParsedType::String),
        )),
    );

    insert_symbol(
        &mut symbols,
        "Boolean",
        function_value(vec![Type::Unknown], Type::Boolean, false, 0),
        SymbolKind::Function,
        Some(function_signature(
            vec![],
            vec![Some(ParsedType::Unknown)],
            Some(ParsedType::Boolean),
        )),
    );

    insert_symbol(
        &mut symbols,
        "Map",
        generated_map_value(),
        SymbolKind::Function,
        Some(function_signature(
            vec![type_param("K", None, None), type_param("V", None, None)],
            vec![],
            Some(ParsedType::Named(named(
                "Map",
                vec![
                    ParsedType::Named(named("K", vec![])),
                    ParsedType::Named(named("V", vec![])),
                ],
            ))),
        )),
    );

    insert_symbol(
        &mut symbols,
        "Uint8Array",
        array_value(Type::Number),
        SymbolKind::Function,
        Some(function_signature(
            vec![],
            vec![Some(ParsedType::Unknown)],
            Some(ParsedType::Named(named("Uint8Array", vec![]))),
        )),
    );

    insert_type_declaration(
        &mut type_declarations,
        "Partial",
        type_alias_decl(
            "Partial",
            "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
            vec![type_param("T", None, None)],
            ParsedType::Mapped(typescript_rust_syntax::ParsedMappedType {
                key_name: "P".to_string(),
                key_span: None,
                constraint: Box::new(ParsedType::KeyOf(Box::new(ParsedType::Named(named(
                    "T",
                    vec![],
                ))))),
                value_type: Box::new(ParsedType::IndexedAccess(
                    typescript_rust_syntax::ParsedIndexedAccessType {
                        object_type: Box::new(ParsedType::Named(named("T", vec![]))),
                        index_type: Box::new(ParsedType::Named(named("P", vec![]))),
                        span: None,
                    },
                )),
                optional: true,
                span: None,
            }),
        ),
    );

    insert_type_declaration(
        &mut type_declarations,
        "Pick",
        type_alias_decl(
            "Pick",
            "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
            vec![
                type_param("T", None, None),
                type_param(
                    "K",
                    Some(ParsedType::KeyOf(Box::new(ParsedType::Named(named(
                        "T",
                        vec![],
                    ))))),
                    None,
                ),
            ],
            ParsedType::Mapped(typescript_rust_syntax::ParsedMappedType {
                key_name: "P".to_string(),
                key_span: None,
                constraint: Box::new(ParsedType::Named(named("K", vec![]))),
                value_type: Box::new(ParsedType::IndexedAccess(
                    typescript_rust_syntax::ParsedIndexedAccessType {
                        object_type: Box::new(ParsedType::Named(named("T", vec![]))),
                        index_type: Box::new(ParsedType::Named(named("P", vec![]))),
                        span: None,
                    },
                )),
                optional: false,
                span: None,
            }),
        ),
    );

    insert_type_declaration(
        &mut type_declarations,
        "Record",
        type_alias_decl(
            "Record",
            "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
            vec![
                type_param(
                    "K",
                    Some(ParsedType::KeyOf(Box::new(ParsedType::Any))),
                    None,
                ),
                type_param("T", None, None),
            ],
            ParsedType::Mapped(typescript_rust_syntax::ParsedMappedType {
                key_name: "P".to_string(),
                key_span: None,
                constraint: Box::new(ParsedType::Named(named("K", vec![]))),
                value_type: Box::new(ParsedType::Named(named("T", vec![]))),
                optional: false,
                span: None,
            }),
        ),
    );

    insert_type_declaration(
        &mut type_declarations,
        "Omit",
        type_alias_decl(
            "Omit",
            "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
            vec![
                type_param("T", None, None),
                type_param(
                    "K",
                    Some(ParsedType::KeyOf(Box::new(ParsedType::Any))),
                    None,
                ),
            ],
            ParsedType::Named(named(
                "Pick",
                vec![
                    ParsedType::Named(named("T", vec![])),
                    ParsedType::Named(named(
                        "Exclude",
                        vec![
                            ParsedType::KeyOf(Box::new(ParsedType::Named(named("T", vec![])))),
                            ParsedType::Named(named("K", vec![])),
                        ],
                    )),
                ],
            )),
        ),
    );

    insert_type_declaration(
        &mut type_declarations,
        "Parameters",
        type_alias_decl(
            "Parameters",
            "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
            vec![type_param("T", None, None)],
            ParsedType::Array(Box::new(ParsedType::Unknown)),
        ),
    );

    insert_type_declaration(
        &mut type_declarations,
        "ReturnType",
        type_alias_decl(
            "ReturnType",
            "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
            vec![type_param("T", None, None)],
            ParsedType::Unknown,
        ),
    );

    // type Exclude<T, U> = T extends U ? never : T;
    insert_type_declaration(
        &mut type_declarations,
        "Exclude",
        type_alias_decl(
            "Exclude",
            "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
            vec![type_param("T", None, None), type_param("U", None, None)],
            conditional(
                ParsedType::Named(named("T", vec![])),
                ParsedType::Named(named("U", vec![])),
                ParsedType::Never,
                ParsedType::Named(named("T", vec![])),
            ),
        ),
    );

    // type Extract<T, U> = T extends U ? T : never;
    insert_type_declaration(
        &mut type_declarations,
        "Extract",
        type_alias_decl(
            "Extract",
            "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
            vec![type_param("T", None, None), type_param("U", None, None)],
            conditional(
                ParsedType::Named(named("T", vec![])),
                ParsedType::Named(named("U", vec![])),
                ParsedType::Named(named("T", vec![])),
                ParsedType::Never,
            ),
        ),
    );

    // type NonNullable<T> = T extends null | undefined ? never : T;
    // `null` is modeled as `undefined` here, so the extends type collapses to
    // `undefined`; both branches stay faithful for the `string | undefined` case.
    insert_type_declaration(
        &mut type_declarations,
        "NonNullable",
        type_alias_decl(
            "NonNullable",
            "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
            vec![type_param("T", None, None)],
            conditional(
                ParsedType::Named(named("T", vec![])),
                ParsedType::Union(vec![ParsedType::Undefined, ParsedType::Undefined]),
                ParsedType::Never,
                ParsedType::Named(named("T", vec![])),
            ),
        ),
    );

    GeneratedDefaultLibSnapshot {
        file_name: "crates/typescript-rust-checker/generated-libs/lib.es.generated.d.ts",
        type_declarations,
        symbols,
    }
}

fn build_dom_snapshot() -> GeneratedDefaultLibSnapshot {
    let mut type_declarations = TypeDeclarationTable::new();
    let mut symbols = SymbolTable::new();

    insert_type_declaration(
        &mut type_declarations,
        "TextEncoder",
        interface_decl(
            "TextEncoder",
            "crates/typescript-rust-checker/generated-libs/lib.dom.generated.d.ts",
            vec![],
            vec![],
            vec![interface_member(
                "encode",
                ParsedType::Function(ParsedFunctionType {
                    parameters: vec![fn_param(ParsedType::String, true)],
                    return_type: Box::new(ParsedType::Named(named("Uint8Array", vec![]))),
                    type_parameters: vec![],
                }),
                false,
            )],
        ),
    );

    insert_symbol(
        &mut symbols,
        "TextEncoder",
        function_value(
            vec![],
            object_value(
                vec![(
                    "encode",
                    function_value(vec![Type::String], array_value(Type::Number), false, 0),
                    false,
                )],
                None,
            ),
            false,
            0,
        ),
        SymbolKind::Function,
        Some(function_signature(
            vec![],
            vec![Some(ParsedType::String)],
            Some(ParsedType::Named(named("Uint8Array", vec![]))),
        )),
    );

    insert_type_declaration(
        &mut type_declarations,
        "AuthenticatorTransport",
        type_alias_decl(
            "AuthenticatorTransport",
            "crates/typescript-rust-checker/generated-libs/lib.dom.generated.d.ts",
            vec![],
            ParsedType::Union(vec![
                ParsedType::StringLiteral("ble".to_string()),
                ParsedType::StringLiteral("cable".to_string()),
                ParsedType::StringLiteral("hybrid".to_string()),
                ParsedType::StringLiteral("internal".to_string()),
                ParsedType::StringLiteral("nfc".to_string()),
                ParsedType::StringLiteral("smart-card".to_string()),
                ParsedType::StringLiteral("usb".to_string()),
            ]),
        ),
    );

    insert_type_declaration(
        &mut type_declarations,
        "Crypto",
        interface_decl(
            "Crypto",
            "crates/typescript-rust-checker/generated-libs/lib.dom.generated.d.ts",
            vec![],
            vec![],
            vec![interface_member(
                "getRandomValues",
                ParsedType::Function(ParsedFunctionType {
                    parameters: vec![fn_param(
                        ParsedType::Named(named("Uint8Array", vec![])),
                        false,
                    )],
                    return_type: Box::new(ParsedType::Named(named("Uint8Array", vec![]))),
                    type_parameters: vec![],
                }),
                false,
            )],
        ),
    );

    insert_type_declaration(
        &mut type_declarations,
        "Headers",
        interface_decl(
            "Headers",
            "crates/typescript-rust-checker/generated-libs/lib.dom.generated.d.ts",
            vec![],
            vec![],
            vec![],
        ),
    );

    insert_type_declaration(
        &mut type_declarations,
        "Request",
        interface_decl(
            "Request",
            "crates/typescript-rust-checker/generated-libs/lib.dom.generated.d.ts",
            vec![],
            vec![],
            vec![],
        ),
    );

    insert_type_declaration(
        &mut type_declarations,
        "Response",
        interface_decl(
            "Response",
            "crates/typescript-rust-checker/generated-libs/lib.dom.generated.d.ts",
            vec![],
            vec![],
            vec![
                interface_member("ok", ParsedType::Boolean, false),
                interface_member("status", ParsedType::Number, false),
                interface_member(
                    "json",
                    ParsedType::Function(ParsedFunctionType {
                        parameters: vec![],
                        return_type: Box::new(ParsedType::Unknown),
                        type_parameters: vec![],
                    }),
                    false,
                ),
            ],
        ),
    );

    insert_type_declaration(
        &mut type_declarations,
        "URL",
        interface_decl(
            "URL",
            "crates/typescript-rust-checker/generated-libs/lib.dom.generated.d.ts",
            vec![],
            vec![],
            vec![],
        ),
    );

    insert_type_declaration(
        &mut type_declarations,
        "Console",
        interface_decl(
            "Console",
            "crates/typescript-rust-checker/generated-libs/lib.dom.generated.d.ts",
            vec![],
            vec![],
            vec![
                interface_member("log", ParsedType::Any, false),
                interface_member("warn", ParsedType::Any, false),
                interface_member("error", ParsedType::Any, false),
            ],
        ),
    );

    insert_symbol(
        &mut symbols,
        "fetch",
        function_value(
            vec![Type::Unknown, Type::Unknown],
            object_value(
                vec![
                    ("ok", Type::Boolean, false),
                    ("status", Type::Number, false),
                    (
                        "json",
                        function_value(vec![], Type::Unknown, false, 0),
                        false,
                    ),
                ],
                None,
            ),
            false,
            2,
        ),
        SymbolKind::Function,
        Some(function_signature(
            vec![],
            vec![Some(ParsedType::Unknown), Some(ParsedType::Unknown)],
            Some(ParsedType::Named(named(
                "Promise",
                vec![ParsedType::Named(named("Response", vec![]))],
            ))),
        )),
    );

    insert_symbol(
        &mut symbols,
        "Headers",
        object_value(vec![], None),
        SymbolKind::Function,
        Some(function_signature(
            vec![],
            vec![Some(ParsedType::Unknown)],
            Some(ParsedType::Named(named("Headers", vec![]))),
        )),
    );

    insert_symbol(
        &mut symbols,
        "Request",
        object_value(vec![], None),
        SymbolKind::Function,
        Some(function_signature(
            vec![],
            vec![Some(ParsedType::Unknown), Some(ParsedType::Unknown)],
            Some(ParsedType::Named(named("Request", vec![]))),
        )),
    );

    insert_symbol(
        &mut symbols,
        "Response",
        object_value(
            vec![
                ("ok", Type::Boolean, false),
                ("status", Type::Number, false),
                (
                    "json",
                    function_value(vec![], Type::Unknown, false, 0),
                    false,
                ),
            ],
            None,
        ),
        SymbolKind::Function,
        Some(function_signature(
            vec![],
            vec![Some(ParsedType::Unknown), Some(ParsedType::Unknown)],
            Some(ParsedType::Named(named("Response", vec![]))),
        )),
    );

    insert_symbol(
        &mut symbols,
        "URL",
        object_value(vec![], None),
        SymbolKind::Function,
        Some(function_signature(
            vec![],
            vec![Some(ParsedType::String)],
            Some(ParsedType::Named(named("URL", vec![]))),
        )),
    );

    insert_symbol(
        &mut symbols,
        "crypto",
        object_value(
            vec![(
                "getRandomValues",
                function_value(
                    vec![Type::Array(Box::new(Type::Number))],
                    array_value(Type::Number),
                    false,
                    1,
                ),
                false,
            )],
            None,
        ),
        SymbolKind::Const,
        None,
    );

    insert_symbol(
        &mut symbols,
        "console",
        object_value(
            vec![
                ("log", Type::Any, false),
                ("warn", Type::Any, false),
                ("error", Type::Any, false),
            ],
            None,
        ),
        SymbolKind::Const,
        None,
    );

    GeneratedDefaultLibSnapshot {
        file_name: "crates/typescript-rust-checker/generated-libs/lib.dom.generated.d.ts",
        type_declarations,
        symbols,
    }
}

fn insert_type_declaration(
    table: &mut TypeDeclarationTable,
    name: &str,
    declaration: TypeDeclarationInfo,
) {
    let _ = table.insert(name.to_string(), declaration);
}

fn insert_symbol(
    table: &mut SymbolTable,
    name: &str,
    ty: Type,
    kind: SymbolKind,
    function_signature: Option<FunctionSignatureInfo>,
) {
    let _ = table.insert(
        name.to_string(),
        SymbolInfo {
            ty,
            kind,
            function_signature,
        },
    );
}

fn type_param(
    name: &str,
    constraint: Option<ParsedType>,
    default_type: Option<ParsedType>,
) -> ParsedTypeParameter {
    ParsedTypeParameter {
        name: name.to_string(),
        name_span: None,
        constraint,
        default_type,
        span: None,
    }
}

fn fn_param(ty: ParsedType, optional: bool) -> ParsedFunctionTypeParameter {
    ParsedFunctionTypeParameter {
        name: None,
        name_span: None,
        ty,
        optional,
        is_this: false,
        rest: false,
    }
}

fn interface_member(name: &str, ty: ParsedType, optional: bool) -> ParsedInterfaceMember {
    ParsedInterfaceMember {
        name: name.to_string(),
        name_span: None,
        optional,
        ty,
    }
}

fn named(name: &str, type_arguments: Vec<ParsedType>) -> ParsedNamedType {
    ParsedNamedType {
        name: name.to_string(),
        span: None,
        type_arguments,
    }
}

fn conditional(
    check_type: ParsedType,
    extends_type: ParsedType,
    true_type: ParsedType,
    false_type: ParsedType,
) -> ParsedType {
    ParsedType::Conditional(ParsedConditionalType {
        check_type: Box::new(check_type),
        extends_type: Box::new(extends_type),
        true_type: Box::new(true_type),
        false_type: Box::new(false_type),
        span: None,
    })
}

fn function_signature(
    type_parameters: Vec<ParsedTypeParameter>,
    parameter_types: Vec<Option<ParsedType>>,
    return_type: Option<ParsedType>,
) -> FunctionSignatureInfo {
    FunctionSignatureInfo {
        type_parameters,
        parameter_types,
        return_type,
    }
}

fn type_alias_decl(
    name: &str,
    file_name: &str,
    type_parameters: Vec<ParsedTypeParameter>,
    ty: ParsedType,
) -> TypeDeclarationInfo {
    TypeDeclarationInfo::Alias(TypeAliasInfo {
        name: name.to_string(),
        file_name: file_name.to_string(),
        name_span: None,
        type_parameters,
        ty,
        resolution_scope: None,
    })
}

fn interface_decl(
    name: &str,
    file_name: &str,
    type_parameters: Vec<ParsedTypeParameter>,
    extends: Vec<ParsedNamedType>,
    members: Vec<ParsedInterfaceMember>,
) -> TypeDeclarationInfo {
    TypeDeclarationInfo::Interface(crate::symbols::InterfaceInfo {
        name: name.to_string(),
        file_name: file_name.to_string(),
        name_span: None,
        type_parameters,
        extends,
        members,
        string_index_type: None,
        resolution_scope: None,
    })
}

fn object_value(properties: Vec<(&str, Type, bool)>, string_index_type: Option<Type>) -> Type {
    let mut map = BTreeMap::new();
    for (name, ty, optional) in properties {
        map.insert(name.to_string(), ObjectProperty { ty, optional });
    }

    Type::Object(ObjectType::new(map, string_index_type))
}

fn array_value(element: Type) -> Type {
    Type::Array(Box::new(element))
}

fn function_value(
    parameters: Vec<Type>,
    return_type: Type,
    is_variadic: bool,
    required_parameter_count: usize,
) -> Type {
    Type::Function(FunctionType::new(
        parameters,
        return_type,
        is_variadic,
        required_parameter_count,
    ))
}

fn generated_map_value() -> Type {
    let mut properties = BTreeMap::new();
    properties.insert(
        "get".to_string(),
        ObjectProperty {
            ty: Type::Function(FunctionType::new(vec![Type::Any], Type::Any, false, 1)),
            optional: false,
        },
    );
    properties.insert(
        "set".to_string(),
        ObjectProperty {
            ty: Type::Function(FunctionType::new(
                vec![Type::Any, Type::Any],
                Type::Any,
                false,
                2,
            )),
            optional: false,
        },
    );
    properties.insert(
        "has".to_string(),
        ObjectProperty {
            ty: Type::Function(FunctionType::new(vec![Type::Any], Type::Boolean, false, 1)),
            optional: false,
        },
    );
    properties.insert(
        "delete".to_string(),
        ObjectProperty {
            ty: Type::Function(FunctionType::new(vec![Type::Any], Type::Boolean, false, 1)),
            optional: false,
        },
    );
    properties.insert(
        "clear".to_string(),
        ObjectProperty {
            ty: Type::Function(FunctionType::new(vec![], Type::Void, false, 0)),
            optional: false,
        },
    );
    properties.insert(
        "size".to_string(),
        ObjectProperty {
            ty: Type::Number,
            optional: false,
        },
    );

    Type::Object(ObjectType::new(properties, None))
}
