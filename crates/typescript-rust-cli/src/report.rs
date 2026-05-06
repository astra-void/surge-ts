use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use serde_json::{Map, Value};
use typescript_rust_checker::CompatibilityStats;
use typescript_rust_config::LoadedTsConfig;
use typescript_rust_diagnostics::{
    Diagnostic, DiagnosticCoverageStats, catalog_coverage_stats, render_diagnostics,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum ReportFormat {
    Text,
    Json,
}

#[derive(Debug, Clone)]
pub struct CompatReportCountEntry {
    pub key: String,
    pub count: usize,
}

#[derive(Debug, Clone)]
pub struct CompatReportParserErrorEntry {
    pub file_name: String,
    pub message: String,
    pub count: usize,
}

#[derive(Debug, Clone)]
pub struct CompatReportCategorizedCountEntry {
    pub key: String,
    pub category: String,
    pub count: usize,
}

#[derive(Debug, Clone)]
pub struct CompatReportModuleExportCountEntry {
    pub module_specifier: String,
    pub export_name: String,
    pub count: usize,
}

#[derive(Debug, Clone)]
pub struct ProjectCompatibilityReport {
    pub root_dir: String,
    pub files_loaded: usize,
    pub visibility_warning: Option<String>,
    pub diagnostics_total: usize,
    pub loaded_source_files: usize,
    pub loaded_root_declaration_files: usize,
    pub loaded_dependency_declaration_files: usize,
    pub loaded_generated_declaration_files: usize,
    pub suppressed_declaration_diagnostics_total: usize,
    pub suppressed_rust_only_diagnostics_total: usize,
    pub diagnostics_root_source_total: usize,
    pub diagnostics_root_declaration_total: usize,
    pub diagnostics_dependency_declaration_total: usize,
    pub diagnostics_generated_declaration_total: usize,
    pub diagnostics_by_file_kind: Vec<CompatReportCountEntry>,
    pub by_code: Vec<CompatReportCountEntry>,
    pub by_file: Vec<CompatReportCountEntry>,
    pub ts2305_by_module_and_export: Vec<CompatReportModuleExportCountEntry>,
    pub ts2307_by_module_specifier: Vec<CompatReportCategorizedCountEntry>,
    pub ts2304_by_identifier: Vec<CompatReportCategorizedCountEntry>,
    pub node_modules_source_diagnostics_total: usize,
    pub node_modules_source_diagnostics_by_prefix: Vec<CompatReportCountEntry>,
    pub parser_errors: Vec<CompatReportParserErrorEntry>,
    pub external_module_stubs_total: usize,
    pub external_module_stubs: Vec<CompatReportCountEntry>,
    pub declaration_files_loaded: usize,
    pub ambient_external_modules: Vec<String>,
    pub diagnostic_coverage: DiagnosticCoverageStats,
}

fn is_relative_specifier(specifier: &str) -> bool {
    specifier == "."
        || specifier == ".."
        || specifier.starts_with("./")
        || specifier.starts_with("../")
        || specifier.starts_with(".\\")
        || specifier.starts_with("..\\")
}

pub fn build_project_compatibility_report(
    loaded: &LoadedTsConfig,
    diagnostics: &[Diagnostic],
    sources: &[(PathBuf, String, String)],
    stats: &CompatibilityStats,
) -> ProjectCompatibilityReport {
    let mut by_code = HashMap::<String, usize>::new();
    let mut by_file = HashMap::<String, usize>::new();
    let mut parser_errors = HashMap::<(String, String), usize>::new();
    let mut ts2305_by_module_and_export = HashMap::<(String, String), usize>::new();
    let mut ts2307_by_module_specifier = HashMap::<(String, String), usize>::new();
    let mut ts2304_by_identifier = HashMap::<(String, String), usize>::new();
    let mut node_modules_source_diagnostics_by_prefix = HashMap::<String, usize>::new();
    let mut external_module_stubs = HashMap::<String, usize>::new();
    let mut external_module_stubs_total = 0;
    let mut declaration_files_loaded = 0;
    let mut loaded_source_files = 0;
    let mut loaded_root_declaration_files = 0;
    let mut loaded_dependency_declaration_files = 0;
    let mut loaded_generated_declaration_files = 0;
    let mut diagnostics_root_source_total = 0;
    let mut diagnostics_root_declaration_total = 0;
    let mut diagnostics_dependency_declaration_total = 0;
    let mut diagnostics_generated_declaration_total = 0;
    let mut node_modules_source_diagnostics_total = 0;
    let mut diagnostics_by_file_kind = HashMap::<String, usize>::new();
    let mut ambient_external_modules_set = std::collections::HashSet::new();

    for (_, file_name, source_text) in sources {
        match classify_file_kind(file_name) {
            FileKindLabel::RootSource => loaded_source_files += 1,
            FileKindLabel::RootDeclaration => loaded_root_declaration_files += 1,
            FileKindLabel::DependencyDeclaration => loaded_dependency_declaration_files += 1,
            FileKindLabel::GeneratedDeclaration => loaded_generated_declaration_files += 1,
        }

        if is_declaration_file_name(file_name) {
            declaration_files_loaded += 1;
        }
        let parsed = typescript_rust_syntax::parse_source(source_text, file_name);
        for statement in parsed.statements {
            match statement {
                typescript_rust_syntax::ParsedStatement::ImportDeclaration(import) => {
                    if !is_relative_specifier(&import.module_specifier) {
                        *external_module_stubs
                            .entry(import.module_specifier.clone())
                            .or_default() += 1;
                        external_module_stubs_total += 1;
                    }
                }
                typescript_rust_syntax::ParsedStatement::DeclareModuleDeclaration(
                    declare_module,
                ) => {
                    ambient_external_modules_set.insert(declare_module.module_specifier.clone());
                }
                typescript_rust_syntax::ParsedStatement::ExportDeclaration(export) => {
                    let module_specifier = match &export {
                        typescript_rust_syntax::ParsedExportDeclaration::Named {
                            module_specifier,
                            ..
                        } => module_specifier.as_deref(),
                        typescript_rust_syntax::ParsedExportDeclaration::All {
                            module_specifier,
                            ..
                        } => Some(module_specifier.as_str()),
                        _ => None,
                    };
                    if let Some(spec) = module_specifier {
                        if !is_relative_specifier(spec) {
                            *external_module_stubs.entry(spec.to_string()).or_default() += 1;
                            external_module_stubs_total += 1;
                        }
                    }
                }
                _ => {}
            }
        }
    }

    for diagnostic in diagnostics {
        let code = diagnostic.code.to_string();
        *by_code.entry(code.clone()).or_default() += 1;
        let file_label = report_path_label(&loaded.root_dir, &diagnostic.file_name);
        *by_file.entry(file_label.clone()).or_default() += 1;

        match classify_file_kind(&diagnostic.file_name) {
            FileKindLabel::RootSource => diagnostics_root_source_total += 1,
            FileKindLabel::RootDeclaration => diagnostics_root_declaration_total += 1,
            FileKindLabel::DependencyDeclaration => diagnostics_dependency_declaration_total += 1,
            FileKindLabel::GeneratedDeclaration => diagnostics_generated_declaration_total += 1,
        }
        *diagnostics_by_file_kind
            .entry(
                classify_file_kind(&diagnostic.file_name)
                    .label()
                    .to_string(),
            )
            .or_default() += 1;

        if is_node_modules_source_file(&diagnostic.file_name) {
            node_modules_source_diagnostics_total += 1;
            if let Some(prefix) = node_modules_source_prefix(&diagnostic.file_name) {
                *node_modules_source_diagnostics_by_prefix
                    .entry(prefix)
                    .or_default() += 1;
            }
        }

        if code == "typescript-rust::parser-error" {
            *parser_errors
                .entry((file_label, diagnostic.message.clone()))
                .or_default() += 1;
        }

        match code.as_str() {
            "TS2305" => {
                if let Some((module_specifier, export_name)) =
                    extract_ts2305_module_export(&diagnostic.message)
                {
                    *ts2305_by_module_and_export
                        .entry((module_specifier, export_name))
                        .or_default() += 1;
                }
            }
            "TS2307" => {
                if let Some(specifier) = extract_ts2307_module_specifier(&diagnostic.message) {
                    let category = classify_ts2307_module_specifier(&specifier);
                    *ts2307_by_module_specifier
                        .entry((specifier, category.to_string()))
                        .or_default() += 1;
                }
            }
            "TS2304" => {
                if let Some(identifier) = extract_ts2304_identifier(&diagnostic.message) {
                    let category = classify_ts2304_identifier(&identifier);
                    *ts2304_by_identifier
                        .entry((identifier, category.to_string()))
                        .or_default() += 1;
                }
            }
            _ => {}
        }
    }

    ProjectCompatibilityReport {
        root_dir: loaded.root_dir.display().to_string(),
        files_loaded: loaded.files.len(),
        visibility_warning: if loaded.files.is_empty() {
            Some("no source files were discovered for the project".to_string())
        } else {
            None
        },
        diagnostics_total: diagnostics.len(),
        loaded_source_files,
        loaded_root_declaration_files,
        loaded_dependency_declaration_files,
        loaded_generated_declaration_files,
        suppressed_declaration_diagnostics_total: stats.suppressed_declaration_diagnostics_total,
        suppressed_rust_only_diagnostics_total: stats.suppressed_rust_only_diagnostics_total,
        diagnostics_root_source_total,
        diagnostics_root_declaration_total,
        diagnostics_dependency_declaration_total,
        diagnostics_generated_declaration_total,
        diagnostics_by_file_kind: sort_counts(diagnostics_by_file_kind),
        by_code: sort_counts(by_code),
        by_file: sort_counts(by_file),
        ts2305_by_module_and_export: sort_module_export_counts(ts2305_by_module_and_export),
        ts2307_by_module_specifier: sort_categorized_counts(ts2307_by_module_specifier),
        ts2304_by_identifier: sort_categorized_counts(ts2304_by_identifier),
        node_modules_source_diagnostics_total,
        node_modules_source_diagnostics_by_prefix: sort_counts(
            node_modules_source_diagnostics_by_prefix,
        ),
        parser_errors: sort_parser_errors(parser_errors),
        external_module_stubs_total,
        external_module_stubs: sort_counts(external_module_stubs),
        declaration_files_loaded,
        ambient_external_modules: {
            let mut list: Vec<_> = ambient_external_modules_set.into_iter().collect();
            list.sort();
            list
        },
        diagnostic_coverage: catalog_coverage_stats(),
    }
}

pub fn render_project_diagnostics_json(
    loaded: &LoadedTsConfig,
    diagnostics: &[Diagnostic],
    sources: &[(PathBuf, String, String)],
    max_diagnostics: Option<usize>,
) -> Value {
    let limit = max_diagnostics.unwrap_or(usize::MAX);
    let diagnostics = diagnostics
        .iter()
        .take(limit)
        .map(|diagnostic| render_diagnostic_json(loaded, diagnostic, sources))
        .collect::<Vec<_>>();

    let mut root = Map::new();
    root.insert("diagnostics".to_string(), Value::Array(diagnostics));

    Value::Object(root)
}

pub fn render_project_compatibility_report_text(report: &ProjectCompatibilityReport) -> String {
    let mut lines = Vec::new();
    lines.push("Compatibility report".to_string());
    lines.push(format!("Root dir: {}", report.root_dir));
    lines.push(format!("Files loaded: {}", report.files_loaded));
    lines.push(format!(
        "Loaded source files: {}",
        report.loaded_source_files
    ));
    lines.push(format!(
        "Loaded root declarations: {}",
        report.loaded_root_declaration_files
    ));
    lines.push(format!(
        "Loaded dependency declarations: {}",
        report.loaded_dependency_declaration_files
    ));
    lines.push(format!(
        "Loaded generated declarations: {}",
        report.loaded_generated_declaration_files
    ));
    if let Some(warning) = &report.visibility_warning {
        lines.push(format!("Visibility warning: {warning}"));
    }
    lines.push(format!("Diagnostics: {}", report.diagnostics_total));
    lines.push(format!(
        "Suppressed declaration diagnostics: {}",
        report.suppressed_declaration_diagnostics_total
    ));
    lines.push(format!(
        "Suppressed Rust-only diagnostics: {}",
        report.suppressed_rust_only_diagnostics_total
    ));
    lines.push(String::new());
    lines.push("Diagnostics by file kind:".to_string());
    if report.diagnostics_by_file_kind.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        for entry in &report.diagnostics_by_file_kind {
            lines.push(format!("{}  {}", entry.key, entry.count));
        }
    }
    lines.push(String::new());
    lines.push(format!(
        "Diagnostics from root source files: {}",
        report.diagnostics_root_source_total
    ));
    lines.push(format!(
        "Diagnostics from root declaration files: {}",
        report.diagnostics_root_declaration_total
    ));
    lines.push(format!(
        "Diagnostics from dependency declarations: {}",
        report.diagnostics_dependency_declaration_total
    ));
    lines.push(format!(
        "Diagnostics from generated declarations: {}",
        report.diagnostics_generated_declaration_total
    ));
    lines.push(String::new());
    lines.push("Diagnostic coverage:".to_string());
    lines.push(format!(
        "  catalog total: {}",
        report.diagnostic_coverage.catalog_total
    ));
    lines.push(format!(
        "  emitted total: {}",
        report.diagnostic_coverage.emitted_total
    ));
    lines.push(format!(
        "  catalog-only total: {}",
        report.diagnostic_coverage.catalog_only_total
    ));
    lines.push(format!(
        "  emitted TypeScript diagnostics: {}",
        report.diagnostic_coverage.emitted_typescript_total
    ));
    lines.push(format!(
        "  catalog-only TypeScript diagnostics: {}",
        report.diagnostic_coverage.catalog_only_typescript_total
    ));
    lines.push(String::new());
    lines.push("By code:".to_string());
    if report.by_code.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        for entry in &report.by_code {
            lines.push(format!("{}  {}", entry.key, entry.count));
        }
    }
    lines.push(String::new());
    lines.push("By file:".to_string());
    if report.by_file.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        for entry in &report.by_file {
            lines.push(format!("{}  {}", entry.key, entry.count));
        }
    }

    lines.push(String::new());
    lines.push("TS2305 by module/export:".to_string());
    if report.ts2305_by_module_and_export.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        for entry in &report.ts2305_by_module_and_export {
            lines.push(format!(
                "{} :: {}  {}",
                entry.module_specifier, entry.export_name, entry.count
            ));
        }
    }

    lines.push(String::new());
    lines.push("TS2307 by module specifier:".to_string());
    if report.ts2307_by_module_specifier.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        for entry in &report.ts2307_by_module_specifier {
            lines.push(format!(
                "{} [{}]  {}",
                entry.key, entry.category, entry.count
            ));
        }
    }

    lines.push(String::new());
    lines.push("TS2304 by identifier:".to_string());
    if report.ts2304_by_identifier.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        for entry in &report.ts2304_by_identifier {
            lines.push(format!(
                "{} [{}]  {}",
                entry.key, entry.category, entry.count
            ));
        }
    }

    lines.push(String::new());
    lines.push(format!(
        "Node_modules source diagnostics: {}",
        report.node_modules_source_diagnostics_total
    ));
    lines.push("Node_modules source diagnostics by package/source prefix:".to_string());
    if report.node_modules_source_diagnostics_by_prefix.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        for entry in &report.node_modules_source_diagnostics_by_prefix {
            lines.push(format!("{}  {}", entry.key, entry.count));
        }
    }

    lines.push(String::new());
    lines.push(format!("Parser errors: {}", parser_error_total(report)));
    lines.push("Top parser errors:".to_string());
    if report.parser_errors.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        for entry in &report.parser_errors {
            lines.push(format!(
                "{}: {}  {}",
                entry.file_name, entry.message, entry.count
            ));
        }
    }

    lines.push(String::new());
    lines.push(format!(
        "External module stubs: {}",
        report.external_module_stubs_total
    ));
    lines.push("External modules:".to_string());
    if report.external_module_stubs.is_empty() {
        lines.push("  (none)".to_string());
    } else {
        for entry in &report.external_module_stubs {
            lines.push(format!("{}  {}", entry.key, entry.count));
        }
    }

    if report.declaration_files_loaded > 0 {
        lines.push(String::new());
        lines.push(format!(
            "Declaration files loaded: {}",
            report.declaration_files_loaded
        ));
        if !report.ambient_external_modules.is_empty() {
            lines.push(format!(
                "Ambient external modules: {}",
                report.ambient_external_modules.len()
            ));
        }
    }

    lines.join("\n")
}

pub fn render_project_compatibility_report_json(report: &ProjectCompatibilityReport) -> Value {
    let mut root = Map::new();
    root.insert(
        "rootDir".to_string(),
        Value::String(report.root_dir.clone()),
    );
    root.insert(
        "filesLoaded".to_string(),
        Value::from(report.files_loaded as u64),
    );
    if let Some(warning) = &report.visibility_warning {
        root.insert(
            "visibilityWarning".to_string(),
            Value::String(warning.clone()),
        );
    }
    root.insert(
        "diagnosticsTotal".to_string(),
        Value::from(report.diagnostics_total as u64),
    );
    root.insert(
        "loadedSourceFiles".to_string(),
        Value::from(report.loaded_source_files as u64),
    );
    root.insert(
        "loadedRootDeclarationFiles".to_string(),
        Value::from(report.loaded_root_declaration_files as u64),
    );
    root.insert(
        "loadedDependencyDeclarationFiles".to_string(),
        Value::from(report.loaded_dependency_declaration_files as u64),
    );
    root.insert(
        "loadedGeneratedDeclarationFiles".to_string(),
        Value::from(report.loaded_generated_declaration_files as u64),
    );
    root.insert(
        "suppressedDeclarationDiagnosticsTotal".to_string(),
        Value::from(report.suppressed_declaration_diagnostics_total as u64),
    );
    root.insert(
        "suppressedRustOnlyDiagnosticsTotal".to_string(),
        Value::from(report.suppressed_rust_only_diagnostics_total as u64),
    );
    root.insert(
        "diagnosticsRootSourceTotal".to_string(),
        Value::from(report.diagnostics_root_source_total as u64),
    );
    root.insert(
        "diagnosticsRootDeclarationTotal".to_string(),
        Value::from(report.diagnostics_root_declaration_total as u64),
    );
    root.insert(
        "diagnosticsDependencyDeclarationTotal".to_string(),
        Value::from(report.diagnostics_dependency_declaration_total as u64),
    );
    root.insert(
        "diagnosticsGeneratedDeclarationTotal".to_string(),
        Value::from(report.diagnostics_generated_declaration_total as u64),
    );
    root.insert(
        "diagnosticsByFileKind".to_string(),
        Value::Array(
            report
                .diagnostics_by_file_kind
                .iter()
                .map(|entry| {
                    let mut item = Map::new();
                    item.insert("kind".to_string(), Value::String(entry.key.clone()));
                    item.insert("count".to_string(), Value::from(entry.count as u64));
                    Value::Object(item)
                })
                .collect(),
        ),
    );

    let mut coverage_map = Map::new();
    coverage_map.insert(
        "catalogTotal".to_string(),
        Value::from(report.diagnostic_coverage.catalog_total as u64),
    );
    coverage_map.insert(
        "emittedTotal".to_string(),
        Value::from(report.diagnostic_coverage.emitted_total as u64),
    );
    coverage_map.insert(
        "catalogOnlyTotal".to_string(),
        Value::from(report.diagnostic_coverage.catalog_only_total as u64),
    );
    coverage_map.insert(
        "emittedTypeScriptTotal".to_string(),
        Value::from(report.diagnostic_coverage.emitted_typescript_total as u64),
    );
    coverage_map.insert(
        "catalogOnlyTypeScriptTotal".to_string(),
        Value::from(report.diagnostic_coverage.catalog_only_typescript_total as u64),
    );
    root.insert(
        "diagnosticCoverage".to_string(),
        Value::Object(coverage_map),
    );

    root.insert(
        "byCode".to_string(),
        Value::Array(
            report
                .by_code
                .iter()
                .map(|entry| {
                    let mut item = Map::new();
                    item.insert("code".to_string(), Value::String(entry.key.clone()));
                    item.insert("count".to_string(), Value::from(entry.count as u64));
                    Value::Object(item)
                })
                .collect(),
        ),
    );
    root.insert(
        "byFile".to_string(),
        Value::Array(
            report
                .by_file
                .iter()
                .map(|entry| {
                    let mut item = Map::new();
                    item.insert("fileName".to_string(), Value::String(entry.key.clone()));
                    item.insert("count".to_string(), Value::from(entry.count as u64));
                    Value::Object(item)
                })
                .collect(),
        ),
    );
    root.insert(
        "ts2305ByModuleAndExport".to_string(),
        Value::Array(
            report
                .ts2305_by_module_and_export
                .iter()
                .map(|entry| {
                    let mut item = Map::new();
                    item.insert(
                        "moduleSpecifier".to_string(),
                        Value::String(entry.module_specifier.clone()),
                    );
                    item.insert(
                        "exportName".to_string(),
                        Value::String(entry.export_name.clone()),
                    );
                    item.insert("count".to_string(), Value::from(entry.count as u64));
                    Value::Object(item)
                })
                .collect(),
        ),
    );
    root.insert(
        "ts2307ByModuleSpecifier".to_string(),
        Value::Array(
            report
                .ts2307_by_module_specifier
                .iter()
                .map(|entry| {
                    let mut item = Map::new();
                    item.insert("specifier".to_string(), Value::String(entry.key.clone()));
                    item.insert(
                        "category".to_string(),
                        Value::String(entry.category.clone()),
                    );
                    item.insert("count".to_string(), Value::from(entry.count as u64));
                    Value::Object(item)
                })
                .collect(),
        ),
    );
    root.insert(
        "ts2304ByIdentifier".to_string(),
        Value::Array(
            report
                .ts2304_by_identifier
                .iter()
                .map(|entry| {
                    let mut item = Map::new();
                    item.insert("identifier".to_string(), Value::String(entry.key.clone()));
                    item.insert(
                        "category".to_string(),
                        Value::String(entry.category.clone()),
                    );
                    item.insert("count".to_string(), Value::from(entry.count as u64));
                    Value::Object(item)
                })
                .collect(),
        ),
    );
    let mut node_modules_source_json = Map::new();
    node_modules_source_json.insert(
        "total".to_string(),
        Value::from(report.node_modules_source_diagnostics_total as u64),
    );
    node_modules_source_json.insert(
        "byPrefix".to_string(),
        Value::Array(
            report
                .node_modules_source_diagnostics_by_prefix
                .iter()
                .map(|entry| {
                    let mut item = Map::new();
                    item.insert("prefix".to_string(), Value::String(entry.key.clone()));
                    item.insert("count".to_string(), Value::from(entry.count as u64));
                    Value::Object(item)
                })
                .collect(),
        ),
    );
    root.insert(
        "nodeModulesSourceDiagnostics".to_string(),
        Value::Object(node_modules_source_json),
    );
    root.insert(
        "parserErrors".to_string(),
        Value::Array(
            report
                .parser_errors
                .iter()
                .map(|entry| {
                    let mut item = Map::new();
                    item.insert(
                        "fileName".to_string(),
                        Value::String(entry.file_name.clone()),
                    );
                    item.insert("message".to_string(), Value::String(entry.message.clone()));
                    item.insert("count".to_string(), Value::from(entry.count as u64));
                    Value::Object(item)
                })
                .collect(),
        ),
    );

    let mut stubs_json = Map::new();
    stubs_json.insert(
        "total".to_string(),
        Value::from(report.external_module_stubs_total as u64),
    );
    stubs_json.insert(
        "bySpecifier".to_string(),
        Value::Array(
            report
                .external_module_stubs
                .iter()
                .map(|entry| {
                    let mut item = Map::new();
                    item.insert("specifier".to_string(), Value::String(entry.key.clone()));
                    item.insert("count".to_string(), Value::from(entry.count as u64));
                    Value::Object(item)
                })
                .collect(),
        ),
    );
    root.insert("externalModuleStubs".to_string(), Value::Object(stubs_json));

    if report.declaration_files_loaded > 0 {
        root.insert(
            "declarationFilesLoaded".to_string(),
            Value::from(report.declaration_files_loaded as u64),
        );
        root.insert(
            "ambientExternalModules".to_string(),
            Value::Array(
                report
                    .ambient_external_modules
                    .iter()
                    .map(|s| Value::String(s.clone()))
                    .collect(),
            ),
        );
    }

    Value::Object(root)
}

pub fn render_project_diagnostics_preview(
    diagnostics: &[Diagnostic],
    sources: &[(PathBuf, String, String)],
    show_spans: bool,
    max_diagnostics: Option<usize>,
) -> String {
    if diagnostics.is_empty() {
        return String::new();
    }

    let mut diagnostics_by_file: HashMap<String, Vec<Diagnostic>> = HashMap::new();
    for diagnostic in diagnostics {
        diagnostics_by_file
            .entry(diagnostic.file_name.clone())
            .or_default()
            .push(diagnostic.clone());
    }

    let limit = max_diagnostics.unwrap_or(usize::MAX);
    let mut rendered = Vec::new();
    let mut rendered_count = 0usize;
    let mut truncated = false;

    for (file_path, file_name, source_text) in sources {
        if rendered_count >= limit {
            truncated = diagnostics.len() > rendered_count;
            break;
        }

        let Some(file_diagnostics) = diagnostics_by_file.remove(file_name) else {
            continue;
        };

        let block = render_diagnostic_block(
            file_path.as_path(),
            source_text,
            &file_diagnostics,
            show_spans,
            limit.saturating_sub(rendered_count),
            &mut rendered_count,
        );

        if !block.is_empty() {
            rendered.push(block);
        }

        if rendered_count >= limit {
            truncated = diagnostics.len() > rendered_count || !diagnostics_by_file.is_empty();
            break;
        }
    }

    if !diagnostics_by_file.is_empty() && rendered_count < limit {
        let mut remaining = diagnostics_by_file.into_iter().collect::<Vec<_>>();
        remaining.sort_by(|left, right| left.0.cmp(&right.0));

        for (file_name, file_diagnostics) in remaining {
            if rendered_count >= limit {
                truncated = diagnostics.len() > rendered_count;
                break;
            }

            let block = render_diagnostic_block(
                Path::new(&file_name),
                "",
                &file_diagnostics,
                show_spans,
                limit.saturating_sub(rendered_count),
                &mut rendered_count,
            );

            if !block.is_empty() {
                rendered.push(block);
            }
        }

        truncated = truncated || diagnostics.len() > rendered_count;
    }

    if truncated {
        rendered.push(format!(
            "Showing first {} of {} diagnostics.",
            rendered_count,
            diagnostics.len()
        ));
    }

    rendered.join("\n\n")
}

fn render_diagnostic_block(
    file_path: &Path,
    source_text: &str,
    diagnostics: &[Diagnostic],
    show_spans: bool,
    remaining_limit: usize,
    rendered_count: &mut usize,
) -> String {
    if diagnostics.is_empty() || remaining_limit == 0 {
        return String::new();
    }

    let take = remaining_limit.min(diagnostics.len());
    let diagnostics = &diagnostics[..take];
    *rendered_count += take;

    let rendered = if show_spans {
        render_diagnostics_with_spans(diagnostics, source_text)
    } else {
        render_diagnostics(diagnostics, source_text)
    };

    format!("{}\n{}", file_path.display(), rendered)
}

fn render_diagnostics_with_spans(diagnostics: &[Diagnostic], source_text: &str) -> String {
    if diagnostics.is_empty() {
        return String::new();
    }

    diagnostics
        .iter()
        .map(|diagnostic| render_diagnostic_with_span(diagnostic, source_text))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_diagnostic_with_span(diagnostic: &Diagnostic, source_text: &str) -> String {
    let mut header = format!("{} {}", diagnostic.code, diagnostic.file_name);

    if let Some(span) = diagnostic.span {
        let (line, column) = line_col_from_offset(source_text, span.start);
        header.push_str(&format!(
            " start={} end={} line={} column={}",
            span.start, span.end, line, column
        ));
    } else {
        header.push_str(" (no span)");
    }

    format!("{header}\n{}", diagnostic.render(source_text))
}

pub(crate) fn line_col_from_offset(source_text: &str, offset: usize) -> (usize, usize) {
    let mut line = 1usize;
    let mut column = 1usize;
    let target = offset.min(source_text.len());

    for (byte_index, ch) in source_text.char_indices() {
        if byte_index >= target {
            break;
        }

        if ch == '\n' {
            line += 1;
            column = 1;
        } else {
            column += 1;
        }
    }

    (line, column)
}

fn render_diagnostic_json(
    loaded: &LoadedTsConfig,
    diagnostic: &Diagnostic,
    sources: &[(PathBuf, String, String)],
) -> Value {
    let mut item = Map::new();
    item.insert(
        "code".to_string(),
        Value::String(diagnostic.code.to_string()),
    );
    item.insert(
        "fileName".to_string(),
        Value::String(report_path_label(&loaded.root_dir, &diagnostic.file_name)),
    );
    item.insert(
        "message".to_string(),
        Value::String(diagnostic.message.clone()),
    );

    if let Some(span) = diagnostic.span {
        let mut span_json = Map::new();
        span_json.insert("start".to_string(), Value::from(span.start as u64));
        span_json.insert("end".to_string(), Value::from(span.end as u64));
        item.insert("span".to_string(), Value::Object(span_json));

        if let Some(source_text) = source_text_for_diagnostic(sources, &diagnostic.file_name) {
            let (line, column) = line_col_from_offset(source_text, span.start);
            item.insert("line".to_string(), Value::from(line as u64));
            item.insert("column".to_string(), Value::from(column as u64));
        }
    }

    Value::Object(item)
}

fn source_text_for_diagnostic<'a>(
    sources: &'a [(PathBuf, String, String)],
    file_name: &str,
) -> Option<&'a str> {
    sources
        .iter()
        .find(|(_, source_file_name, _)| source_file_name == file_name)
        .map(|(_, _, source_text)| source_text.as_str())
}

fn sort_counts(counts: HashMap<String, usize>) -> Vec<CompatReportCountEntry> {
    let mut entries = counts
        .into_iter()
        .map(|(key, count)| CompatReportCountEntry { key, count })
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.key.cmp(&right.key))
    });
    entries
}

fn sort_module_export_counts(
    counts: HashMap<(String, String), usize>,
) -> Vec<CompatReportModuleExportCountEntry> {
    let mut entries = counts
        .into_iter()
        .map(
            |((module_specifier, export_name), count)| CompatReportModuleExportCountEntry {
                module_specifier,
                export_name,
                count,
            },
        )
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.module_specifier.cmp(&right.module_specifier))
            .then_with(|| left.export_name.cmp(&right.export_name))
    });
    entries
}

fn sort_parser_errors(
    counts: HashMap<(String, String), usize>,
) -> Vec<CompatReportParserErrorEntry> {
    let mut entries = counts
        .into_iter()
        .map(
            |((file_name, message), count)| CompatReportParserErrorEntry {
                file_name,
                message,
                count,
            },
        )
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.file_name.cmp(&right.file_name))
            .then_with(|| left.message.cmp(&right.message))
    });
    entries
}

fn parser_error_total(report: &ProjectCompatibilityReport) -> usize {
    report.parser_errors.iter().map(|entry| entry.count).sum()
}

fn sort_categorized_counts(
    counts: HashMap<(String, String), usize>,
) -> Vec<CompatReportCategorizedCountEntry> {
    let mut entries = counts
        .into_iter()
        .map(
            |((key, category), count)| CompatReportCategorizedCountEntry {
                key,
                category,
                count,
            },
        )
        .collect::<Vec<_>>();
    entries.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then_with(|| left.key.cmp(&right.key))
            .then_with(|| left.category.cmp(&right.category))
    });
    entries
}

fn report_path_label(root_dir: &Path, file_name: &str) -> String {
    let path = Path::new(file_name);
    path.strip_prefix(root_dir)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| file_name.to_string())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum FileKindLabel {
    RootSource,
    RootDeclaration,
    DependencyDeclaration,
    GeneratedDeclaration,
}

impl FileKindLabel {
    fn label(self) -> &'static str {
        match self {
            FileKindLabel::RootSource => "root-source",
            FileKindLabel::RootDeclaration => "root-declaration",
            FileKindLabel::DependencyDeclaration => "dependency-declaration",
            FileKindLabel::GeneratedDeclaration => "generated-declaration",
        }
    }
}

fn classify_file_kind(file_name: &str) -> FileKindLabel {
    let lower = file_name.to_ascii_lowercase();
    let is_decl = is_declaration_file_name(file_name);

    if is_decl {
        if lower.contains("/.nuxt/")
            || lower.contains("/.generated/")
            || lower.contains("/generated/")
            || lower.contains("/dist/")
        {
            return FileKindLabel::GeneratedDeclaration;
        }

        if lower.contains("/node_modules/") || lower.contains("/node_modules/.pnpm/") {
            return FileKindLabel::DependencyDeclaration;
        }

        return FileKindLabel::RootDeclaration;
    }

    FileKindLabel::RootSource
}

fn is_declaration_file_name(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.ends_with(".d.ts") || lower.ends_with(".d.mts") || lower.ends_with(".d.cts")
}

fn is_node_modules_source_file(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.contains("/node_modules/") && !is_declaration_file_name(file_name)
}

fn node_modules_source_prefix(file_name: &str) -> Option<String> {
    let normalized = file_name.replace('\\', "/");
    let needle = "/node_modules/";
    let index = normalized.find(needle)?;
    let remainder = &normalized[index + needle.len()..];
    let mut segments = remainder.split('/');

    let first = segments.next()?;
    if first == ".pnpm" {
        let _package_version = segments.next()?;
        let _nested = segments.next()?;
        let package_name = segments.next()?;
        if package_name.is_empty() {
            return None;
        }
        if package_name.starts_with('@') {
            let package_subpath = segments.next()?;
            return Some(format!("{package_name}/{package_subpath}"));
        }
        return Some(package_name.to_string());
    }

    if first.starts_with('@') {
        let package_subpath = segments.next()?;
        return Some(format!("{first}/{package_subpath}"));
    }

    Some(first.to_string())
}

fn extract_ts2305_module_export(message: &str) -> Option<(String, String)> {
    let prefix = "Module ";
    let suffix = " has no exported member ";
    let start = message.find(prefix)? + prefix.len();
    let rest = &message[start..];
    let quote = rest.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let rest = &rest[1..];
    let module_end = rest.find(quote)?;
    let module = &rest[..module_end];
    let rest = &rest[module_end + 1..];
    let suffix_start = rest.find(suffix)? + suffix.len();
    let rest = &rest[suffix_start..];
    let member_quote = rest.chars().next()?;
    if member_quote != '\'' && member_quote != '"' {
        return None;
    }
    let rest = &rest[1..];
    let member_end = rest.find(member_quote)?;
    let member = &rest[..member_end];
    Some((module.to_string(), member.to_string()))
}

fn extract_ts2307_module_specifier(message: &str) -> Option<String> {
    let prefix = "module ";
    let start = message.find(prefix)? + prefix.len();
    let rest = &message[start..];
    let quote = rest.chars().next()?;
    if quote != '\'' && quote != '"' {
        return None;
    }
    let rest = &rest[1..];
    let end = rest.find(quote)?;
    Some(rest[..end].to_string())
}

fn extract_ts2304_identifier(message: &str) -> Option<String> {
    for prefix in ["Cannot find name ", "Cannot find namespace "] {
        if let Some(start) = message.find(prefix) {
            let rest = &message[start + prefix.len()..];
            let quote = rest.chars().next()?;
            if quote != '\'' && quote != '"' {
                continue;
            }
            let rest = &rest[1..];
            if let Some(end) = rest.find(quote) {
                return Some(rest[..end].to_string());
            }
        }
    }

    None
}

// These are triage categories for reporting, not semantic claims about the
// underlying compiler behavior.
fn classify_ts2307_module_specifier(specifier: &str) -> String {
    if is_relative_specifier(specifier) {
        if is_json_module_specifier(specifier) {
            return "json".to_string();
        }
        if is_generated_module_specifier(specifier) {
            return "generated-file".to_string();
        }
        if is_config_tooling_module_specifier(specifier) {
            return "config/tooling".to_string();
        }
        return "relative".to_string();
    }

    if is_package_subpath_specifier(specifier) {
        return "package-subpath".to_string();
    }

    "package".to_string()
}

fn classify_ts2304_identifier(identifier: &str) -> String {
    if is_jsx_like_identifier(identifier) {
        return "jsx-like".to_string();
    }
    if is_dom_like_identifier(identifier) {
        return "dom-like".to_string();
    }
    if is_node_like_identifier(identifier) {
        return "node-like".to_string();
    }
    if is_local_unresolved_identifier(identifier) {
        return "local unresolved".to_string();
    }
    if is_package_derived_identifier(identifier) {
        return "package-derived".to_string();
    }

    "unknown".to_string()
}

fn is_json_module_specifier(specifier: &str) -> bool {
    specifier.to_ascii_lowercase().ends_with(".json")
}

fn is_generated_module_specifier(specifier: &str) -> bool {
    let lower = specifier.to_ascii_lowercase();
    lower.contains(".gen") || lower.contains("/generated/")
}

fn is_config_tooling_module_specifier(specifier: &str) -> bool {
    let lower = specifier.to_ascii_lowercase();
    lower.ends_with(".config")
        || lower.ends_with(".config.ts")
        || lower.ends_with(".config.tsx")
        || lower.ends_with(".config.mts")
        || lower.ends_with(".config.cts")
        || lower.ends_with(".config.js")
        || lower.ends_with(".config.mjs")
        || lower.ends_with(".config.cjs")
        || lower.contains(".config/")
        || lower.contains("/config.")
        || lower.contains("/config/")
        || lower.contains("vitest.config")
        || lower.contains("eslint.config")
        || lower.contains("tailwind.config")
        || lower.contains("next.config")
        || lower.contains("postcss.config")
        || lower.contains("drizzle.config")
        || lower.contains("sandbox.config")
        || lower.contains("playwright.config")
        || lower.contains("turbo.json")
        || lower.ends_with("package.json")
        || lower.ends_with("tsconfig.json")
        || lower.ends_with("deno.json")
        || lower.ends_with("vercel.json")
        || lower.ends_with("package-lock.json")
}

fn is_package_subpath_specifier(specifier: &str) -> bool {
    if !specifier.contains('/') {
        return false;
    }

    if !specifier.starts_with('@') {
        return true;
    }

    specifier.split('/').count() > 2
}

fn is_jsx_like_identifier(identifier: &str) -> bool {
    matches!(
        identifier,
        "JSX" | "IntrinsicElements" | "Fragment" | "React"
    )
}

fn is_dom_like_identifier(identifier: &str) -> bool {
    matches!(
        identifier,
        "document"
            | "window"
            | "navigator"
            | "Headers"
            | "FormData"
            | "URLSearchParams"
            | "Blob"
            | "File"
            | "Response"
            | "Request"
            | "ReadableStream"
            | "WritableStream"
            | "TransformStream"
            | "Event"
            | "MessageEvent"
            | "HTMLElement"
            | "Element"
            | "Node"
            | "Text"
            | "Document"
            | "console"
    )
}

fn is_node_like_identifier(identifier: &str) -> bool {
    matches!(
        identifier,
        "process" | "Buffer" | "require" | "module" | "exports" | "__dirname" | "__filename"
    )
}

fn is_local_unresolved_identifier(identifier: &str) -> bool {
    identifier
        .chars()
        .next()
        .map(|first| first.is_ascii_lowercase() || first == '_')
        .unwrap_or(false)
}

fn is_package_derived_identifier(identifier: &str) -> bool {
    identifier.len() > 1
        && identifier
            .chars()
            .next()
            .map(|first| first.is_ascii_uppercase())
            .unwrap_or(false)
}
