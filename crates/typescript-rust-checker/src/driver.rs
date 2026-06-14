use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use typescript_rust_diagnostics::Diagnostic;
use typescript_rust_syntax::{
    ParsedDefaultExportDeclaration, ParsedExportDeclaration, ParsedInterfaceDeclaration,
    ParsedStatement, ParsedType, ParsedTypeAliasDeclaration, parse_source,
};

use crate::checks::{assign, call, expr, function as check_function, var};
use crate::context::{CheckerContext, DeclarationNamespace, DeclarationResolutionKey, FileKind};
use crate::default_lib::inject_generated_default_lib_snapshot_for_file_name;
use crate::infer::{report_duplicate_type_parameters, validate_local_type_declaration};
use crate::load_default_lib_inputs;
use crate::paths::canonicalize_if_exists_string;
use crate::program::collect_function_signatures_from_statements;
use crate::symbols::{InterfaceInfo, TypeAliasInfo, TypeDeclarationInfo};

pub fn check_source(source_text: &str, file_name: &str) -> Vec<Diagnostic> {
    check_source_with_options(
        source_text,
        file_name,
        crate::context::CheckerOptions::default(),
    )
}

pub fn check_source_with_options(
    source_text: &str,
    file_name: &str,
    options: crate::context::CheckerOptions,
) -> Vec<Diagnostic> {
    let parsed = parse_source(source_text, file_name);
    let file_name = parsed.file_name;
    let mut file_kinds = std::collections::HashMap::new();
    file_kinds.insert(file_name.clone(), classify_file_kind(&file_name));
    let mut ctx = CheckerContext::new(file_name.clone(), options, file_kinds);

    crate::builtins::inject_builtins(&mut ctx);
    inject_generated_default_libs(&mut ctx);

    let mut merged_td = ctx.ambient_global_type_declarations.clone();
    for (k, v) in ctx.type_declarations.iter() {
        let _ = merged_td.insert(k.clone(), v.clone());
    }
    ctx.type_declarations = merged_td;

    let mut merged_sym = ctx.ambient_global_symbols.clone();
    for (k, v) in ctx.symbols.iter() {
        let _ = merged_sym.insert(k.clone(), v.clone());
    }
    ctx.set_symbols(merged_sym);

    for message in parsed.parser_errors {
        let diagnostic = Diagnostic::typescript_rust_parser_error(message, file_name.clone());
        ctx.push(diagnostic);
    }

    collect_type_declarations(&parsed.statements, &mut ctx);
    collect_global_augmentations_from_statements(&parsed.statements, &mut ctx);
    sync_global_this_symbol(&mut ctx);
    let mut merged_sym = ctx.ambient_global_symbols.clone();
    for (k, v) in ctx.symbols.iter() {
        let _ = merged_sym.insert(k.clone(), v.clone());
    }
    ctx.set_symbols(merged_sym);

    let current_type_declarations = ctx.type_declarations.clone();
    let current_symbols = ctx.symbols.clone();
    let validation_symbols = crate::modules::collect_exportable_value_symbols(
        &parsed.statements,
        &current_type_declarations,
        &current_symbols,
        &mut ctx,
    );
    let saved_symbols = std::mem::replace(&mut ctx.symbols, validation_symbols);

    validate_local_type_declarations(&parsed.statements, &file_name, &mut ctx);
    validate_direct_utility_aliases(&parsed.statements, &mut ctx);
    ctx.symbols = saved_symbols;

    for statement in parsed.statements {
        check_statement(statement, &mut ctx);
    }

    ctx.finish()
}

fn inject_generated_default_libs(ctx: &mut CheckerContext) {
    let default_lib_inputs = load_default_lib_inputs(ctx.options.no_lib, None);
    let original_file_name = ctx.file_name.clone();

    for input in default_lib_inputs {
        let _ = inject_generated_default_lib_snapshot_for_file_name(&input.file_name, ctx, None);
    }

    ctx.set_file_name(original_file_name);
}

pub(crate) fn collect_type_declarations(statements: &[ParsedStatement], ctx: &mut CheckerContext) {
    for statement in statements {
        collect_type_declarations_from_statement(statement, ctx);
    }
}

pub(crate) fn collect_global_augmentations_from_statements(
    statements: &[ParsedStatement],
    ctx: &mut CheckerContext,
) {
    for statement in statements {
        let ParsedStatement::DeclareModuleDeclaration(module) = statement else {
            continue;
        };

        if module.module_specifier != "global" {
            continue;
        }

        let saved_type_declarations = std::mem::replace(
            &mut ctx.type_declarations,
            crate::symbols::TypeDeclarationTable::new(),
        );
        let saved_symbols = std::mem::replace(&mut ctx.symbols, crate::symbols::SymbolTable::new());

        collect_type_declarations(&module.statements, ctx);
        let ambient_td = ctx.type_declarations.clone();
        for (name, decl) in ambient_td.iter() {
            let _ = ctx
                .ambient_global_type_declarations
                .insert(name.clone(), decl.clone());
        }

        let mut local_function_signatures = HashMap::new();
        let mut current_symbols = std::mem::take(&mut ctx.symbols);
        collect_function_signatures_from_statements(
            &module.statements,
            0,
            &mut current_symbols,
            &mut local_function_signatures,
            ctx,
        );
        ctx.symbols = current_symbols;

        for stmt in &module.statements {
            let var = match stmt {
                ParsedStatement::VariableDeclaration(var) => Some(var),
                ParsedStatement::ExportDeclaration(
                    typescript_rust_syntax::ParsedExportDeclaration::Statement {
                        declaration, ..
                    },
                ) => {
                    if let ParsedStatement::VariableDeclaration(var) = declaration.as_ref() {
                        Some(var)
                    } else {
                        None
                    }
                }
                _ => None,
            };

            if let Some(var) = var {
                let ty = var
                    .declared_type
                    .as_ref()
                    .map(|ty| crate::infer::map_parsed_type(ty.clone(), ctx))
                    .unwrap_or(typescript_rust_types::Type::Unknown);
                if ctx.ambient_global_symbols.get(&var.name).is_none() {
                    ctx.ambient_global_symbols.insert(
                        var.name.clone(),
                        crate::symbols::SymbolInfo {
                            ty,
                            kind: if matches!(
                                var.kind,
                                typescript_rust_syntax::ParsedVariableKind::Const
                            ) {
                                crate::symbols::SymbolKind::Const
                            } else {
                                crate::symbols::SymbolKind::Let
                            },
                            function_signature: None,
                        },
                    );
                }
            }
        }

        for (loc, fun_ty) in local_function_signatures {
            let name = match &module.statements[loc.statement_index] {
                ParsedStatement::FunctionDeclaration(f) => f.name.clone(),
                ParsedStatement::ExportDeclaration(
                    typescript_rust_syntax::ParsedExportDeclaration::Default {
                        declaration:
                            typescript_rust_syntax::ParsedDefaultExportDeclaration::Function(f),
                        ..
                    },
                ) => f.name.clone(),
                ParsedStatement::ExportDeclaration(
                    typescript_rust_syntax::ParsedExportDeclaration::Statement {
                        declaration, ..
                    },
                ) => {
                    if let ParsedStatement::FunctionDeclaration(f) = declaration.as_ref() {
                        f.name.clone()
                    } else {
                        "unknown".to_string()
                    }
                }
                _ => "unknown".to_string(),
            };

            if ctx.ambient_global_symbols.get(&name).is_none() {
                ctx.ambient_global_symbols.insert(
                    name,
                    crate::symbols::SymbolInfo {
                        ty: typescript_rust_types::Type::Function(fun_ty),
                        kind: crate::symbols::SymbolKind::Function,
                        function_signature: None,
                    },
                );
            }
        }

        ctx.type_declarations = saved_type_declarations;
        ctx.symbols = saved_symbols;
    }
}

pub(crate) fn sync_global_this_symbol(ctx: &mut CheckerContext) {
    use std::collections::BTreeMap;

    let mut properties = BTreeMap::new();
    for (name, symbol) in ctx.ambient_global_symbols.iter() {
        if name.as_ref() == "globalThis" {
            continue;
        }

        properties.insert(
            name.to_string(),
            typescript_rust_types::ObjectProperty::required(symbol.ty.clone()),
        );
    }

    ctx.ambient_global_symbols.insert(
        "globalThis".to_string(),
        crate::symbols::SymbolInfo {
            ty: typescript_rust_types::Type::Object(crate::arena::alloc_object_type(
                properties, None,
            )),
            kind: crate::symbols::SymbolKind::Const,
            function_signature: None,
        },
    );
}

pub(crate) fn validate_direct_utility_aliases(
    statements: &[ParsedStatement],
    ctx: &mut CheckerContext,
) {
    for statement in statements {
        validate_direct_utility_aliases_from_statement(statement, ctx);
    }
}

pub(crate) fn validate_local_type_declarations(
    statements: &[ParsedStatement],
    file_name: &str,
    ctx: &mut CheckerContext,
) {
    let mut local_declarations = Vec::new();
    let mut seen = HashSet::new();

    collect_local_type_declarations_from_statements(
        statements,
        file_name,
        &mut seen,
        &mut local_declarations,
        ctx,
    );

    for declaration in local_declarations.into_iter().rev() {
        validate_local_type_declaration(&declaration, ctx);
    }
}

fn collect_local_type_declarations_from_statements(
    statements: &[ParsedStatement],
    file_name: &str,
    seen: &mut HashSet<DeclarationResolutionKey>,
    local_declarations: &mut Vec<TypeDeclarationInfo>,
    ctx: &CheckerContext,
) {
    for statement in statements {
        collect_local_type_declarations_from_statement(
            statement,
            file_name,
            seen,
            local_declarations,
            ctx,
        );
    }
}

fn collect_local_type_declarations_from_statement(
    statement: &ParsedStatement,
    file_name: &str,
    seen: &mut HashSet<DeclarationResolutionKey>,
    local_declarations: &mut Vec<TypeDeclarationInfo>,
    ctx: &CheckerContext,
) {
    match statement {
        ParsedStatement::TypeAliasDeclaration(alias) => {
            let key = DeclarationResolutionKey {
                file_name: canonicalize_if_exists_string(std::path::Path::new(file_name)),
                name: alias.name.clone(),
                namespace: DeclarationNamespace::Type,
            };
            if seen.insert(key)
                && let Some(declaration) =
                    ctx.type_declarations
                        .iter()
                        .find_map(|(_, declaration)| match declaration {
                            TypeDeclarationInfo::Alias(info)
                                if info.name == alias.name
                                    && canonicalize_if_exists_string(std::path::Path::new(
                                        &info.file_name,
                                    )) == canonicalize_if_exists_string(
                                        std::path::Path::new(file_name),
                                    ) =>
                            {
                                Some(declaration)
                            }
                            _ => None,
                        })
            {
                local_declarations.push(attach_current_type_scope_if_missing(
                    declaration.clone(),
                    ctx,
                ));
            }
        }
        ParsedStatement::InterfaceDeclaration(interface) => {
            let key = DeclarationResolutionKey {
                file_name: canonicalize_if_exists_string(std::path::Path::new(file_name)),
                name: interface.name.clone(),
                namespace: DeclarationNamespace::Type,
            };
            if seen.insert(key)
                && let Some(declaration) =
                    ctx.type_declarations
                        .iter()
                        .find_map(|(_, declaration)| match declaration {
                            TypeDeclarationInfo::Interface(info)
                                if info.name == interface.name
                                    && canonicalize_if_exists_string(std::path::Path::new(
                                        &info.file_name,
                                    )) == canonicalize_if_exists_string(
                                        std::path::Path::new(file_name),
                                    ) =>
                            {
                                Some(declaration)
                            }
                            _ => None,
                        })
            {
                local_declarations.push(attach_current_type_scope_if_missing(
                    declaration.clone(),
                    ctx,
                ));
            }
        }
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Statement {
            declaration,
            ..
        }) => collect_local_type_declarations_from_statement(
            declaration.as_ref(),
            file_name,
            seen,
            local_declarations,
            ctx,
        ),
        _ => {}
    }
}

fn attach_current_type_scope_if_missing(
    declaration: TypeDeclarationInfo,
    ctx: &CheckerContext,
) -> TypeDeclarationInfo {
    let current_scope = ctx.type_declaration_scope.clone().unwrap_or_else(|| {
        Arc::new(crate::symbols::TypeDeclarationScope::new(vec![Arc::new(
            ctx.type_declarations.clone(),
        )]))
    });

    match declaration {
        TypeDeclarationInfo::Alias(mut alias) => {
            alias.resolution_scope = Some(current_scope);
            TypeDeclarationInfo::Alias(alias)
        }
        TypeDeclarationInfo::Interface(mut interface) => {
            interface.resolution_scope = Some(current_scope);
            TypeDeclarationInfo::Interface(interface)
        }
    }
}

fn collect_type_declarations_from_statement(statement: &ParsedStatement, ctx: &mut CheckerContext) {
    match statement {
        ParsedStatement::TypeAliasDeclaration(alias) => {
            collect_type_alias(alias, ctx);
        }
        ParsedStatement::InterfaceDeclaration(interface) => {
            collect_interface(interface, ctx);
        }
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Statement {
            declaration,
            ..
        }) => collect_type_declarations_from_statement(declaration.as_ref(), ctx),
        _ => {}
    }
}

fn check_statement(statement: ParsedStatement, ctx: &mut CheckerContext) {
    match statement {
        ParsedStatement::VariableDeclaration(variable) => {
            var::check_variable_declaration(variable, ctx);
        }
        ParsedStatement::Assignment(assignment) => {
            assign::check_assignment(assignment, ctx);
        }
        ParsedStatement::FunctionDeclaration(function) => {
            check_function::check_function_declaration(function, ctx);
        }
        ParsedStatement::Call(call) => {
            call::check_call(call, ctx);
        }
        ParsedStatement::Expression(expression) => {
            expr::check_expression_statement(expression, ctx);
        }
        ParsedStatement::TypeAliasDeclaration(_) => {}
        ParsedStatement::InterfaceDeclaration(_) => {}
        ParsedStatement::ImportDeclaration(import) => {
            if crate::modules::is_external_specifier(&import.module_specifier) {
                let suppress_unresolved_diagnostic =
                    matches!(
                        &import.kind,
                        typescript_rust_syntax::ParsedImportKind::SideEffect
                    ) && is_runtime_js_only_module(&import.module_specifier, ctx);

                if !ctx.options.stub_external_modules && !suppress_unresolved_diagnostic {
                    let mut diagnostic = match &import.kind {
                        typescript_rust_syntax::ParsedImportKind::SideEffect => {
                            Diagnostic::ts2882(&import.module_specifier, ctx.file_name.clone())
                        }
                        _ => Diagnostic::ts2307(&import.module_specifier, ctx.file_name.clone()),
                    };
                    if let Some(span) = import.module_specifier_span {
                        diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                    }
                    ctx.push(diagnostic);
                }

                // Stub the imports to avoid cascades in single-file mode
                match &import.kind {
                    typescript_rust_syntax::ParsedImportKind::Named {
                        specifiers,
                        is_type_only,
                    } => {
                        for specifier in specifiers {
                            if *is_type_only {
                                let declaration = crate::symbols::TypeDeclarationInfo::Alias(
                                    crate::symbols::TypeAliasInfo {
                                        name: specifier.local_name.to_string(),
                                        file_name: ctx.file_name.clone(),
                                        name_span: specifier.name_span,
                                        type_parameters: vec![],
                                        ty: typescript_rust_syntax::ParsedType::Unknown,
                                        resolution_scope: None,
                                    },
                                );
                                let _ = ctx
                                    .type_declarations
                                    .insert(specifier.local_name.clone(), declaration);
                            } else {
                                let declaration = crate::symbols::TypeDeclarationInfo::Alias(
                                    crate::symbols::TypeAliasInfo {
                                        name: specifier.local_name.to_string(),
                                        file_name: ctx.file_name.clone(),
                                        name_span: specifier.name_span,
                                        type_parameters: vec![],
                                        ty: typescript_rust_syntax::ParsedType::Unknown,
                                        resolution_scope: None,
                                    },
                                );
                                let _ = ctx
                                    .type_declarations
                                    .insert(specifier.local_name.clone(), declaration);
                                let _ = ctx.symbols.insert(
                                    specifier.local_name.clone(),
                                    crate::symbols::SymbolInfo {
                                        ty: typescript_rust_types::Type::Unknown,
                                        kind: crate::symbols::SymbolKind::Var,
                                        function_signature: None,
                                    },
                                );
                            }
                        }
                    }
                    typescript_rust_syntax::ParsedImportKind::DefaultAndNamed {
                        local_name,
                        name_span,
                        is_type_only,
                        specifiers,
                    } => {
                        if *is_type_only {
                            let declaration = crate::symbols::TypeDeclarationInfo::Alias(
                                crate::symbols::TypeAliasInfo {
                                    name: local_name.clone(),
                                    file_name: ctx.file_name.clone(),
                                    name_span: *name_span,
                                    type_parameters: vec![],
                                    ty: typescript_rust_syntax::ParsedType::Unknown,
                                    resolution_scope: None,
                                },
                            );
                            let _ = ctx
                                .type_declarations
                                .insert(local_name.clone(), declaration);
                        } else {
                            let _ = ctx.symbols.insert(
                                local_name.clone(),
                                crate::symbols::SymbolInfo {
                                    ty: typescript_rust_types::Type::Unknown,
                                    kind: crate::symbols::SymbolKind::Var,
                                    function_signature: None,
                                },
                            );
                        }

                        for specifier in specifiers {
                            if *is_type_only {
                                let declaration = crate::symbols::TypeDeclarationInfo::Alias(
                                    crate::symbols::TypeAliasInfo {
                                        name: specifier.local_name.to_string(),
                                        file_name: ctx.file_name.clone(),
                                        name_span: specifier.name_span,
                                        type_parameters: vec![],
                                        ty: typescript_rust_syntax::ParsedType::Unknown,
                                        resolution_scope: None,
                                    },
                                );
                                let _ = ctx
                                    .type_declarations
                                    .insert(specifier.local_name.clone(), declaration);
                            } else {
                                let declaration = crate::symbols::TypeDeclarationInfo::Alias(
                                    crate::symbols::TypeAliasInfo {
                                        name: specifier.local_name.to_string(),
                                        file_name: ctx.file_name.clone(),
                                        name_span: specifier.name_span,
                                        type_parameters: vec![],
                                        ty: typescript_rust_syntax::ParsedType::Unknown,
                                        resolution_scope: None,
                                    },
                                );
                                let _ = ctx
                                    .type_declarations
                                    .insert(specifier.local_name.clone(), declaration);
                                let _ = ctx.symbols.insert(
                                    specifier.local_name.clone(),
                                    crate::symbols::SymbolInfo {
                                        ty: typescript_rust_types::Type::Unknown,
                                        kind: crate::symbols::SymbolKind::Var,
                                        function_signature: None,
                                    },
                                );
                            }
                        }
                    }
                    typescript_rust_syntax::ParsedImportKind::Default { local_name, .. } => {
                        let _ = ctx.symbols.insert(
                            local_name.clone(),
                            crate::symbols::SymbolInfo {
                                ty: typescript_rust_types::Type::Unknown,
                                kind: crate::symbols::SymbolKind::Var,
                                function_signature: None,
                            },
                        );
                    }
                    typescript_rust_syntax::ParsedImportKind::Namespace {
                        local_name,
                        is_type_only,
                        ..
                    } => {
                        if *is_type_only {
                            let declaration = crate::symbols::TypeDeclarationInfo::Alias(
                                crate::symbols::TypeAliasInfo {
                                    name: local_name.clone(),
                                    file_name: ctx.file_name.clone(),
                                    name_span: None,
                                    type_parameters: vec![],
                                    ty: typescript_rust_syntax::ParsedType::Unknown,
                                    resolution_scope: None,
                                },
                            );
                            let _ = ctx
                                .type_declarations
                                .insert(local_name.clone(), declaration);
                        } else {
                            let _ = ctx.symbols.insert(
                                local_name.clone(),
                                crate::symbols::SymbolInfo {
                                    ty: typescript_rust_types::Type::Unknown,
                                    kind: crate::symbols::SymbolKind::Const,
                                    function_signature: None,
                                },
                            );
                        }
                    }
                    _ => {}
                }
            } else {
                let mut diagnostic =
                    Diagnostic::typescript_rust_unsupported_module_syntax(ctx.file_name.clone());

                if let Some(span) = import.span.or(import.module_specifier_span) {
                    diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                }

                ctx.push(diagnostic);
            }
        }
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Statement {
            declaration,
            ..
        }) => check_statement(*declaration, ctx),
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Named {
            module_specifier: Some(specifier),
            span,
            module_specifier_span,
            ..
        }) => {
            if crate::modules::is_external_specifier(&specifier) {
                if !ctx.options.stub_external_modules {
                    let mut diagnostic = Diagnostic::ts2307(&specifier, ctx.file_name.clone());
                    if let Some(span) = module_specifier_span {
                        diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                    }
                    ctx.push(diagnostic);
                }
            } else {
                let mut diagnostic =
                    Diagnostic::typescript_rust_unsupported_module_syntax(ctx.file_name.clone());

                if let Some(span) = span {
                    diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                }

                ctx.push(diagnostic);
            }
        }
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Named { .. }) => {}
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Namespace { .. }) => {}
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Default {
            declaration,
            span,
        }) => match declaration {
            ParsedDefaultExportDeclaration::Function(function) => {
                check_function::check_function_declaration(function, ctx);
            }
            ParsedDefaultExportDeclaration::Class { .. } => {}
            ParsedDefaultExportDeclaration::Expression(expression) => {
                expr::check_expression_statement(expression, ctx);
            }
            ParsedDefaultExportDeclaration::Unsupported { .. } => {
                let mut diagnostic =
                    Diagnostic::typescript_rust_unsupported_module_syntax(ctx.file_name.clone());

                if let Some(span) = span {
                    diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                }

                ctx.push(diagnostic);
            }
        },
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::All {
            module_specifier,
            span,
            module_specifier_span,
            ..
        }) => {
            if crate::modules::is_external_specifier(&module_specifier) {
                if !ctx.options.stub_external_modules {
                    let mut diagnostic =
                        Diagnostic::ts2307(&module_specifier, ctx.file_name.clone());
                    if let Some(span) = module_specifier_span {
                        diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                    }
                    ctx.push(diagnostic);
                }
            } else {
                let mut diagnostic =
                    Diagnostic::typescript_rust_unsupported_module_syntax(ctx.file_name.clone());

                if let Some(span) = span {
                    diagnostic = diagnostic.with_span(crate::context::convert_span(span));
                }

                ctx.push(diagnostic);
            }
        }
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Empty { .. }) => {}
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Equals { .. }) => {}
        ParsedStatement::DeclareModuleDeclaration(_) => {}
        ParsedStatement::UnsupportedDeclaration { span } => {
            let mut diag =
                typescript_rust_diagnostics::Diagnostic::typescript_rust_unsupported_declaration(
                    ctx.file_name.clone(),
                );
            if let Some(s) = span {
                diag = diag.with_span(crate::context::convert_span(s));
            }
            ctx.push(diag);
        }
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Unsupported { span }) => {
            let mut diagnostic =
                Diagnostic::typescript_rust_unsupported_module_syntax(ctx.file_name.clone());

            if let Some(span) = span {
                diagnostic = diagnostic.with_span(crate::context::convert_span(span));
            }

            ctx.push(diagnostic);
        }
    }
}

fn is_runtime_js_only_module(module_specifier: &str, ctx: &CheckerContext) -> bool {
    let Some(resolved_path) = ctx.options.resolved_modules.get(module_specifier) else {
        return false;
    };

    let lower = resolved_path.to_ascii_lowercase();
    lower.ends_with(".js")
        || lower.ends_with(".jsx")
        || lower.ends_with(".mjs")
        || lower.ends_with(".cjs")
}

fn validate_direct_utility_aliases_from_statement(
    statement: &ParsedStatement,
    ctx: &mut CheckerContext,
) {
    match statement {
        ParsedStatement::TypeAliasDeclaration(alias) => {
            validate_direct_utility_alias(alias, ctx);
        }
        ParsedStatement::ExportDeclaration(ParsedExportDeclaration::Statement {
            declaration,
            ..
        }) => validate_direct_utility_aliases_from_statement(declaration.as_ref(), ctx),
        _ => {}
    }
}

fn validate_direct_utility_alias(alias: &ParsedTypeAliasDeclaration, ctx: &mut CheckerContext) {
    if ctx.current_file_kind == crate::context::FileKind::GeneratedDeclaration {
        return;
    }

    let ParsedType::Named(named_type) = &alias.ty else {
        return;
    };

    if !matches!(
        named_type.name.as_str(),
        "Record" | "Partial" | "Pick" | "Omit"
    ) {
        return;
    }

    let _ = crate::infer::map_parsed_type_with_substitution(
        alias.ty.clone(),
        ctx,
        &crate::infer::TypeParameterSubstitution::new(),
    );
}

fn classify_file_kind(file_name: &str) -> FileKind {
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".d.ts") || lower.ends_with(".d.mts") || lower.ends_with(".d.cts") {
        return FileKind::RootDeclaration;
    }

    FileKind::RootSource
}

pub(crate) fn collect_type_alias(alias: &ParsedTypeAliasDeclaration, ctx: &mut CheckerContext) {
    report_duplicate_type_parameters(&alias.type_parameters, ctx);

    let info = TypeAliasInfo {
        name: alias.name.clone(),
        file_name: ctx.file_name.clone(),
        name_span: alias.name_span,
        type_parameters: alias.type_parameters.clone(),
        ty: alias.ty.clone(),
        resolution_scope: None,
    };

    if ctx
        .type_declarations
        .insert(alias.name.clone(), TypeDeclarationInfo::Alias(info))
        .is_some()
    {
        let mut diagnostic = Diagnostic::ts2300(&alias.name, ctx.file_name.clone());

        if let Some(span) = alias.name_span {
            diagnostic = diagnostic.with_span(crate::context::convert_span(span));
        }

        ctx.push(diagnostic);
    }
}

pub(crate) fn collect_interface(interface: &ParsedInterfaceDeclaration, ctx: &mut CheckerContext) {
    report_duplicate_type_parameters(&interface.type_parameters, ctx);

    let info = InterfaceInfo {
        name: interface.name.clone(),
        file_name: ctx.file_name.clone(),
        name_span: interface.name_span,
        type_parameters: interface.type_parameters.clone(),
        extends: interface.extends.clone(),
        members: interface.members.clone(),
        resolution_scope: None,
    };

    if ctx
        .type_declarations
        .insert(interface.name.clone(), TypeDeclarationInfo::Interface(info))
        .is_some()
    {
        let mut diagnostic = Diagnostic::ts2300(&interface.name, ctx.file_name.clone());

        if let Some(span) = interface.name_span {
            diagnostic = diagnostic.with_span(crate::context::convert_span(span));
        }

        ctx.push(diagnostic);
    }
}
