use std::collections::BTreeMap;

use crate::context::CheckerContext;
use crate::symbols::{SymbolInfo, SymbolKind, TypeAliasInfo, TypeDeclarationInfo};
use typescript_rust_syntax::{ParsedType, ParsedTypeParameter};
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
        "Date",
        "Error",
        "RegExp",
    ];

    for name in types {
        let type_parameters = if name == "Record" || name == "Pick" || name == "Omit" {
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
        } else if name != "Date" && name != "Error" && name != "RegExp" {
            vec![ParsedTypeParameter {
                name: "T".to_string(),
                name_span: None,
                constraint: None,
                default_type: None,
                span: None,
            }]
        } else {
            vec![]
        };

        let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo {
            name: name.to_string(),
            file_name: "<built-in>".to_string(),
            name_span: None,
            type_parameters,
            ty: ParsedType::Unknown,
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
        })),
    );
    console_props.insert(
        "warn".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::Any],
            return_type: Box::new(Type::Void),
        })),
    );
    console_props.insert(
        "error".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::Any],
            return_type: Box::new(Type::Void),
        })),
    );

    // Math
    let mut math_props = BTreeMap::new();
    math_props.insert(
        "max".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::Number, Type::Number],
            return_type: Box::new(Type::Number),
        })),
    );
    math_props.insert(
        "min".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::Number, Type::Number],
            return_type: Box::new(Type::Number),
        })),
    );
    math_props.insert(
        "floor".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::Number],
            return_type: Box::new(Type::Number),
        })),
    );
    math_props.insert(
        "round".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::Number],
            return_type: Box::new(Type::Number),
        })),
    );

    // JSON
    let mut json_props = BTreeMap::new();
    json_props.insert(
        "parse".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::String],
            return_type: Box::new(Type::Any),
        })),
    );
    json_props.insert(
        "stringify".to_string(),
        ObjectProperty::required(Type::Function(FunctionType {
            parameters: vec![Type::Any],
            return_type: Box::new(Type::String),
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
        ("Date", Type::Any),
        ("Error", Type::Any),
        ("RegExp", Type::Any),
        (
            "setTimeout",
            Type::Function(FunctionType {
                parameters: vec![
                    Type::Function(FunctionType {
                        parameters: vec![],
                        return_type: Box::new(Type::Void),
                    }),
                    Type::Number,
                ],
                return_type: Box::new(Type::Number),
            }),
        ),
        (
            "clearTimeout",
            Type::Function(FunctionType {
                parameters: vec![Type::Number],
                return_type: Box::new(Type::Void),
            }),
        ),
        (
            "parseInt",
            Type::Function(FunctionType {
                parameters: vec![Type::String],
                return_type: Box::new(Type::Number),
            }),
        ),
        (
            "parseFloat",
            Type::Function(FunctionType {
                parameters: vec![Type::String],
                return_type: Box::new(Type::Number),
            }),
        ),
        ("Number", Type::Any),
        ("String", Type::Any),
        ("Boolean", Type::Any),
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
