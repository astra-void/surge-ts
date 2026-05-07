use std::collections::BTreeMap;

use crate::context::CheckerContext;
use crate::symbols::{SymbolInfo, SymbolKind, TypeAliasInfo, TypeDeclarationInfo};
use typescript_rust_syntax::{
    ParsedFunctionType, ParsedFunctionTypeParameter, ParsedNamedType, ParsedObjectType,
    ParsedObjectTypeProperty, ParsedType, ParsedTypeParameter,
};
use typescript_rust_types::{FunctionType, ObjectProperty, ObjectType, Type};

pub(crate) fn inject_builtins(ctx: &mut CheckerContext) {
    if ctx.options.no_lib {
        return;
    }

    inject_builtin_types(ctx);
    inject_builtin_values(ctx);
}

fn inject_builtin_types(ctx: &mut CheckerContext) {
    let types = [
        "Array",
        "ReadonlyArray",
        "Promise",
        "PromiseLike",
        "Record",
        "Partial",
        "Required",
        "Readonly",
        "Pick",
        "Omit",
        "Awaited",
        "ReturnType",
        "Parameters",
        "Object",
        "Map",
        "Uint8Array",
        "Date",
        "Error",
        "RegExp",
    ];

    for name in types {
        let type_parameters = if name == "Record" {
            vec![
                ParsedTypeParameter {
                    name: "K".to_string(),
                    name_span: None,
                    constraint: None,
                    default_type: None,
                    span: None,
                },
                ParsedTypeParameter {
                    name: "T".to_string(),
                    name_span: None,
                    constraint: None,
                    default_type: None,
                    span: None,
                },
            ]
        } else if name == "Map" {
            vec![
                ParsedTypeParameter {
                    name: "K".to_string(),
                    name_span: None,
                    constraint: None,
                    default_type: None,
                    span: None,
                },
                ParsedTypeParameter {
                    name: "V".to_string(),
                    name_span: None,
                    constraint: None,
                    default_type: None,
                    span: None,
                },
            ]
        } else if name == "Pick" || name == "Omit" {
            vec![
                ParsedTypeParameter {
                    name: "T".to_string(),
                    name_span: None,
                    constraint: None,
                    default_type: None,
                    span: None,
                },
                ParsedTypeParameter {
                    name: "K".to_string(),
                    name_span: None,
                    constraint: None,
                    default_type: None,
                    span: None,
                },
            ]
        } else if name == "Object"
            || name == "Uint8Array"
            || name == "Date"
            || name == "Error"
            || name == "RegExp"
        {
            vec![]
        } else {
            vec![ParsedTypeParameter {
                name: "T".to_string(),
                name_span: None,
                constraint: None,
                default_type: None,
                span: None,
            }]
        };

        let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo {
            name: name.to_string(),
            file_name: "<built-in>".to_string(),
            name_span: None,
            type_parameters,
            ty: match name {
                "Map" => map_builtin_type(),
                "Uint8Array" => ParsedType::Array(Box::new(ParsedType::Number)),
                "Promise" | "PromiseLike" => {
                    ParsedType::Named(typescript_rust_syntax::ParsedNamedType {
                        name: "T".to_string(),
                        span: None,
                        type_arguments: vec![],
                    })
                }
                "Object" => ParsedType::Unknown,
                _ => ParsedType::Unknown,
            },
            resolution_scope: None,
        });

        if ctx.ambient_global_type_declarations.get(name).is_none() {
            let _ = ctx
                .ambient_global_type_declarations
                .insert(name.to_string(), declaration);
        }
    }
}

fn inject_builtin_values(ctx: &mut CheckerContext) {
    // console
    let mut console_props = BTreeMap::new();
    console_props.insert(
        "log".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::Any],
            return_type: Box::new(Type::Void),
            is_variadic: true,
            required_parameter_count: 1,
        })),
    );
    console_props.insert(
        "warn".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::Any],
            return_type: Box::new(Type::Void),
            is_variadic: true,
            required_parameter_count: 1,
        })),
    );
    console_props.insert(
        "error".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::Any],
            return_type: Box::new(Type::Void),
            is_variadic: true,
            required_parameter_count: 1,
        })),
    );

    // Math
    let mut math_props = BTreeMap::new();
    math_props.insert(
        "max".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::Number, Type::Number],
            return_type: Box::new(Type::Number),
            is_variadic: true,
            required_parameter_count: 2,
        })),
    );
    math_props.insert(
        "min".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::Number, Type::Number],
            return_type: Box::new(Type::Number),
            is_variadic: true,
            required_parameter_count: 2,
        })),
    );
    math_props.insert(
        "floor".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::Number],
            return_type: Box::new(Type::Number),
            is_variadic: true,
            required_parameter_count: 1,
        })),
    );
    math_props.insert(
        "round".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::Number],
            return_type: Box::new(Type::Number),
            is_variadic: true,
            required_parameter_count: 1,
        })),
    );

    // JSON
    let mut json_props = BTreeMap::new();
    json_props.insert(
        "parse".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::String],
            return_type: Box::new(Type::Any),
            is_variadic: false,
            required_parameter_count: 1,
        })),
    );
    json_props.insert(
        "stringify".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::Any],
            return_type: Box::new(Type::String),
            is_variadic: false,
            required_parameter_count: 1,
        })),
    );

    // Array
    let mut array_props = BTreeMap::new();
    array_props.insert(
        "from".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::Any],
            return_type: Box::new(Type::Array(Box::new(Type::Any))),
            is_variadic: true,
            required_parameter_count: 1,
        })),
    );

    // Date
    let mut date_props = BTreeMap::new();
    date_props.insert(
        "now".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![],
            return_type: Box::new(Type::Number),
            is_variadic: false,
            required_parameter_count: 0,
        })),
    );

    let mut promise_props = BTreeMap::new();
    promise_props.insert(
        "resolve".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::Any],
            return_type: Box::new(Type::Any),
            is_variadic: true,
            required_parameter_count: 1,
        })),
    );

    let builtins = vec![
        (
            "console",
            Type::Object(ObjectType {
                properties: console_props,
            }),
        ),
        (
            "Array",
            Type::Object(ObjectType {
                properties: array_props,
            }),
        ),
        (
            "Math",
            Type::Object(ObjectType {
                properties: math_props,
            }),
        ),
        (
            "JSON",
            Type::Object(ObjectType {
                properties: json_props,
            }),
        ),
        (
            "Date",
            Type::Object(ObjectType {
                properties: date_props,
            }),
        ),
        (
            "Promise",
            Type::Object(ObjectType {
                properties: promise_props,
            }),
        ),
        ("Error", Type::Any),
        ("RegExp", Type::Any),
        ("Object", Type::Any),
        ("globalThis", Type::Any),
        ("Map", Type::Any),
        ("Uint8Array", Type::Any),
        (
            "isNaN",
            Type::Function(FunctionType {
                parameters: vec![Type::Any],
                return_type: Box::new(Type::Boolean),
                is_variadic: false,
                required_parameter_count: 1,
            }),
        ),
        (
            "setTimeout",
            Type::Function(FunctionType {
                parameters: vec![
                    Type::Function(FunctionType {
                        parameters: vec![],
                        return_type: Box::new(Type::Void),
                        is_variadic: false,
                        required_parameter_count: 0,
                    }),
                    Type::Number,
                ],
                return_type: Box::new(Type::Number),
                is_variadic: true,
                required_parameter_count: 2,
            }),
        ),
        (
            "clearTimeout",
            Type::Function(FunctionType {
                parameters: vec![Type::Number],
                return_type: Box::new(Type::Void),
                is_variadic: false,
                required_parameter_count: 1,
            }),
        ),
        (
            "parseInt",
            Type::Function(FunctionType {
                parameters: vec![Type::String],
                return_type: Box::new(Type::Number),
                is_variadic: true,
                required_parameter_count: 1,
            }),
        ),
        (
            "parseFloat",
            Type::Function(FunctionType {
                parameters: vec![Type::String],
                return_type: Box::new(Type::Number),
                is_variadic: false,
                required_parameter_count: 1,
            }),
        ),
        (
            "Number",
            Type::Function(FunctionType {
                parameters: vec![Type::Any],
                return_type: Box::new(Type::Number),
                is_variadic: true,
                required_parameter_count: 1,
            }),
        ),
        (
            "String",
            Type::Function(FunctionType {
                parameters: vec![Type::Any],
                return_type: Box::new(Type::String),
                is_variadic: true,
                required_parameter_count: 1,
            }),
        ),
        (
            "Boolean",
            Type::Function(FunctionType {
                parameters: vec![Type::Any],
                return_type: Box::new(Type::Boolean),
                is_variadic: true,
                required_parameter_count: 1,
            }),
        ),
        (
            "TextEncoder",
            Type::Function(FunctionType {
                parameters: vec![],
                return_type: Box::new(text_encoder_instance_type()),
                is_variadic: false,
                required_parameter_count: 0,
            }),
        ),
    ];

    for (name, ty) in builtins {
        if ctx.ambient_global_symbols.get(name).is_none() {
            let symbol = SymbolInfo {
                ty,
                kind: SymbolKind::Const,
            };
            let _ = ctx.ambient_global_symbols.insert(name.to_string(), symbol);
        }
    }
}

fn map_builtin_type() -> ParsedType {
    ParsedType::Object(ParsedObjectType {
        properties: vec![
            ParsedObjectTypeProperty {
                name: "get".to_string(),
                name_span: None,
                ty: ParsedType::Function(ParsedFunctionType {
                    parameters: vec![ParsedFunctionTypeParameter {
                        name: Some("key".to_string()),
                        name_span: None,
                        ty: ParsedType::Named(ParsedNamedType {
                            name: "K".to_string(),
                            span: None,
                            type_arguments: vec![],
                        }),
                        optional: false,
                    }],
                    return_type: Box::new(ParsedType::Named(ParsedNamedType {
                        name: "V".to_string(),
                        span: None,
                        type_arguments: vec![],
                    })),
                    type_parameters: vec![],
                }),
                optional: false,
            },
            ParsedObjectTypeProperty {
                name: "set".to_string(),
                name_span: None,
                ty: ParsedType::Function(ParsedFunctionType {
                    parameters: vec![
                        ParsedFunctionTypeParameter {
                            name: Some("key".to_string()),
                            name_span: None,
                            ty: ParsedType::Named(ParsedNamedType {
                                name: "K".to_string(),
                                span: None,
                                type_arguments: vec![],
                            }),
                            optional: false,
                        },
                        ParsedFunctionTypeParameter {
                            name: Some("value".to_string()),
                            name_span: None,
                            ty: ParsedType::Named(ParsedNamedType {
                                name: "V".to_string(),
                                span: None,
                                type_arguments: vec![],
                            }),
                            optional: false,
                        },
                    ],
                    return_type: Box::new(ParsedType::Any),
                    type_parameters: vec![],
                }),
                optional: false,
            },
            ParsedObjectTypeProperty {
                name: "delete".to_string(),
                name_span: None,
                ty: ParsedType::Function(ParsedFunctionType {
                    parameters: vec![ParsedFunctionTypeParameter {
                        name: Some("key".to_string()),
                        name_span: None,
                        ty: ParsedType::Named(ParsedNamedType {
                            name: "K".to_string(),
                            span: None,
                            type_arguments: vec![],
                        }),
                        optional: false,
                    }],
                    return_type: Box::new(ParsedType::Boolean),
                    type_parameters: vec![],
                }),
                optional: false,
            },
            ParsedObjectTypeProperty {
                name: "has".to_string(),
                name_span: None,
                ty: ParsedType::Function(ParsedFunctionType {
                    parameters: vec![ParsedFunctionTypeParameter {
                        name: Some("key".to_string()),
                        name_span: None,
                        ty: ParsedType::Named(ParsedNamedType {
                            name: "K".to_string(),
                            span: None,
                            type_arguments: vec![],
                        }),
                        optional: false,
                    }],
                    return_type: Box::new(ParsedType::Boolean),
                    type_parameters: vec![],
                }),
                optional: false,
            },
            ParsedObjectTypeProperty {
                name: "clear".to_string(),
                name_span: None,
                ty: ParsedType::Function(ParsedFunctionType {
                    parameters: vec![],
                    return_type: Box::new(ParsedType::Void),
                    type_parameters: vec![],
                }),
                optional: false,
            },
        ],
    })
}

fn text_encoder_instance_type() -> Type {
    let mut properties = BTreeMap::new();
    properties.insert(
        "encode".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::String],
            return_type: Box::new(Type::Array(Box::new(Type::Number))),
            is_variadic: false,
            required_parameter_count: 1,
        })),
    );

    Type::Object(ObjectType { properties })
}
