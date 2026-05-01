use std::{fs, path::PathBuf, process::ExitCode};

use clap::{Error, Parser, error::ErrorKind};
use serde_json::{Map, Value};
use typescript_rust_checker::{CheckerOptions, check_source_with_options};
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

    #[arg(long)]
    no_implicit_any: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if cli.project.is_some() && cli.file_path.is_some() {
        Error::raw(
            ErrorKind::ArgumentConflict,
            "cannot use a positional file path together with --project",
        )
        .exit();
    }

    if let Some(project) = cli.project {
        return run_project_mode(project, cli.show_config);
    }

    if cli.show_config {
        Error::raw(
            ErrorKind::MissingRequiredArgument,
            "--showConfig requires --project",
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

    run_single_file_mode(file_path, cli.no_implicit_any)
}

fn run_single_file_mode(file_path: PathBuf, no_implicit_any: bool) -> ExitCode {
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
        CheckerOptions { no_implicit_any },
    );
    println!("{}", render_diagnostics(&diagnostics, &source_text));

    ExitCode::SUCCESS
}

fn run_project_mode(project: PathBuf, show_config: bool) -> ExitCode {
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
    };

    for file_path in loaded.files {
        let source_text = match fs::read_to_string(&file_path) {
            Ok(source_text) => source_text,
            Err(error) => {
                eprintln!("failed to read {}: {error}", file_path.display());
                return ExitCode::from(1);
            }
        };

        let diagnostics =
            check_source_with_options(&source_text, &file_path.to_string_lossy(), checker_options);
        if diagnostics.is_empty() {
            continue;
        }

        println!(
            "{}\n{}",
            file_path.display(),
            render_diagnostics(&diagnostics, &source_text)
        );
    }

    ExitCode::SUCCESS
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
