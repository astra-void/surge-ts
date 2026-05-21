mod import_graph;
mod package_declarations;
mod path_mapping;
mod report;

use std::time::Instant;
use std::{fs, path::PathBuf, process::ExitCode};

use clap::{Error, Parser, error::ErrorKind};
use report::{
    ReportFormat, build_project_compatibility_report, render_project_compatibility_report_json,
    render_project_compatibility_report_text, render_project_diagnostics_json,
    render_project_diagnostics_preview,
};
use serde_json::{Map, Value};
use typescript_rust_checker::{
    CheckerOptions, SourceFileInput, check_program_with_stats_and_jobs, check_source_with_options,
};
use typescript_rust_config::{TsConfigLoadOptions, canonicalize_if_exists_string, load_tsconfig};
use typescript_rust_diagnostics::{Diagnostic, DiagnosticCode, render_diagnostics};

#[derive(Debug, Clone, clap::ValueEnum)]
enum CliDiagnosticProfile {
    Tsc,
    Native,
}

impl Into<typescript_rust_checker::DiagnosticProfile> for CliDiagnosticProfile {
    fn into(self) -> typescript_rust_checker::DiagnosticProfile {
        match self {
            CliDiagnosticProfile::Tsc => typescript_rust_checker::DiagnosticProfile::Tsc,
            CliDiagnosticProfile::Native => typescript_rust_checker::DiagnosticProfile::Native,
        }
    }
}

#[derive(Debug, Default)]
struct CliTimings {
    config_project_loading: std::time::Duration,
    file_discovery: std::time::Duration,
    package_declaration_discovery: std::time::Duration,
    import_graph_expansion: std::time::Duration,
    diagnostic_rendering: std::time::Duration,
}

#[derive(Debug, Parser)]
#[command(author, version, about, disable_help_subcommand = true)]
struct Cli {
    #[arg(value_name = "FILE")]
    file_path: Option<PathBuf>,

    #[arg(short, long, value_name = "TSCONFIG")]
    project: Option<PathBuf>,

    #[arg(long = "showConfig")]
    show_config: bool,

    #[arg(long = "showSpans")]
    show_spans: bool,

    #[arg(long = "ignoreConfig")]
    ignore_config: bool,

    #[arg(long = "compatReport")]
    compat_report: bool,

    #[arg(long, value_enum)]
    format: Option<ReportFormat>,

    #[arg(long = "diagnosticProfile", value_enum)]
    diagnostic_profile: Option<CliDiagnosticProfile>,

    #[arg(long = "maxDiagnostics")]
    max_diagnostics: Option<usize>,

    #[arg(long, value_parser = parse_jobs)]
    jobs: Option<usize>,

    #[arg(long = "stubExternalModules")]
    stub_external_modules: bool,

    #[arg(long)]
    no_implicit_any: bool,

    #[arg(long = "noLib")]
    no_lib: bool,

    #[arg(long, hide = true)]
    timings: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if cli.timings {
        unsafe {
            std::env::set_var("TYPESCRIPT_RUST_TIMINGS", "1");
        }
    }

    if cli.max_diagnostics == Some(0) {
        Error::raw(
            ErrorKind::InvalidValue,
            "--maxDiagnostics must be greater than 0",
        )
        .exit();
    }

    if cli.project.is_some() && cli.file_path.is_some() {
        Error::raw(
            ErrorKind::ArgumentConflict,
            "cannot use a positional file path together with --project",
        )
        .exit();
    }

    if cli.project.is_some() {
        if cli.ignore_config {
            Error::raw(
                ErrorKind::ArgumentConflict,
                "--ignoreConfig cannot be used with --project",
            )
            .exit();
        }
        return run_project_mode(
            cli.project.unwrap(),
            cli.show_config,
            cli.show_spans,
            cli.compat_report,
            cli.format.unwrap_or(ReportFormat::Text),
            cli.max_diagnostics,
            cli.jobs.unwrap_or(1),
            cli.stub_external_modules,
            cli.diagnostic_profile
                .unwrap_or(CliDiagnosticProfile::Tsc)
                .into(),
            cli.timings,
        );
    }

    if cli.show_config {
        Error::raw(
            ErrorKind::MissingRequiredArgument,
            "--showConfig requires --project",
        )
        .exit();
    }

    if cli.jobs.is_some() {
        Error::raw(
            ErrorKind::ArgumentConflict,
            "--jobs is only supported with --project",
        )
        .exit();
    }

    if cli.compat_report {
        Error::raw(
            ErrorKind::MissingRequiredArgument,
            "--compatReport requires --project",
        )
        .exit();
    }

    let Some(file_path) = cli.file_path else {
        Error::raw(
            ErrorKind::MissingRequiredArgument,
            "expected a file path or --project",
        )
        .exit();
    };

    run_single_file_mode(
        file_path,
        cli.no_implicit_any,
        cli.no_lib,
        cli.stub_external_modules,
        cli.show_spans,
        cli.format.unwrap_or(ReportFormat::Text),
        cli.max_diagnostics,
        cli.ignore_config,
        cli.diagnostic_profile
            .unwrap_or(CliDiagnosticProfile::Tsc)
            .into(),
    )
}

fn run_single_file_mode(
    file_path: PathBuf,
    no_implicit_any: bool,
    no_lib: bool,
    stub_external_modules: bool,
    show_spans: bool,
    format: ReportFormat,
    max_diagnostics: Option<usize>,
    ignore_config: bool,
    diagnostic_profile: typescript_rust_checker::DiagnosticProfile,
) -> ExitCode {
    if !ignore_config
        && std::env::current_dir()
            .map(|dir| dir.join("tsconfig.json").exists())
            .unwrap_or(false)
    {
        let diagnostic = typescript_rust_diagnostics::Diagnostic::ts5112("<command line>");
        match format {
            ReportFormat::Text => println!("{}", diagnostic.render("")),
            ReportFormat::Json => {
                let json = serde_json::to_string_pretty(&render_single_file_diagnostics_json(
                    &file_path,
                    &[diagnostic],
                    "",
                    max_diagnostics,
                ))
                .unwrap();
                println!("{}", json);
            }
        }
        return ExitCode::from(1);
    }
    let source_text = match fs::read_to_string(&file_path) {
        Ok(source_text) => source_text,
        Err(error) => {
            eprintln!("failed to read {}: {error}", file_path.display());
            return ExitCode::from(1);
        }
    };

    let file_name = canonicalize_if_exists_string(&file_path);

    let diagnostics = check_source_with_options(
        &source_text,
        &file_name,
        CheckerOptions {
            no_implicit_any,
            no_lib,
            skip_lib_check: false,
            stub_external_modules,
            resolved_modules: std::collections::HashMap::new(),
            types: Vec::new(),
            diagnostic_profile,
        },
    );
    match format {
        ReportFormat::Text => println!(
            "{}",
            render_single_file_diagnostics(
                &file_path,
                &diagnostics,
                &source_text,
                show_spans,
                max_diagnostics
            )
        ),
        ReportFormat::Json => println!(
            "{}",
            serde_json::to_string_pretty(&render_single_file_diagnostics_json(
                &file_path,
                &diagnostics,
                &source_text,
                max_diagnostics
            ))
            .unwrap()
        ),
    }

    ExitCode::SUCCESS
}

fn run_project_mode(
    project: PathBuf,
    show_config: bool,
    show_spans: bool,
    compat_report: bool,
    format: ReportFormat,
    max_diagnostics: Option<usize>,
    jobs: usize,
    stub_external_modules: bool,
    diagnostic_profile: typescript_rust_checker::DiagnosticProfile,
    timings_enabled: bool,
) -> ExitCode {
    let mut timings = CliTimings::default();

    let config_start = Instant::now();
    let loaded = load_tsconfig(TsConfigLoadOptions { project });
    if timings_enabled {
        timings.config_project_loading += config_start.elapsed();
    }

    for diagnostic in &loaded.diagnostics {
        eprintln!("{diagnostic}");
    }

    if show_config {
        let config = build_show_config_json(&loaded);
        println!("{}", serde_json::to_string_pretty(&config).unwrap());
        if timings_enabled {
            render_cli_timings(&timings);
        }
        return ExitCode::SUCCESS;
    }

    if loaded.files.is_empty() {
        let diagnostics = vec![project_has_no_source_files_diagnostic(&loaded)];
        let stats = typescript_rust_checker::CompatibilityStats::default();
        let exit_code = render_project_mode_output(
            &loaded,
            &diagnostics,
            &[],
            &stats,
            show_spans,
            compat_report,
            format,
            max_diagnostics,
            timings_enabled,
            &mut timings,
        );
        if timings_enabled {
            render_cli_timings(&timings);
        }
        return exit_code;
    }

    let mut inputs = Vec::with_capacity(loaded.files.len());
    let mut sources = Vec::with_capacity(loaded.files.len());

    let file_discovery_start = Instant::now();
    for file_path in &loaded.files {
        let source_text = match fs::read_to_string(&file_path) {
            Ok(source_text) => source_text,
            Err(error) => {
                eprintln!("failed to read {}: {error}", file_path.display());
                return ExitCode::from(1);
            }
        };

        let file_name = canonicalize_if_exists_string(file_path);
        inputs.push(SourceFileInput {
            file_name: file_name.clone(),
            source_text: source_text.clone(),
        });
        sources.push((file_path.clone(), file_name, source_text));
    }
    if timings_enabled {
        timings.file_discovery += file_discovery_start.elapsed();
    }

    let mut resolved_modules = std::collections::HashMap::new();
    let mut package_resolution_cache =
        package_declarations::PackageDeclarationResolverCache::default();
    loop {
        let files_before = inputs.len();

        let package_start = Instant::now();
        let package_modules =
            package_declarations::resolve_package_declaration_entrypoints_with_cache(
                &mut inputs,
                &mut sources,
                &loaded.root_dir,
                &mut package_resolution_cache,
            );
        if timings_enabled {
            timings.package_declaration_discovery += package_start.elapsed();
        }
        for (specifier, resolved_file) in package_modules {
            resolved_modules.insert(specifier, resolved_file);
        }

        let import_graph_start = Instant::now();
        let graph_loaded = import_graph::expand_project_inputs(
            &mut inputs,
            &mut sources,
            &loaded.root_dir,
            &loaded.compiler_options.paths,
        );
        if timings_enabled {
            timings.import_graph_expansion += import_graph_start.elapsed();
        }

        if graph_loaded == 0 && inputs.len() == files_before {
            break;
        }
    }

    let path_modules = path_mapping::resolve_path_mappings(
        &inputs,
        &loaded.compiler_options.paths,
        &loaded.root_dir,
    );

    for (k, v) in path_modules {
        resolved_modules.insert(k, v);
    }

    let checker_options = CheckerOptions {
        no_implicit_any: loaded.compiler_options.no_implicit_any,
        no_lib: loaded.compiler_options.no_lib,
        skip_lib_check: loaded.compiler_options.skip_lib_check,
        stub_external_modules,
        resolved_modules,
        types: loaded.compiler_options.types.clone(),
        diagnostic_profile,
    };

    let result = check_program_with_stats_and_jobs(inputs, checker_options, jobs);
    let diagnostics = apply_project_no_lib_compatibility_diagnostics(
        result.diagnostics,
        loaded.compiler_options.no_lib,
        diagnostic_profile,
    );
    let exit_code = render_project_mode_output(
        &loaded,
        &diagnostics,
        &sources,
        &result.stats,
        show_spans,
        compat_report,
        format,
        max_diagnostics,
        timings_enabled,
        &mut timings,
    );
    if timings_enabled {
        render_cli_timings(&timings);
    }
    exit_code
}

fn parse_jobs(value: &str) -> Result<usize, String> {
    let jobs = value
        .parse::<usize>()
        .map_err(|_| format!("invalid value for --jobs: {value}"))?;
    if jobs == 0 {
        return Err("--jobs must be greater than 0".to_string());
    }

    Ok(jobs)
}

fn render_project_mode_output(
    loaded: &typescript_rust_config::LoadedTsConfig,
    diagnostics: &[Diagnostic],
    sources: &[(PathBuf, String, String)],
    stats: &typescript_rust_checker::CompatibilityStats,
    show_spans: bool,
    compat_report: bool,
    format: ReportFormat,
    max_diagnostics: Option<usize>,
    timings_enabled: bool,
    timings: &mut CliTimings,
) -> ExitCode {
    let render_start = Instant::now();
    if compat_report {
        let report = build_project_compatibility_report(loaded, diagnostics, sources, stats);
        match format {
            ReportFormat::Text => {
                println!("{}", render_project_compatibility_report_text(&report));
                let preview = render_project_diagnostics_preview(
                    diagnostics,
                    sources,
                    show_spans,
                    max_diagnostics,
                );
                if !preview.is_empty() {
                    println!();
                    println!("{}", preview);
                }
            }
            ReportFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&render_project_compatibility_report_json(
                        &report
                    ))
                    .unwrap()
                );
            }
        }
        if timings_enabled {
            timings.diagnostic_rendering += render_start.elapsed();
        }
        return ExitCode::SUCCESS;
    }

    match format {
        ReportFormat::Text => {
            let preview = render_project_diagnostics_preview(
                diagnostics,
                sources,
                show_spans,
                max_diagnostics,
            );
            if !preview.is_empty() {
                println!("{}", preview);
            }
        }
        ReportFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(&render_project_diagnostics_json(
                    loaded,
                    diagnostics,
                    sources,
                    max_diagnostics
                ))
                .unwrap()
            );
        }
    }

    if timings_enabled {
        timings.diagnostic_rendering += render_start.elapsed();
    }

    ExitCode::SUCCESS
}

fn apply_project_no_lib_compatibility_diagnostics(
    diagnostics: Vec<Diagnostic>,
    no_lib: bool,
    diagnostic_profile: typescript_rust_checker::DiagnosticProfile,
) -> Vec<Diagnostic> {
    if !no_lib || diagnostic_profile != typescript_rust_checker::DiagnosticProfile::Tsc {
        return diagnostics;
    }

    let mut filtered = diagnostics
        .into_iter()
        .filter(|diagnostic| !matches!(diagnostic.code, DiagnosticCode::TypeScript(2304)))
        .collect::<Vec<_>>();

    filtered.extend(project_no_lib_missing_global_type_diagnostics());
    filtered
}

fn project_no_lib_missing_global_type_diagnostics() -> Vec<Diagnostic> {
    let file_name = String::new();

    [
        "Array",
        "Boolean",
        "CallableFunction",
        "Function",
        "IArguments",
        "NewableFunction",
        "Number",
        "Object",
        "RegExp",
        "String",
    ]
    .into_iter()
    .map(|global_type| {
        Diagnostic::new(
            DiagnosticCode::TypeScript(2318),
            format!("Cannot find global type '{global_type}'."),
            file_name.clone(),
        )
    })
    .collect()
}

fn render_cli_timings(timings: &CliTimings) {
    eprintln!("CLI timings:");
    eprintln!(
        "  config_project_loading: {}",
        format_duration(timings.config_project_loading)
    );
    eprintln!(
        "  file_discovery: {}",
        format_duration(timings.file_discovery)
    );
    eprintln!(
        "  package_declaration_discovery: {}",
        format_duration(timings.package_declaration_discovery)
    );
    eprintln!(
        "  import_graph_expansion: {}",
        format_duration(timings.import_graph_expansion)
    );
    eprintln!(
        "  diagnostic_rendering: {}",
        format_duration(timings.diagnostic_rendering)
    );
}

fn format_duration(duration: std::time::Duration) -> String {
    format!("{:.3}ms", duration.as_secs_f64() * 1000.0)
}

fn render_single_file_diagnostics(
    file_path: &std::path::Path,
    diagnostics: &[typescript_rust_diagnostics::Diagnostic],
    source_text: &str,
    show_spans: bool,
    max_diagnostics: Option<usize>,
) -> String {
    if diagnostics.is_empty() {
        return render_diagnostics(&[], source_text);
    }

    let total = diagnostics.len();
    let limit = max_diagnostics.unwrap_or(total).min(total);
    let truncated = limit < total;
    let diagnostics = &diagnostics[..limit];

    let rendered = if show_spans {
        render_diagnostics_with_spans(file_path, diagnostics, source_text)
    } else {
        render_diagnostics(diagnostics, source_text)
    };

    if truncated {
        format!(
            "{rendered}\n\nShowing first {} of {} diagnostics.",
            diagnostics.len(),
            total
        )
    } else {
        rendered
    }
}

fn render_single_file_diagnostics_json(
    file_path: &std::path::Path,
    diagnostics: &[typescript_rust_diagnostics::Diagnostic],
    source_text: &str,
    max_diagnostics: Option<usize>,
) -> Value {
    let limit = max_diagnostics.unwrap_or(usize::MAX);
    let diagnostics = diagnostics
        .iter()
        .take(limit)
        .map(|diagnostic| build_single_file_diagnostic_json(file_path, diagnostic, source_text))
        .collect::<Vec<_>>();

    let mut root = Map::new();
    root.insert("diagnostics".to_string(), Value::Array(diagnostics));

    Value::Object(root)
}

fn build_single_file_diagnostic_json(
    file_path: &std::path::Path,
    diagnostic: &typescript_rust_diagnostics::Diagnostic,
    source_text: &str,
) -> Value {
    let mut item = Map::new();
    item.insert(
        "code".to_string(),
        Value::String(diagnostic.code.to_string()),
    );
    item.insert(
        "fileName".to_string(),
        if diagnostic.file_name == "<command line>" {
            Value::String(diagnostic.file_name.clone())
        } else {
            Value::String(file_path.display().to_string())
        },
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

        let (line, column) = line_col_from_offset(source_text, span.start);
        item.insert("line".to_string(), Value::from(line as u64));
        item.insert("column".to_string(), Value::from(column as u64));
    }

    Value::Object(item)
}

fn build_show_config_json(loaded: &typescript_rust_config::LoadedTsConfig) -> Value {
    let mut root = Map::new();
    root.insert(
        "configPath".to_string(),
        Value::String(loaded.config_path.display().to_string()),
    );
    root.insert(
        "rootDir".to_string(),
        Value::String(loaded.root_dir.display().to_string()),
    );
    root.insert(
        "files".to_string(),
        Value::Array(
            loaded
                .files
                .iter()
                .map(|path| Value::String(path.display().to_string()))
                .collect(),
        ),
    );
    root.insert(
        "compilerOptions".to_string(),
        build_compiler_options_json(&loaded.compiler_options),
    );

    Value::Object(root)
}

fn project_has_no_source_files_diagnostic(
    loaded: &typescript_rust_config::LoadedTsConfig,
) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::Custom("typescript-rust::project-has-no-source-files"),
        format!(
            "no source files were discovered for {}",
            loaded.config_path.display()
        ),
        loaded.config_path.display().to_string(),
    )
}

fn render_diagnostics_with_spans(
    file_path: &std::path::Path,
    diagnostics: &[typescript_rust_diagnostics::Diagnostic],
    source_text: &str,
) -> String {
    if diagnostics.is_empty() {
        return String::new();
    }

    diagnostics
        .iter()
        .map(|diagnostic| render_diagnostic_with_spans(file_path, diagnostic, source_text))
        .collect::<Vec<_>>()
        .join("\n\n")
}

fn render_diagnostic_with_spans(
    file_path: &std::path::Path,
    diagnostic: &typescript_rust_diagnostics::Diagnostic,
    source_text: &str,
) -> String {
    let mut header = format!("{} {}", diagnostic.code, file_path.display());

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

fn line_col_from_offset(source_text: &str, offset: usize) -> (usize, usize) {
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

fn build_compiler_options_json(
    compiler_options: &typescript_rust_config::NormalizedCompilerOptions,
) -> Value {
    let mut options = Map::new();
    options.insert("strict".to_string(), Value::Bool(compiler_options.strict));
    options.insert(
        "noImplicitAny".to_string(),
        Value::Bool(compiler_options.no_implicit_any),
    );
    options.insert(
        "target".to_string(),
        Value::String(script_target_to_string(compiler_options.target).to_string()),
    );
    options.insert(
        "module".to_string(),
        Value::String(module_kind_to_string(compiler_options.module).to_string()),
    );
    options.insert(
        "moduleResolution".to_string(),
        Value::String(
            module_resolution_kind_to_string(compiler_options.module_resolution).to_string(),
        ),
    );
    options.insert(
        "allowJs".to_string(),
        Value::Bool(compiler_options.allow_js),
    );
    options.insert(
        "checkJs".to_string(),
        Value::Bool(compiler_options.check_js),
    );
    options.insert("noEmit".to_string(), Value::Bool(compiler_options.no_emit));
    options.insert(
        "skipLibCheck".to_string(),
        Value::Bool(compiler_options.skip_lib_check),
    );
    options.insert("noLib".to_string(), Value::Bool(compiler_options.no_lib));

    if let Some(jsx) = compiler_options.jsx.as_ref() {
        options.insert(
            "jsx".to_string(),
            Value::String(jsx_mode_to_string(jsx).to_string()),
        );
    }
    if !compiler_options.paths.is_empty() {
        options.insert(
            "paths".to_string(),
            Value::Object(
                compiler_options
                    .paths
                    .iter()
                    .map(|mapping| {
                        (
                            mapping.pattern.clone(),
                            Value::Array(
                                mapping
                                    .substitutions
                                    .iter()
                                    .map(|substitution| Value::String(substitution.clone()))
                                    .collect(),
                            ),
                        )
                    })
                    .collect(),
            ),
        );
    }
    if !compiler_options.type_roots.is_empty() {
        options.insert(
            "typeRoots".to_string(),
            Value::Array(
                compiler_options
                    .type_roots
                    .iter()
                    .map(|path| Value::String(path.display().to_string()))
                    .collect(),
            ),
        );
    }
    if !compiler_options.types.is_empty() {
        options.insert(
            "types".to_string(),
            Value::Array(
                compiler_options
                    .types
                    .iter()
                    .map(|ty| Value::String(ty.clone()))
                    .collect(),
            ),
        );
    }

    Value::Object(options)
}

fn script_target_to_string(target: typescript_rust_config::ScriptTarget) -> &'static str {
    match target {
        typescript_rust_config::ScriptTarget::ES2015 => "es2015",
        typescript_rust_config::ScriptTarget::ES2016 => "es2016",
        typescript_rust_config::ScriptTarget::ES2017 => "es2017",
        typescript_rust_config::ScriptTarget::ES2018 => "es2018",
        typescript_rust_config::ScriptTarget::ES2019 => "es2019",
        typescript_rust_config::ScriptTarget::ES2020 => "es2020",
        typescript_rust_config::ScriptTarget::ES2021 => "es2021",
        typescript_rust_config::ScriptTarget::ES2022 => "es2022",
        typescript_rust_config::ScriptTarget::ES2023 => "es2023",
        typescript_rust_config::ScriptTarget::ES2024 => "es2024",
        typescript_rust_config::ScriptTarget::ESNext => "esnext",
    }
}

fn module_kind_to_string(module: typescript_rust_config::ModuleKind) -> &'static str {
    match module {
        typescript_rust_config::ModuleKind::CommonJS => "commonjs",
        typescript_rust_config::ModuleKind::ES2015 => "es2015",
        typescript_rust_config::ModuleKind::ES2020 => "es2020",
        typescript_rust_config::ModuleKind::ES2022 => "es2022",
        typescript_rust_config::ModuleKind::ESNext => "esnext",
        typescript_rust_config::ModuleKind::Node16 => "node16",
        typescript_rust_config::ModuleKind::Node18 => "node18",
        typescript_rust_config::ModuleKind::NodeNext => "nodenext",
        typescript_rust_config::ModuleKind::Preserve => "preserve",
    }
}

fn module_resolution_kind_to_string(
    module_resolution: typescript_rust_config::ModuleResolutionKind,
) -> &'static str {
    match module_resolution {
        typescript_rust_config::ModuleResolutionKind::Node16 => "node16",
        typescript_rust_config::ModuleResolutionKind::NodeNext => "nodenext",
        typescript_rust_config::ModuleResolutionKind::Bundler => "bundler",
    }
}

fn jsx_mode_to_string(jsx: &typescript_rust_config::JsxMode) -> &'static str {
    match jsx {
        typescript_rust_config::JsxMode::Preserve => "preserve",
        typescript_rust_config::JsxMode::React => "react",
        typescript_rust_config::JsxMode::ReactJsx => "react-jsx",
        typescript_rust_config::JsxMode::ReactJsxDev => "react-jsxdev",
    }
}
