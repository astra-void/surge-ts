mod report;

use std::{fs, path::PathBuf, process::ExitCode};

use clap::{Error, Parser, error::ErrorKind};
use report::{
    ReportFormat, build_project_compatibility_report, render_project_compatibility_report_json,
    render_project_compatibility_report_text, render_project_diagnostics_json,
    render_project_diagnostics_preview,
};
use serde_json::{Map, Value};
use typescript_rust_checker::{
    CheckerOptions, SourceFileInput, check_program_with_options, check_source_with_options,
};
use typescript_rust_config::{TsConfigLoadOptions, load_tsconfig};
use typescript_rust_diagnostics::render_diagnostics;

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

    #[arg(long = "maxDiagnostics")]
    max_diagnostics: Option<usize>,

    #[arg(long = "stubExternalModules")]
    stub_external_modules: bool,

    #[arg(long)]
    no_implicit_any: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

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
            cli.stub_external_modules,
        );
    }

    if cli.show_config {
        Error::raw(
            ErrorKind::MissingRequiredArgument,
            "--showConfig requires --project",
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
        cli.stub_external_modules,
        cli.show_spans,
        cli.format.unwrap_or(ReportFormat::Text),
        cli.max_diagnostics,
        cli.ignore_config,
    )
}

fn run_single_file_mode(
    file_path: PathBuf,
    no_implicit_any: bool,
    stub_external_modules: bool,
    show_spans: bool,
    format: ReportFormat,
    max_diagnostics: Option<usize>,
    ignore_config: bool,
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

    let diagnostics = check_source_with_options(
        &source_text,
        &file_path.to_string_lossy(),
        CheckerOptions {
            no_implicit_any,
            stub_external_modules,
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
    stub_external_modules: bool,
) -> ExitCode {
    let loaded = load_tsconfig(TsConfigLoadOptions { project });

    for diagnostic in &loaded.diagnostics {
        eprintln!("{diagnostic}");
    }

    if show_config {
        let config = build_show_config_json(&loaded);
        println!("{}", serde_json::to_string_pretty(&config).unwrap());
        return ExitCode::SUCCESS;
    }

    let checker_options = CheckerOptions {
        no_implicit_any: loaded.compiler_options.no_implicit_any,
        stub_external_modules,
    };

    let mut inputs = Vec::with_capacity(loaded.files.len());
    let mut sources = Vec::with_capacity(loaded.files.len());

    for file_path in &loaded.files {
        let source_text = match fs::read_to_string(&file_path) {
            Ok(source_text) => source_text,
            Err(error) => {
                eprintln!("failed to read {}: {error}", file_path.display());
                return ExitCode::from(1);
            }
        };

        let file_name = file_path.to_string_lossy().into_owned();
        inputs.push(SourceFileInput {
            file_name: file_name.clone(),
            source_text: source_text.clone(),
        });
        sources.push((file_path.clone(), file_name, source_text));
    }

    let diagnostics = check_program_with_options(inputs, checker_options);
    if compat_report {
        let report = build_project_compatibility_report(&loaded, &diagnostics, &sources);
        match format {
            ReportFormat::Text => {
                println!("{}", render_project_compatibility_report_text(&report));
                let preview = render_project_diagnostics_preview(
                    &diagnostics,
                    &sources,
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
        return ExitCode::SUCCESS;
    }

    match format {
        ReportFormat::Text => {
            let preview = render_project_diagnostics_preview(
                &diagnostics,
                &sources,
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
                    &loaded,
                    &diagnostics,
                    &sources,
                    max_diagnostics
                ))
                .unwrap()
            );
        }
    }

    ExitCode::SUCCESS
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
