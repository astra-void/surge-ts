mod import_graph;
mod io_stats;
mod package_declarations;
mod package_resolution;
mod path_mapping;
mod report;

use std::io::IsTerminal;
use std::time::Instant;
use std::{fs, path::PathBuf, process::ExitCode};

use clap::{Error, Parser, error::ErrorKind};
use report::{
    ReportFormat, build_project_compatibility_report, render_project_compatibility_report_json,
    render_project_compatibility_report_text, render_project_diagnostics_json,
    render_project_diagnostics_preview,
};
use serde_json::{Map, Value};
use surge_ts_checker::{
    CheckerOptions, DefaultLibRequest, SourceFileInput, check_program_with_stats_and_jobs,
    check_source_with_options, load_default_lib_inputs,
};
use surge_ts_config::{
    ScriptTarget, TsConfigLoadOptions, canonicalize_if_exists_string, load_tsconfig,
};
use surge_ts_diagnostics::{
    Diagnostic, DiagnosticCode, TscRenderItem, TscRenderOptions, render_diagnostics,
    render_diagnostics_tsc,
};

/// Selects how diagnostics are rendered for human-readable (non-`--format json`)
/// output. `tsc` is the default and mirrors the TypeScript compiler's text
/// output; `custom` preserves the project's original Rust-style report; `json`
/// is the machine-readable form (equivalent to `--format json`).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum CliDiagnosticStyle {
    Tsc,
    Custom,
    Json,
}

#[derive(Debug, Clone, Copy)]
enum DiagnosticStyle {
    Tsc,
    Custom,
    Json,
}

/// Controls the multi-line `tsc`-style code-frame output. `auto` follows the
/// terminal (pretty when stdout is a TTY, like `tsc`).
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
enum PrettyMode {
    #[value(name = "true")]
    Always,
    #[value(name = "false")]
    Never,
    #[value(name = "auto")]
    Auto,
}

fn resolve_diagnostic_style(
    style: Option<CliDiagnosticStyle>,
    format: Option<ReportFormat>,
) -> DiagnosticStyle {
    match style {
        Some(CliDiagnosticStyle::Tsc) => DiagnosticStyle::Tsc,
        Some(CliDiagnosticStyle::Custom) => DiagnosticStyle::Custom,
        Some(CliDiagnosticStyle::Json) => DiagnosticStyle::Json,
        // Back-compat: `--format json` keeps emitting JSON (the oracle harness
        // relies on it). Plain `--format text` and the unset default both map to
        // the new tsc-compatible renderer.
        None => match format {
            Some(ReportFormat::Json) => DiagnosticStyle::Json,
            _ => DiagnosticStyle::Tsc,
        },
    }
}

fn resolve_pretty(mode: PrettyMode) -> bool {
    match mode {
        PrettyMode::Always => true,
        PrettyMode::Never => false,
        PrettyMode::Auto => std::io::stdout().is_terminal(),
    }
}

fn resolve_color(pretty: bool) -> bool {
    if !pretty {
        return false;
    }
    if std::env::var_os("NO_COLOR").is_some() {
        return false;
    }
    if std::env::var_os("FORCE_COLOR").is_some() {
        return true;
    }
    std::io::stdout().is_terminal()
}

/// The display label `tsc` would use for a file: relative to the current working
/// directory when possible (with forward slashes), otherwise the path as-is.
/// Empty for diagnostics with no real file (globals, command-line diagnostics).
fn tsc_path_label(file_name: &str) -> String {
    if file_name.is_empty() || file_name == "<command line>" {
        return String::new();
    }
    let path = std::path::Path::new(file_name);
    if let Ok(cwd) = std::env::current_dir() {
        if let Ok(relative) = path.strip_prefix(&cwd) {
            return relative.to_string_lossy().replace('\\', "/");
        }
    }
    file_name.replace('\\', "/")
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum CliDiagnosticProfile {
    Tsc,
    Native,
}

impl Into<surge_ts_checker::DiagnosticProfile> for CliDiagnosticProfile {
    fn into(self) -> surge_ts_checker::DiagnosticProfile {
        match self {
            CliDiagnosticProfile::Tsc => surge_ts_checker::DiagnosticProfile::Tsc,
            CliDiagnosticProfile::Native => surge_ts_checker::DiagnosticProfile::Native,
        }
    }
}

#[derive(Debug, Default)]
struct CliTimings {
    config_project_loading: std::time::Duration,
    file_discovery: std::time::Duration,
    default_lib_loading: std::time::Duration,
    package_declaration_discovery: std::time::Duration,
    import_graph_expansion: std::time::Duration,
    path_mapping_resolution: std::time::Duration,
    checking: std::time::Duration,
    diagnostic_rendering: std::time::Duration,
    total: std::time::Duration,
    source_read_io: std::time::Duration,
    source_files_read: u64,
    source_bytes_read: u64,
    default_lib_files_read: u64,
    default_lib_bytes_read: u64,
    default_lib_read_io: std::time::Duration,
    default_lib_existence_probes: u64,
    default_lib_canonicalize_syscalls: u64,
    expansion_read_io: std::time::Duration,
    expansion_files_read: u64,
    expansion_bytes_read: u64,
    package_json_reads: u64,
    fs_existence_probes: u64,
    fs_read_dir_count: u64,
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

    #[arg(
        long = "diagnosticStyle",
        visible_alias = "diagnostic-style",
        value_enum
    )]
    diagnostic_style: Option<CliDiagnosticStyle>,

    #[arg(long, value_enum)]
    pretty: Option<PrettyMode>,

    #[arg(long = "diagnosticProfile", value_enum)]
    diagnostic_profile: Option<CliDiagnosticProfile>,

    #[arg(long = "maxDiagnostics")]
    max_diagnostics: Option<usize>,

    /// Worker threads for project checking: `auto` (default) sizes by available
    /// cores and workload, `1` forces serial, or pass an explicit count.
    #[arg(long, value_parser = parse_jobs, value_name = "auto|N")]
    jobs: Option<usize>,

    #[arg(long = "stubExternalModules")]
    stub_external_modules: bool,

    #[arg(long)]
    no_implicit_any: bool,

    #[arg(long = "noLib")]
    no_lib: bool,

    /// Debug aid: physical TypeScript `lib*.d.ts` loading is the default, so this
    /// flag is no longer required. When set (or via a `.physicalLibs` marker or
    /// the `SURGE_PHYSICAL_LIBS` env var), a warning is emitted if the
    /// TypeScript package cannot be found and the generated subset is used.
    #[arg(long = "physicalLibs")]
    physical_libs: bool,

    #[arg(long, hide = true)]
    timings: bool,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    if cli.timings {
        unsafe {
            std::env::set_var("SURGE_TIMINGS", "1");
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
        let style = resolve_diagnostic_style(cli.diagnostic_style, cli.format);
        let pretty = resolve_pretty(cli.pretty.unwrap_or(PrettyMode::Auto));
        let color = resolve_color(pretty);
        return run_project_mode(
            cli.project.unwrap(),
            cli.show_config,
            cli.show_spans,
            cli.compat_report,
            style,
            pretty,
            color,
            cli.max_diagnostics,
            cli.jobs.unwrap_or(0),
            cli.stub_external_modules,
            cli.diagnostic_profile
                .unwrap_or(CliDiagnosticProfile::Tsc)
                .into(),
            cli.physical_libs,
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

    let style = resolve_diagnostic_style(cli.diagnostic_style, cli.format);
    let pretty = resolve_pretty(cli.pretty.unwrap_or(PrettyMode::Auto));
    let color = resolve_color(pretty);
    run_single_file_mode(
        file_path,
        cli.no_implicit_any,
        cli.no_lib,
        cli.stub_external_modules,
        cli.show_spans,
        style,
        pretty,
        color,
        cli.max_diagnostics,
        cli.ignore_config,
        cli.diagnostic_profile
            .unwrap_or(CliDiagnosticProfile::Tsc)
            .into(),
    )
}

#[allow(clippy::too_many_arguments)]
fn run_single_file_mode(
    file_path: PathBuf,
    no_implicit_any: bool,
    no_lib: bool,
    stub_external_modules: bool,
    show_spans: bool,
    style: DiagnosticStyle,
    pretty: bool,
    color: bool,
    max_diagnostics: Option<usize>,
    ignore_config: bool,
    diagnostic_profile: surge_ts_checker::DiagnosticProfile,
) -> ExitCode {
    if !ignore_config
        && std::env::current_dir()
            .map(|dir| dir.join("tsconfig.json").exists())
            .unwrap_or(false)
    {
        let diagnostic = surge_ts_diagnostics::Diagnostic::ts5112("<command line>");
        let diagnostics = [diagnostic];
        match style {
            DiagnosticStyle::Json => {
                let json = serde_json::to_string_pretty(&render_single_file_diagnostics_json(
                    &file_path,
                    &diagnostics,
                    "",
                    max_diagnostics,
                ))
                .unwrap();
                println!("{}", json);
            }
            DiagnosticStyle::Custom => println!("{}", diagnostics[0].render("")),
            DiagnosticStyle::Tsc => print!(
                "{}",
                render_single_file_diagnostics_tsc(
                    &diagnostics,
                    "",
                    pretty,
                    color,
                    max_diagnostics
                )
            ),
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
            no_implicit_returns: false,
            no_fallthrough_cases_in_switch: false,
            no_implicit_override: false,
            no_property_access_from_index_signature: false,
            no_unused_locals: false,
            no_unused_parameters: false,
            no_lib,
            skip_lib_check: false,
            stub_external_modules,
            resolved_modules: std::collections::HashMap::new(),
            types: Vec::new(),
            diagnostic_profile,
        },
    );
    // `--showSpans` is a debug aid that forces the custom span renderer even
    // under the default tsc style; JSON output ignores it.
    let force_custom = matches!(style, DiagnosticStyle::Custom) || show_spans;
    match style {
        DiagnosticStyle::Json => println!(
            "{}",
            serde_json::to_string_pretty(&render_single_file_diagnostics_json(
                &file_path,
                &diagnostics,
                &source_text,
                max_diagnostics
            ))
            .unwrap()
        ),
        _ if force_custom => println!(
            "{}",
            render_single_file_diagnostics(
                &file_path,
                &diagnostics,
                &source_text,
                show_spans,
                max_diagnostics
            )
        ),
        DiagnosticStyle::Tsc => print!(
            "{}",
            render_single_file_diagnostics_tsc(
                &diagnostics,
                &source_text,
                pretty,
                color,
                max_diagnostics
            )
        ),
        DiagnosticStyle::Custom => unreachable!("custom handled by force_custom"),
    }

    ExitCode::SUCCESS
}

/// Render single-file diagnostics in tsc-compatible form. The display label is
/// derived from each diagnostic's own file name, so command-line diagnostics
/// (with no real file) collapse to a header-only line.
fn render_single_file_diagnostics_tsc(
    diagnostics: &[Diagnostic],
    source_text: &str,
    pretty: bool,
    color: bool,
    max_diagnostics: Option<usize>,
) -> String {
    let total = diagnostics.len();
    if total == 0 {
        return String::new();
    }
    let limit = max_diagnostics.unwrap_or(total).min(total);
    let shown = &diagnostics[..limit];

    let labels: Vec<String> = shown
        .iter()
        .map(|diagnostic| tsc_path_label(&diagnostic.file_name))
        .collect();
    let items: Vec<TscRenderItem> = shown
        .iter()
        .zip(&labels)
        .map(|(diagnostic, label)| TscRenderItem {
            label,
            source_text,
            diagnostic,
        })
        .collect();

    let mut out = render_diagnostics_tsc(&items, TscRenderOptions { pretty, color });
    if limit < total {
        out.push_str(&format!(
            "\nShowing first {limit} of {total} diagnostics.\n"
        ));
    }
    out
}

/// Whether physical TypeScript `lib*.d.ts` loading was explicitly requested via
/// the `--physicalLibs` flag, a `.physicalLibs` marker file beside the resolved
/// `tsconfig.json`, or the `SURGE_PHYSICAL_LIBS` env var. Physical
/// loading is now the default; this only controls whether a fallback warning is
/// surfaced when the TypeScript package is missing.
fn physical_libs_explicitly_requested(cli_flag: bool, config_path: &std::path::Path) -> bool {
    if cli_flag {
        return true;
    }
    if std::env::var_os("SURGE_PHYSICAL_LIBS").is_some() {
        return true;
    }
    config_path
        .parent()
        .map(|dir| dir.join(".physicalLibs").exists())
        .unwrap_or(false)
}

/// Map a configured `target` to the lib name base used to derive the default
/// `lib.<base>.full.d.ts` aggregate when `compilerOptions.lib` is unset.
fn target_lib_basename(target: ScriptTarget) -> &'static str {
    match target {
        ScriptTarget::ES2015 => "es2015",
        ScriptTarget::ES2016 => "es2016",
        ScriptTarget::ES2017 => "es2017",
        ScriptTarget::ES2018 => "es2018",
        ScriptTarget::ES2019 => "es2019",
        ScriptTarget::ES2020 => "es2020",
        ScriptTarget::ES2021 => "es2021",
        ScriptTarget::ES2022 => "es2022",
        ScriptTarget::ES2023 => "es2023",
        ScriptTarget::ES2024 => "es2024",
        ScriptTarget::ESNext => "esnext",
    }
}

#[allow(clippy::too_many_arguments)]
fn run_project_mode(
    project: PathBuf,
    show_config: bool,
    show_spans: bool,
    compat_report: bool,
    style: DiagnosticStyle,
    pretty: bool,
    color: bool,
    max_diagnostics: Option<usize>,
    jobs: usize,
    stub_external_modules: bool,
    diagnostic_profile: surge_ts_checker::DiagnosticProfile,
    physical_libs_flag: bool,
    timings_enabled: bool,
) -> ExitCode {
    let mut timings = CliTimings::default();

    let run_start = Instant::now();
    let config_start = run_start;
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
            timings.total = run_start.elapsed();
            render_cli_timings(&timings);
        }
        return ExitCode::SUCCESS;
    }

    if loaded.files.is_empty() {
        let diagnostics = vec![project_has_no_source_files_diagnostic(&loaded)];
        let stats = surge_ts_checker::CompatibilityStats::default();
        let exit_code = render_project_mode_output(
            &loaded,
            &diagnostics,
            &[],
            &stats,
            show_spans,
            compat_report,
            style,
            pretty,
            color,
            max_diagnostics,
            timings_enabled,
            &mut timings,
        );
        if timings_enabled {
            timings.total = run_start.elapsed();
            render_cli_timings(&timings);
        }
        return exit_code;
    }

    let file_discovery_start = Instant::now();
    let read_workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
        .max(jobs)
        .min(loaded.files.len());
    let source_read_nanos = std::sync::atomic::AtomicU64::new(0);
    let source_entries = match read_project_sources(&loaded.files, read_workers, &source_read_nanos)
    {
        Ok(entries) => entries,
        Err((file_path, error)) => {
            eprintln!("failed to read {}: {error}", file_path.display());
            return ExitCode::from(1);
        }
    };

    let mut inputs = Vec::with_capacity(source_entries.len());
    let mut sources = Vec::with_capacity(source_entries.len());
    for (file_path, file_name, source_text) in source_entries {
        inputs.push(SourceFileInput {
            file_name: file_name.clone(),
            source_text: source_text.clone(),
        });
        sources.push((file_path, file_name, source_text));
    }
    if timings_enabled {
        timings.file_discovery += file_discovery_start.elapsed();
        timings.source_read_io += std::time::Duration::from_nanos(
            source_read_nanos.load(std::sync::atomic::Ordering::Relaxed),
        );
        timings.source_files_read += inputs.len() as u64;
        timings.source_bytes_read += inputs
            .iter()
            .map(|input| input.source_text.len() as u64)
            .sum::<u64>();
    }

    let default_lib_loading_start = Instant::now();
    let default_lib_load = load_default_lib_inputs(DefaultLibRequest {
        no_lib: loaded.compiler_options.no_lib,
        lib_entries: loaded.compiler_options.lib.as_slice(),
        root_dir: &loaded.root_dir,
        target_basename: target_lib_basename(loaded.compiler_options.target),
    });
    for unknown in &default_lib_load.unknown_libs {
        eprintln!(
            "warning: unknown lib '{unknown}' in compilerOptions.lib; no matching lib*.d.ts file"
        );
    }
    // `--physicalLibs` (and its env/marker equivalents) is now only a debug aid:
    // physical loading is the default, so the flag merely surfaces a warning when
    // the TypeScript package could not be found and the generated subset was used.
    if physical_libs_explicitly_requested(physical_libs_flag, &loaded.config_path)
        && !default_lib_load.used_physical
        && !loaded.compiler_options.no_lib
    {
        eprintln!(
            "warning: --physicalLibs requested but no TypeScript package was found under node_modules; falling back to the generated default-lib subset"
        );
    }
    let default_lib_io = default_lib_load.io_stats;
    let default_lib_inputs = default_lib_load.inputs;
    if timings_enabled {
        timings.default_lib_loading += default_lib_loading_start.elapsed();
        timings.default_lib_files_read += default_lib_inputs.len() as u64;
        timings.default_lib_bytes_read += default_lib_inputs
            .iter()
            .map(|input| input.source_text.len() as u64)
            .sum::<u64>();
        timings.default_lib_read_io += default_lib_io.read_io;
        timings.default_lib_existence_probes += default_lib_io.existence_probes;
        timings.default_lib_canonicalize_syscalls += default_lib_io.canonicalize_syscalls;
    }

    let mut resolved_modules = std::collections::HashMap::new();
    let mut package_resolution_cache =
        package_declarations::PackageDeclarationResolverCache::default();

    let resolver_options = package_resolution::ResolverOptions {
        module_resolution: loaded.compiler_options.module_resolution,
        resolve_exports: loaded.compiler_options.resolve_package_json_exports,
        resolve_imports: loaded.compiler_options.resolve_package_json_imports,
        custom_conditions: loaded.compiler_options.custom_conditions.clone(),
    };

    let type_package_resolution = package_declarations::resolve_type_packages(
        &mut inputs,
        &mut sources,
        &loaded.root_dir,
        loaded.compiler_options.types.as_deref(),
        &loaded.compiler_options.type_roots,
        &mut package_resolution_cache,
    );

    // Explicit `/// <reference types="..." />` directives resolve through the same
    // type roots as `compilerOptions.types`. The resolver is re-run inside the
    // expansion loop so directives in dependency declaration files (added by
    // package resolution / import-graph expansion) participate too.
    let mut reference_type_resolver = package_declarations::ReferenceTypeDirectiveResolver::new(
        &loaded.root_dir,
        &loaded.compiler_options.type_roots,
    );

    loop {
        let files_before = inputs.len();

        let package_start = Instant::now();
        let package_modules =
            package_declarations::resolve_package_declaration_entrypoints_with_cache(
                &mut inputs,
                &mut sources,
                &loaded.root_dir,
                &resolver_options,
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

        reference_type_resolver.scan_and_resolve(
            &mut inputs,
            &mut sources,
            &mut package_resolution_cache,
        );

        if graph_loaded == 0 && inputs.len() == files_before {
            break;
        }
    }

    if timings_enabled {
        let io = io_stats::snapshot();
        timings.expansion_read_io += io.expansion_read_io;
        timings.expansion_files_read += io.expansion_files_read;
        timings.expansion_bytes_read += io.expansion_bytes_read;
        timings.package_json_reads += io.package_json_reads;
        timings.fs_existence_probes += io.fs_existence_probes;
        timings.fs_read_dir_count += io.fs_read_dir_count;
    }

    // Default-lib sources never contribute project imports or package specifiers,
    // so they stay out of the package-declaration / import-graph scan above (which
    // would otherwise re-parse ~3MB of lib `.d.ts` on every pass). Splice them to
    // the front now, preserving the original `[default libs..., project files...]`
    // order the checker expects.
    if !default_lib_inputs.is_empty() {
        let default_lib_sources = default_lib_inputs
            .iter()
            .map(|input| {
                (
                    PathBuf::from(&input.file_name),
                    input.file_name.clone(),
                    input.source_text.clone(),
                )
            })
            .collect::<Vec<_>>();
        inputs.splice(0..0, default_lib_inputs);
        sources.splice(0..0, default_lib_sources);
    }

    let reference_type_resolution = reference_type_resolver.into_resolution();

    // Pass the resolved type-package names to the checker so node-specific
    // builtins and the `@types` ambient-global gate fire for them. Reference-type
    // packages join the configured ones; when the project used the `"*"` wildcard,
    // keep the literal `"*"` sentinel so the checker selects the node install-hint
    // variant (TS2580 vs TS2591) like tsc's `usesWildcardTypes`.
    let mut checker_types = type_package_resolution.effective_type_names.clone();
    for name in &reference_type_resolution.effective_type_names {
        if !checker_types.contains(name) {
            checker_types.push(name.clone());
        }
    }
    if loaded
        .compiler_options
        .types
        .as_deref()
        .is_some_and(|types| types.iter().any(|name| name == "*"))
    {
        checker_types.push("*".to_string());
    }

    let path_mapping_start = Instant::now();
    let path_modules = path_mapping::resolve_path_mappings(
        &inputs,
        &loaded.compiler_options.paths,
        &loaded.root_dir,
    );

    for (k, v) in path_modules {
        resolved_modules.insert(k, v);
    }
    if timings_enabled {
        timings.path_mapping_resolution += path_mapping_start.elapsed();
    }

    let checker_options = CheckerOptions {
        no_implicit_any: loaded.compiler_options.no_implicit_any,
        no_implicit_returns: loaded.compiler_options.no_implicit_returns,
        no_fallthrough_cases_in_switch: loaded.compiler_options.no_fallthrough_cases_in_switch,
        no_implicit_override: loaded.compiler_options.no_implicit_override,
        no_property_access_from_index_signature: loaded.compiler_options.no_property_access_from_index_signature,
        no_unused_locals: loaded.compiler_options.no_unused_locals,
        no_unused_parameters: loaded.compiler_options.no_unused_parameters,
        no_lib: loaded.compiler_options.no_lib,
        skip_lib_check: loaded.compiler_options.skip_lib_check,
        stub_external_modules,
        resolved_modules,
        types: checker_types,
        diagnostic_profile,
    };

    let checking_start = Instant::now();
    let result = check_program_with_stats_and_jobs(inputs, checker_options, jobs);
    if timings_enabled {
        timings.checking += checking_start.elapsed();
    }
    let mut diagnostics = apply_project_no_lib_compatibility_diagnostics(
        result.diagnostics,
        loaded.compiler_options.no_lib,
        diagnostic_profile,
    );
    diagnostics.extend(
        type_package_resolution
            .missing
            .iter()
            .map(|type_name| Diagnostic::ts2688(type_name, String::new())),
    );
    for missing in &reference_type_resolution.missing {
        // tsc locates the TS2688 at the directive in its containing file. When that
        // file is a declaration file, the diagnostic is suppressed under
        // `skipLibCheck`, like any other `.d.ts` diagnostic.
        if loaded.compiler_options.skip_lib_check && missing.from_declaration_file {
            continue;
        }
        diagnostics.push(
            Diagnostic::ts2688(&missing.type_name, missing.file_name.clone()).with_span(
                surge_ts_diagnostics::TextSpan {
                    start: missing.value_span.start,
                    end: missing.value_span.end,
                },
            ),
        );
    }
    let exit_code = render_project_mode_output(
        &loaded,
        &diagnostics,
        &sources,
        &result.stats,
        show_spans,
        compat_report,
        style,
        pretty,
        color,
        max_diagnostics,
        timings_enabled,
        &mut timings,
    );
    if timings_enabled {
        timings.total = run_start.elapsed();
        render_cli_timings(&timings);
    }
    exit_code
}

type ProjectSource = (PathBuf, String, String);

fn read_one_source(
    file_path: &PathBuf,
    read_nanos: &std::sync::atomic::AtomicU64,
) -> Result<ProjectSource, (PathBuf, std::io::Error)> {
    let read_start = Instant::now();
    let read = fs::read_to_string(file_path);
    read_nanos.fetch_add(
        read_start.elapsed().as_nanos() as u64,
        std::sync::atomic::Ordering::Relaxed,
    );
    match read {
        Ok(source_text) => {
            let file_name = canonicalize_if_exists_string(file_path);
            Ok((file_path.clone(), file_name, source_text))
        }
        Err(error) => Err((file_path.clone(), error)),
    }
}

// Project source reads are I/O-bound and independent, so reading them across a
// few threads overlaps the waits. Contiguous chunks keep the original file
// ordering, which the checker relies on.
fn read_project_sources(
    files: &[PathBuf],
    workers: usize,
    read_nanos: &std::sync::atomic::AtomicU64,
) -> Result<Vec<ProjectSource>, (PathBuf, std::io::Error)> {
    if workers <= 1 || files.len() <= 1 {
        return files.iter().map(|f| read_one_source(f, read_nanos)).collect();
    }

    let chunk_size = (files.len() + workers - 1) / workers;
    let chunk_results: Vec<Result<Vec<ProjectSource>, (PathBuf, std::io::Error)>> =
        std::thread::scope(|scope| {
            let handles: Vec<_> = files
                .chunks(chunk_size)
                .map(|chunk| {
                    scope.spawn(move || {
                        chunk.iter().map(|f| read_one_source(f, read_nanos)).collect()
                    })
                })
                .collect();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("source reader thread panicked"))
                .collect()
        });

    let mut sources = Vec::with_capacity(files.len());
    for chunk in chunk_results {
        sources.extend(chunk?);
    }
    Ok(sources)
}

/// `0` is the checker's sentinel for automatic worker-count selection, so `auto`
/// maps to it while a literal `0` from the user is still rejected.
fn parse_jobs(value: &str) -> Result<usize, String> {
    if value.eq_ignore_ascii_case("auto") {
        return Ok(0);
    }

    let jobs = value
        .parse::<usize>()
        .map_err(|_| format!("invalid value for --jobs: {value}"))?;
    if jobs == 0 {
        return Err("--jobs must be greater than 0".to_string());
    }

    Ok(jobs)
}

#[allow(clippy::too_many_arguments)]
fn render_project_mode_output(
    loaded: &surge_ts_config::LoadedTsConfig,
    diagnostics: &[Diagnostic],
    sources: &[(PathBuf, String, String)],
    stats: &surge_ts_checker::CompatibilityStats,
    show_spans: bool,
    compat_report: bool,
    style: DiagnosticStyle,
    pretty: bool,
    color: bool,
    max_diagnostics: Option<usize>,
    timings_enabled: bool,
    timings: &mut CliTimings,
) -> ExitCode {
    let render_start = Instant::now();
    let json_output = matches!(style, DiagnosticStyle::Json);

    if compat_report {
        let report = build_project_compatibility_report(loaded, diagnostics, sources, stats);
        if json_output {
            println!(
                "{}",
                serde_json::to_string_pretty(&render_project_compatibility_report_json(&report))
                    .unwrap()
            );
        } else {
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
        if timings_enabled {
            timings.diagnostic_rendering += render_start.elapsed();
        }
        return ExitCode::SUCCESS;
    }

    // `--showSpans` is a debug aid that forces the custom span renderer even
    // under the default tsc style.
    let force_custom = matches!(style, DiagnosticStyle::Custom) || show_spans;
    match style {
        DiagnosticStyle::Json => {
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
        _ if force_custom => {
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
        DiagnosticStyle::Tsc => {
            print!(
                "{}",
                render_project_diagnostics_tsc(
                    diagnostics,
                    sources,
                    pretty,
                    color,
                    max_diagnostics
                )
            );
        }
        DiagnosticStyle::Custom => unreachable!("custom handled by force_custom"),
    }

    if timings_enabled {
        timings.diagnostic_rendering += render_start.elapsed();
    }

    ExitCode::SUCCESS
}

/// Render project diagnostics in tsc-compatible form. Diagnostics are grouped by
/// file in source-load order (matching the custom preview and tsc's file
/// ordering); diagnostics whose file is not among the loaded sources (such as
/// global "Cannot find global type" diagnostics) are appended afterwards with no
/// source excerpt.
fn render_project_diagnostics_tsc(
    diagnostics: &[Diagnostic],
    sources: &[(PathBuf, String, String)],
    pretty: bool,
    color: bool,
    max_diagnostics: Option<usize>,
) -> String {
    if diagnostics.is_empty() {
        return String::new();
    }

    let mut by_file: std::collections::HashMap<&str, Vec<&Diagnostic>> =
        std::collections::HashMap::new();
    for diagnostic in diagnostics {
        by_file
            .entry(diagnostic.file_name.as_str())
            .or_default()
            .push(diagnostic);
    }

    let mut ordered: Vec<(String, &str, &Diagnostic)> = Vec::with_capacity(diagnostics.len());
    for (_, file_name, source_text) in sources {
        if let Some(file_diagnostics) = by_file.remove(file_name.as_str()) {
            let label = tsc_path_label(file_name);
            for diagnostic in file_diagnostics {
                ordered.push((label.clone(), source_text.as_str(), diagnostic));
            }
        }
    }
    // Any diagnostics not attached to a loaded source file, in original order.
    if !by_file.is_empty() {
        for diagnostic in diagnostics {
            if by_file.contains_key(diagnostic.file_name.as_str()) {
                ordered.push((tsc_path_label(&diagnostic.file_name), "", diagnostic));
            }
        }
    }

    let total = ordered.len();
    let limit = max_diagnostics.unwrap_or(total).min(total);
    let truncated = limit < total;
    ordered.truncate(limit);

    let items: Vec<TscRenderItem> = ordered
        .iter()
        .map(|(label, source_text, diagnostic)| TscRenderItem {
            label,
            source_text,
            diagnostic,
        })
        .collect();

    let mut out = render_diagnostics_tsc(&items, TscRenderOptions { pretty, color });
    if truncated {
        out.push_str(&format!(
            "\nShowing first {limit} of {total} diagnostics.\n"
        ));
    }
    out
}

fn apply_project_no_lib_compatibility_diagnostics(
    diagnostics: Vec<Diagnostic>,
    no_lib: bool,
    diagnostic_profile: surge_ts_checker::DiagnosticProfile,
) -> Vec<Diagnostic> {
    if !no_lib || diagnostic_profile != surge_ts_checker::DiagnosticProfile::Tsc {
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
        "  default_lib_loading: {}",
        format_duration(timings.default_lib_loading)
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
        "  path_mapping_resolution: {}",
        format_duration(timings.path_mapping_resolution)
    );
    eprintln!("  checking: {}", format_duration(timings.checking));
    eprintln!(
        "  diagnostic_rendering: {}",
        format_duration(timings.diagnostic_rendering)
    );
    let accounted = timings.config_project_loading
        + timings.file_discovery
        + timings.default_lib_loading
        + timings.package_declaration_discovery
        + timings.import_graph_expansion
        + timings.path_mapping_resolution
        + timings.checking
        + timings.diagnostic_rendering;
    eprintln!("  total: {}", format_duration(timings.total));
    eprintln!(
        "  unaccounted: {}",
        format_duration(timings.total.saturating_sub(accounted))
    );
    eprintln!("  io:");
    eprintln!(
        "    source_read_io: {}",
        format_duration(timings.source_read_io)
    );
    eprintln!("    source_files_read: {}", timings.source_files_read);
    eprintln!(
        "    source_bytes_read: {} ({})",
        timings.source_bytes_read,
        format_throughput(timings.source_bytes_read, timings.source_read_io)
    );
    eprintln!(
        "    default_lib_files_read: {}",
        timings.default_lib_files_read
    );
    eprintln!(
        "    default_lib_bytes_read: {}",
        timings.default_lib_bytes_read
    );
    eprintln!(
        "    default_lib_read_io: {}",
        format_duration(timings.default_lib_read_io)
    );
    eprintln!(
        "    default_lib_existence_probes: {}",
        timings.default_lib_existence_probes
    );
    eprintln!(
        "    default_lib_canonicalize_syscalls: {}",
        timings.default_lib_canonicalize_syscalls
    );
    eprintln!(
        "    expansion_read_io: {}",
        format_duration(timings.expansion_read_io)
    );
    eprintln!("    expansion_files_read: {}", timings.expansion_files_read);
    eprintln!(
        "    expansion_bytes_read: {} ({})",
        timings.expansion_bytes_read,
        format_throughput(timings.expansion_bytes_read, timings.expansion_read_io)
    );
    eprintln!("    package_json_reads: {}", timings.package_json_reads);
    eprintln!("    fs_existence_probes: {}", timings.fs_existence_probes);
    eprintln!("    fs_read_dir_count: {}", timings.fs_read_dir_count);
}

fn format_throughput(bytes: u64, elapsed: std::time::Duration) -> String {
    let seconds = elapsed.as_secs_f64();
    if seconds <= 0.0 || bytes == 0 {
        return "-".to_string();
    }
    let mib_per_sec = (bytes as f64 / (1024.0 * 1024.0)) / seconds;
    format!("{mib_per_sec:.1} MiB/s")
}

fn format_duration(duration: std::time::Duration) -> String {
    format!("{:.3}ms", duration.as_secs_f64() * 1000.0)
}

fn render_single_file_diagnostics(
    file_path: &std::path::Path,
    diagnostics: &[surge_ts_diagnostics::Diagnostic],
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
    diagnostics: &[surge_ts_diagnostics::Diagnostic],
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
    diagnostic: &surge_ts_diagnostics::Diagnostic,
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

fn build_show_config_json(loaded: &surge_ts_config::LoadedTsConfig) -> Value {
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

fn project_has_no_source_files_diagnostic(loaded: &surge_ts_config::LoadedTsConfig) -> Diagnostic {
    Diagnostic::new(
        DiagnosticCode::Custom("surge::project-has-no-source-files"),
        format!(
            "no source files were discovered for {}",
            loaded.config_path.display()
        ),
        loaded.config_path.display().to_string(),
    )
}

fn render_diagnostics_with_spans(
    file_path: &std::path::Path,
    diagnostics: &[surge_ts_diagnostics::Diagnostic],
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
    diagnostic: &surge_ts_diagnostics::Diagnostic,
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
    compiler_options: &surge_ts_config::NormalizedCompilerOptions,
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
    if !compiler_options.lib.is_empty() {
        options.insert(
            "lib".to_string(),
            Value::Array(
                compiler_options
                    .lib
                    .iter()
                    .map(|lib| Value::String(lib.clone()))
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
    if let Some(types) = &compiler_options.types {
        options.insert(
            "types".to_string(),
            Value::Array(types.iter().map(|ty| Value::String(ty.clone())).collect()),
        );
    }

    Value::Object(options)
}

fn script_target_to_string(target: surge_ts_config::ScriptTarget) -> &'static str {
    match target {
        surge_ts_config::ScriptTarget::ES2015 => "es2015",
        surge_ts_config::ScriptTarget::ES2016 => "es2016",
        surge_ts_config::ScriptTarget::ES2017 => "es2017",
        surge_ts_config::ScriptTarget::ES2018 => "es2018",
        surge_ts_config::ScriptTarget::ES2019 => "es2019",
        surge_ts_config::ScriptTarget::ES2020 => "es2020",
        surge_ts_config::ScriptTarget::ES2021 => "es2021",
        surge_ts_config::ScriptTarget::ES2022 => "es2022",
        surge_ts_config::ScriptTarget::ES2023 => "es2023",
        surge_ts_config::ScriptTarget::ES2024 => "es2024",
        surge_ts_config::ScriptTarget::ESNext => "esnext",
    }
}

fn module_kind_to_string(module: surge_ts_config::ModuleKind) -> &'static str {
    match module {
        surge_ts_config::ModuleKind::CommonJS => "commonjs",
        surge_ts_config::ModuleKind::ES2015 => "es2015",
        surge_ts_config::ModuleKind::ES2020 => "es2020",
        surge_ts_config::ModuleKind::ES2022 => "es2022",
        surge_ts_config::ModuleKind::ESNext => "esnext",
        surge_ts_config::ModuleKind::Node16 => "node16",
        surge_ts_config::ModuleKind::Node18 => "node18",
        surge_ts_config::ModuleKind::Node20 => "node20",
        surge_ts_config::ModuleKind::NodeNext => "nodenext",
        surge_ts_config::ModuleKind::Preserve => "preserve",
    }
}

fn module_resolution_kind_to_string(
    module_resolution: surge_ts_config::ModuleResolutionKind,
) -> &'static str {
    match module_resolution {
        surge_ts_config::ModuleResolutionKind::Node16 => "node16",
        surge_ts_config::ModuleResolutionKind::Node20 => "node20",
        surge_ts_config::ModuleResolutionKind::NodeNext => "nodenext",
        surge_ts_config::ModuleResolutionKind::Bundler => "bundler",
    }
}

fn jsx_mode_to_string(jsx: &surge_ts_config::JsxMode) -> &'static str {
    match jsx {
        surge_ts_config::JsxMode::Preserve => "preserve",
        surge_ts_config::JsxMode::React => "react",
        surge_ts_config::JsxMode::ReactJsx => "react-jsx",
        surge_ts_config::JsxMode::ReactJsxDev => "react-jsxdev",
    }
}
