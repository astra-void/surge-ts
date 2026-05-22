use std::collections::BTreeMap;

use crate::context::CheckerContext;
use crate::symbols::{SymbolInfo, SymbolKind, TypeAliasInfo, TypeDeclarationInfo};
use typescript_rust_syntax::{
    ParsedFunctionType, ParsedObjectType, ParsedObjectTypeProperty, ParsedType, ParsedTypeParameter,
};
use typescript_rust_types::{FunctionType, ObjectProperty, ObjectType, Type, union_type};

pub(crate) fn inject_builtins(ctx: &mut CheckerContext) {
    if ctx.options.no_lib {
        return;
    }

    inject_builtin_types(ctx);
    inject_builtin_values(ctx);
    inject_configured_types(ctx);
    inject_configured_values(ctx);
}

fn inject_builtin_types(ctx: &mut CheckerContext) {
    let types = ["Object", "Error", "RegExp", "ArrayBuffer"];

    for name in types {
        let type_parameters = vec![];

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
    let builtins = vec![
        (
            "Error",
            Type::Function(FunctionType {
                parameters: vec![Type::Any],
                return_type: Box::new(Type::Any),
                is_variadic: true,
                required_parameter_count: 1,
            }),
        ),
        ("RegExp", Type::Any),
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
        ("document", Type::Any),
    ];

    for (name, ty) in builtins {
        if ctx.ambient_global_symbols.get(name).is_none() {
            let symbol = SymbolInfo {
                ty,
                kind: SymbolKind::Const,
                function_signature: None,
            };
            let _ = ctx.ambient_global_symbols.insert(name.to_string(), symbol);
        }
    }
}

fn inject_configured_types(ctx: &mut CheckerContext) {
    if !ctx.options.types.iter().any(|ty| ty == "node") {
        return;
    }

    if ctx.ambient_global_type_declarations.get("Buffer").is_none() {
        let declaration = TypeDeclarationInfo::Alias(TypeAliasInfo {
            name: "Buffer".to_string(),
            file_name: "<built-in>".to_string(),
            name_span: None,
            type_parameters: vec![ParsedTypeParameter {
                name: "T".to_string(),
                name_span: None,
                constraint: None,
                default_type: Some(ParsedType::Any),
                span: None,
            }],
            ty: ParsedType::Object(ParsedObjectType {
                properties: vec![ParsedObjectTypeProperty {
                    name: "toString".to_string(),
                    name_span: None,
                    ty: ParsedType::Function(ParsedFunctionType {
                        parameters: vec![],
                        return_type: Box::new(ParsedType::String),
                        type_parameters: vec![],
                    }),
                    optional: false,
                }],
            }),
            resolution_scope: None,
        });
        ctx.ambient_global_type_declarations
            .insert("Buffer".to_string(), declaration);
    }
}

fn inject_configured_values(ctx: &mut CheckerContext) {
    if !ctx.options.types.iter().any(|ty| ty == "node") {
        return;
    }

    if ctx.ambient_global_symbols.get("Buffer").is_none() {
        let mut props = BTreeMap::new();
        props.insert(
            "from".to_string(),
            ObjectProperty::required(Type::Function(FunctionType {
                parameters: vec![Type::Any, Type::Any],
                return_type: Box::new(Type::Any),
                is_variadic: true,
                required_parameter_count: 1,
            })),
        );

        ctx.ambient_global_symbols.insert(
            "Buffer".to_string(),
            SymbolInfo {
                ty: Type::Object(ObjectType {
                    properties: props,
                    string_index_type: None,
                }),
                kind: SymbolKind::Const,
                function_signature: None,
            },
        );
    }

    if ctx.ambient_global_symbols.get("process").is_none() {
        let mut process_props = BTreeMap::new();
        process_props.insert(
            "env".to_string(),
            ObjectProperty::required(Type::Object(ObjectType {
                properties: BTreeMap::new(),
                string_index_type: Some(Box::new(union_type(vec![Type::String, Type::Undefined]))),
            })),
        );

        ctx.ambient_global_symbols.insert(
            "process".to_string(),
            SymbolInfo {
                ty: Type::Object(ObjectType {
                    properties: process_props,
                    string_index_type: None,
                }),
                kind: SymbolKind::Const,
                function_signature: None,
            },
        );
    }
}
