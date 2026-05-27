use std::collections::BTreeMap;

use crate::arena::{alloc_function_type, alloc_object_type};
use crate::context::CheckerContext;
use crate::symbols::{SymbolInfo, SymbolKind, TypeAliasInfo, TypeDeclarationInfo};
use typescript_rust_syntax::{
    ParsedFunctionType, ParsedObjectType, ParsedObjectTypeProperty, ParsedType, ParsedTypeParameter,
};
use typescript_rust_types::{ObjectProperty, Type, union_type};

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
            Type::Function(alloc_function_type(vec![Type::Any], Type::Any, true, 1)),
        ),
        ("RegExp", Type::Any),
        (
            "setTimeout",
            Type::Function(alloc_function_type(
                vec![
                    Type::Function(alloc_function_type(vec![], Type::Void, false, 0)),
                    Type::Number,
                ],
                Type::Number,
                true,
                2,
            )),
        ),
        (
            "clearTimeout",
            Type::Function(alloc_function_type(
                vec![Type::Number],
                Type::Void,
                false,
                1,
            )),
        ),
        (
            "parseInt",
            Type::Function(alloc_function_type(
                vec![Type::String],
                Type::Number,
                true,
                1,
            )),
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
            ObjectProperty::required(Type::Function(alloc_function_type(
                vec![Type::Any, Type::Any],
                Type::Any,
                true,
                1,
            ))),
        );

        ctx.ambient_global_symbols.insert(
            "Buffer".to_string(),
            SymbolInfo {
                ty: Type::Object(alloc_object_type(props, None)),
                kind: SymbolKind::Const,
                function_signature: None,
            },
        );
    }

    if ctx.ambient_global_symbols.get("process").is_none() {
        let mut process_props = BTreeMap::new();
        process_props.insert(
            "env".to_string(),
            ObjectProperty::required(Type::Object(alloc_object_type(
                BTreeMap::new(),
                Some(union_type(vec![Type::String, Type::Undefined])),
            ))),
        );

        ctx.ambient_global_symbols.insert(
            "process".to_string(),
            SymbolInfo {
                ty: Type::Object(alloc_object_type(process_props, None)),
                kind: SymbolKind::Const,
                function_signature: None,
            },
        );
    }
}
