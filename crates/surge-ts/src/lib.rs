//! Embeddable TypeScript noEmit compatibility checker.
//!
//! This is the umbrella crate for `surge-ts`. It re-exports the in-memory
//! checking API from [`surge_ts_checker`] and adds [`Project`] — full
//! `tsconfig.json` project checking (config loading, package/`paths`/reference
//! resolution, default-lib loading, and the import-graph fixpoint) behind a
//! single call:
//!
//! ```no_run
//! use surge_ts::{Project, ProjectOptions};
//!
//! let project = Project::load("tsconfig.json");
//! let result = project.check(&ProjectOptions::default());
//! for diagnostic in &result.diagnostics {
//!     println!("{}: {}", diagnostic.code, diagnostic.message);
//! }
//! ```
//!
//! For in-memory, single- or multi-file checking without a tsconfig, use the
//! re-exported [`Checker`] builder directly.

mod import_graph;
mod io_stats;
mod package_declarations;
mod package_resolution;
mod path_mapping;
mod probe;
mod specifier_scan;

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub use surge_ts_checker::{
    CheckResult, Checker, CheckerOptions, CompatibilityStats, Diagnostic, DiagnosticCategory,
    DiagnosticCode, DiagnosticProfile, FileKind, ProgramCheckResult, SourceFileInput, TextSpan,
};
pub use surge_ts_config::{ConfigDiagnostic, LoadedTsConfig, ScriptTarget, TsConfigLoadOptions};

use surge_ts_checker::lowlevel::{DefaultLibRequest, load_default_lib_inputs};
use surge_ts_config::{canonicalize_if_exists_string, load_tsconfig};

/// A resolved project source file: `(path, canonical file name, source text)`.
/// Returned alongside diagnostics so callers can render code frames.
pub type ProjectSource = (PathBuf, String, String);

/// Options for [`Project::check`]. Strictness flags and module/lib options come
/// from the loaded `tsconfig.json`; these are the run-level knobs the config
/// does not carry.
#[derive(Debug, Clone)]
pub struct ProjectOptions {
    /// Worker threads for checking. `0` selects automatically; otherwise the
    /// literal count is used.
    pub jobs: usize,
    /// Suppress non-relative (package) missing-module diagnostics.
    pub stub_external_modules: bool,
    /// `Tsc` (default) or `Native` diagnostic output.
    pub diagnostic_profile: DiagnosticProfile,
    /// Whether physical `lib*.d.ts` loading was explicitly requested. Physical
    /// loading is the default; this only controls whether a fallback warning is
    /// surfaced when the TypeScript package is missing.
    pub physical_libs_requested: bool,
    /// Populate [`ProjectCheckResult::timings`]. Off by default to avoid the
    /// per-step `Instant` and global counter overhead.
    pub collect_timings: bool,
}

impl Default for ProjectOptions {
    fn default() -> Self {
        Self {
            jobs: 0,
            stub_external_modules: false,
            diagnostic_profile: DiagnosticProfile::default(),
            physical_libs_requested: false,
            collect_timings: false,
        }
    }
}

/// Per-phase timing and I/O counters for a project check. Populated only when
/// [`ProjectOptions::collect_timings`] is set; all zero otherwise.
#[derive(Debug, Clone, Default)]
pub struct ProjectTimings {
    pub file_discovery: Duration,
    pub default_lib_loading: Duration,
    pub package_declaration_discovery: Duration,
    pub import_graph_expansion: Duration,
    pub path_mapping_resolution: Duration,
    pub checking: Duration,
    pub source_read_io: Duration,
    pub source_files_read: u64,
    pub source_bytes_read: u64,
    pub default_lib_files_read: u64,
    pub default_lib_bytes_read: u64,
    pub default_lib_read_io: Duration,
    pub default_lib_existence_probes: u64,
    pub default_lib_canonicalize_syscalls: u64,
    pub expansion_read_io: Duration,
    pub expansion_files_read: u64,
    pub expansion_bytes_read: u64,
    pub package_declaration_read_io: Duration,
    pub package_declaration_probes: u64,
    pub package_declaration_probe_io: Duration,
    pub package_json_reads: u64,
    pub fs_existence_probes: u64,
    pub fs_existence_probe_io: Duration,
    pub fs_read_dir_count: u64,
    pub fs_read_dir_io: Duration,
    pub canonicalize_memo_misses: u64,
    pub canonicalize_full_realpaths: u64,
    pub canonicalize_leaf_probes: u64,
    pub canonicalize_miss_io: Duration,
}

/// Outcome of [`Project::check`]: diagnostics, tsc-compatibility stats, the
/// resolved source set (for rendering), any non-fatal warnings, and optional
/// timings.
#[derive(Debug, Clone)]
pub struct ProjectCheckResult {
    pub diagnostics: Vec<Diagnostic>,
    pub stats: CompatibilityStats,
    pub sources: Vec<ProjectSource>,
    pub warnings: Vec<String>,
    pub timings: ProjectTimings,
}

/// A failure that prevents a project check from running.
#[derive(Debug)]
pub enum ProjectError {
    /// A project source file could not be read.
    SourceRead {
        path: PathBuf,
        error: std::io::Error,
    },
}

impl std::fmt::Display for ProjectError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProjectError::SourceRead { path, error } => {
                write!(f, "failed to read {}: {error}", path.display())
            }
        }
    }
}

impl std::error::Error for ProjectError {}

/// A loaded `tsconfig.json` project, ready to check.
///
/// [`Project::load`] performs config discovery and normalization only;
/// [`Project::check`] runs source reading, module/type resolution, default-lib
/// loading, and the type check.
#[derive(Debug, Clone)]
pub struct Project {
    loaded: LoadedTsConfig,
}

impl Project {
    /// Load and normalize a `tsconfig.json`. Configuration-level diagnostics
    /// (parse errors, unknown options) are captured in [`Self::config_diagnostics`]
    /// rather than returned as an error.
    pub fn load(tsconfig_path: impl AsRef<Path>) -> Self {
        surge_ts_config::clear_canonicalize_cache();
        let loaded = load_tsconfig(TsConfigLoadOptions {
            project: tsconfig_path.as_ref().to_path_buf(),
        });
        Self { loaded }
    }

    /// The normalized config, including discovered files and compiler options.
    pub fn config(&self) -> &LoadedTsConfig {
        &self.loaded
    }

    /// Diagnostics produced while loading the config (parse/normalization).
    pub fn config_diagnostics(&self) -> &[ConfigDiagnostic] {
        &self.loaded.diagnostics
    }

    /// Whether the project discovered no input files.
    pub fn is_empty(&self) -> bool {
        self.loaded.files.is_empty()
    }

    /// Read sources, resolve modules/types/default-libs, and type-check the
    /// program. Returns [`ProjectError::SourceRead`] if a discovered file cannot
    /// be read.
    pub fn check(&self, options: &ProjectOptions) -> Result<ProjectCheckResult, ProjectError> {
        let loaded = &self.loaded;
        let mut timings = ProjectTimings::default();
        let collect = options.collect_timings;
        let mut warnings = Vec::new();

        // `io_stats` counters are process-global and accumulate across calls, so
        // take a baseline and report the delta for this check rather than the raw
        // running totals.
        let io_baseline = if collect {
            io_stats::snapshot()
        } else {
            io_stats::IoSnapshot::default()
        };
        let canonicalize_baseline = if collect {
            surge_ts_config::canonicalize_io_snapshot()
        } else {
            surge_ts_config::CanonicalizeIoSnapshot::default()
        };

        if loaded.files.is_empty() {
            return Ok(ProjectCheckResult {
                diagnostics: Vec::new(),
                stats: CompatibilityStats::default(),
                sources: Vec::new(),
                warnings,
                timings,
            });
        }

        probe::clear_probe_cache();

        let file_discovery_start = Instant::now();
        let read_workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .max(options.jobs)
            .min(loaded.files.len());
        let source_read_nanos = std::sync::atomic::AtomicU64::new(0);
        let source_entries = read_project_sources(&loaded.files, read_workers, &source_read_nanos)
            .map_err(|(path, error)| ProjectError::SourceRead { path, error })?;

        let mut inputs = Vec::with_capacity(source_entries.len());
        let mut sources = Vec::with_capacity(source_entries.len());
        for (file_path, file_name, source_text) in source_entries {
            inputs.push(SourceFileInput {
                file_name: file_name.clone(),
                source_text: source_text.clone(),
            });
            sources.push((file_path, file_name, source_text));
        }
        surge_ts_checker::lowlevel::record_loader_rss_stage("sources_read");
        if collect {
            timings.file_discovery += file_discovery_start.elapsed();
            timings.source_read_io +=
                Duration::from_nanos(source_read_nanos.load(std::sync::atomic::Ordering::Relaxed));
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
            warnings.push(format!(
                "unknown lib '{unknown}' in compilerOptions.lib; no matching lib*.d.ts file"
            ));
        }
        if options.physical_libs_requested
            && !default_lib_load.used_physical
            && !loaded.compiler_options.no_lib
        {
            warnings.push(
                "--physicalLibs requested but no TypeScript package was found under node_modules; falling back to the generated default-lib subset".to_string(),
            );
        }
        let default_lib_io = default_lib_load.io_stats;
        let default_lib_inputs = default_lib_load.inputs;
        if collect {
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
        surge_ts_checker::lowlevel::record_loader_rss_stage("default_libs_loaded");

        let mut resolved_modules = surge_ts_types::fx::FxHashMap::default();
        let mut resolved_modules_by_importer: surge_ts_types::fx::FxHashMap<
            String,
            surge_ts_types::fx::FxHashMap<String, String>,
        > = surge_ts_types::fx::FxHashMap::default();
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

        let mut reference_type_resolver = package_declarations::ReferenceTypeDirectiveResolver::new(
            &loaded.root_dir,
            &loaded.compiler_options.type_roots,
        );

        let mut specifier_scanner = specifier_scan::ModuleSpecifierScanner::new();
        let mut import_graph_state = import_graph::ImportGraphState::default();

        loop {
            let files_before = inputs.len();

            let package_start = Instant::now();
            let package_io_before = if collect {
                io_stats::snapshot()
            } else {
                io_stats::IoSnapshot::default()
            };
            let package_modules =
                package_declarations::resolve_package_declaration_entrypoints_with_cache(
                    &mut inputs,
                    &mut sources,
                    &loaded.root_dir,
                    &resolver_options,
                    &mut package_resolution_cache,
                    &mut specifier_scanner,
                );
            if collect {
                timings.package_declaration_discovery += package_start.elapsed();
                let package_io = io_stats::snapshot();
                timings.package_declaration_probes +=
                    package_io.fs_existence_probes - package_io_before.fs_existence_probes;
                timings.package_declaration_probe_io += package_io
                    .fs_existence_probe_io
                    .saturating_sub(package_io_before.fs_existence_probe_io);
            }
            // Package resolutions are importer-scoped; the flat map keeps the
            // first (BFS-order) resolution per specifier as the project-wide
            // fallback for importer-agnostic consumers.
            for (importer, specifier, resolved_file) in package_modules {
                resolved_modules
                    .entry(specifier.clone())
                    .or_insert_with(|| resolved_file.clone());
                resolved_modules_by_importer
                    .entry(importer)
                    .or_default()
                    .insert(specifier, resolved_file);
            }

            let import_graph_start = Instant::now();
            let graph_loaded = import_graph::expand_project_inputs(
                &mut import_graph_state,
                &mut specifier_scanner,
                &mut inputs,
                &mut sources,
                &loaded.root_dir,
                loaded.compiler_options.base_url.as_deref(),
                &loaded.compiler_options.paths,
            );
            if collect {
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
        surge_ts_checker::lowlevel::record_loader_rss_stage("import_graph_expanded");
        io_stats::report_probe_dirs();

        if collect {
            let io = io_stats::snapshot();
            timings.expansion_read_io += io
                .expansion_read_io
                .saturating_sub(io_baseline.expansion_read_io);
            timings.expansion_files_read +=
                io.expansion_files_read - io_baseline.expansion_files_read;
            timings.expansion_bytes_read +=
                io.expansion_bytes_read - io_baseline.expansion_bytes_read;
            timings.package_declaration_read_io += io
                .package_declaration_read_io
                .saturating_sub(io_baseline.package_declaration_read_io);
            timings.package_json_reads += io.package_json_reads - io_baseline.package_json_reads;
            timings.fs_existence_probes += io.fs_existence_probes - io_baseline.fs_existence_probes;
            timings.fs_existence_probe_io += io
                .fs_existence_probe_io
                .saturating_sub(io_baseline.fs_existence_probe_io);
            timings.fs_read_dir_count += io.fs_read_dir_count - io_baseline.fs_read_dir_count;
            timings.fs_read_dir_io += io.fs_read_dir_io.saturating_sub(io_baseline.fs_read_dir_io);
            let canonicalize = surge_ts_config::canonicalize_io_snapshot();
            timings.canonicalize_memo_misses +=
                canonicalize.memo_misses - canonicalize_baseline.memo_misses;
            timings.canonicalize_full_realpaths +=
                canonicalize.full_realpaths - canonicalize_baseline.full_realpaths;
            timings.canonicalize_leaf_probes +=
                canonicalize.leaf_probes - canonicalize_baseline.leaf_probes;
            timings.canonicalize_miss_io += canonicalize
                .miss_io
                .saturating_sub(canonicalize_baseline.miss_io);
        }

        // Default-lib sources never contribute project imports or package
        // specifiers, so they stay out of the package-declaration / import-graph
        // scan above. Splice them to the front now, preserving the
        // `[default libs..., project files...]` order the checker expects.
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

        // The scan caches (parser arena, per-file specifier lists, probe and
        // known-file sets) are loader-lifetime only; release them before the
        // checker's peak so they never count against the program footprint.
        drop(specifier_scanner);
        drop(import_graph_state);
        probe::clear_probe_cache();

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
            loaded.compiler_options.base_url.as_deref(),
            &loaded.root_dir,
        );
        for (k, v) in path_modules {
            resolved_modules.insert(k, v);
        }
        if loaded.compiler_options.allow_synthetic_default_imports {
            resolved_modules.insert(
                CheckerOptions::ALLOW_SYNTHETIC_DEFAULT_IMPORTS_SENTINEL.to_string(),
                String::new(),
            );
        }
        if collect {
            timings.path_mapping_resolution += path_mapping_start.elapsed();
        }

        let checker_options = CheckerOptions {
            no_implicit_any: loaded.compiler_options.no_implicit_any,
            no_implicit_returns: loaded.compiler_options.no_implicit_returns,
            no_fallthrough_cases_in_switch: loaded.compiler_options.no_fallthrough_cases_in_switch,
            no_implicit_override: loaded.compiler_options.no_implicit_override,
            no_property_access_from_index_signature: loaded
                .compiler_options
                .no_property_access_from_index_signature,
            no_unused_locals: loaded.compiler_options.no_unused_locals,
            no_unused_parameters: loaded.compiler_options.no_unused_parameters,
            no_lib: loaded.compiler_options.no_lib,
            skip_lib_check: loaded.compiler_options.skip_lib_check,
            stub_external_modules: options.stub_external_modules,
            resolved_modules,
            resolved_modules_by_importer,
            types: checker_types,
            jsx_automatic_runtime: matches!(
                loaded.compiler_options.jsx,
                Some(surge_ts_config::JsxMode::ReactJsx | surge_ts_config::JsxMode::ReactJsxDev)
            ),
            diagnostic_profile: options.diagnostic_profile,
        };

        let checking_start = Instant::now();
        let result = Checker::new()
            .options(checker_options)
            .jobs(options.jobs)
            .check(inputs);
        if collect {
            timings.checking += checking_start.elapsed();
        }

        let mut diagnostics = apply_project_no_lib_compatibility_diagnostics(
            result.diagnostics,
            loaded.compiler_options.no_lib,
            !loaded.compiler_options.type_roots.is_empty(),
            options.diagnostic_profile,
        );
        diagnostics.extend(
            type_package_resolution
                .missing
                .iter()
                .map(|type_name| Diagnostic::ts2688(type_name, String::new())),
        );
        for missing in &reference_type_resolution.missing {
            if loaded.compiler_options.skip_lib_check && missing.from_declaration_file {
                continue;
            }
            diagnostics.push(
                Diagnostic::ts2688(&missing.type_name, missing.file_name.clone()).with_span(
                    TextSpan {
                        start: missing.value_span.start,
                        end: missing.value_span.end,
                    },
                ),
            );
        }

        Ok(ProjectCheckResult {
            diagnostics,
            stats: result.stats,
            sources,
            warnings,
            timings,
        })
    }
}

type SourceReadResult = Result<ProjectSource, (PathBuf, std::io::Error)>;

fn read_one_source(
    file_path: &PathBuf,
    read_nanos: &std::sync::atomic::AtomicU64,
) -> SourceReadResult {
    let read_start = Instant::now();
    let read = std::fs::read_to_string(file_path);
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
        return files
            .iter()
            .map(|f| read_one_source(f, read_nanos))
            .collect();
    }

    let chunk_size = files.len().div_ceil(workers);
    let chunk_results: Vec<Result<Vec<ProjectSource>, (PathBuf, std::io::Error)>> =
        std::thread::scope(|scope| {
            let handles: Vec<_> = files
                .chunks(chunk_size)
                .map(|chunk| {
                    scope.spawn(move || {
                        chunk
                            .iter()
                            .map(|f| read_one_source(f, read_nanos))
                            .collect()
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

fn apply_project_no_lib_compatibility_diagnostics(
    diagnostics: Vec<Diagnostic>,
    no_lib: bool,
    provides_global_lib: bool,
    diagnostic_profile: DiagnosticProfile,
) -> Vec<Diagnostic> {
    if !no_lib || diagnostic_profile != DiagnosticProfile::Tsc {
        return diagnostics;
    }

    let mut filtered = diagnostics
        .into_iter()
        .filter(|diagnostic| !matches!(diagnostic.code, DiagnosticCode::TypeScript(2304)))
        .collect::<Vec<_>>();

    // tsc emits "Cannot find global type 'Array'/..." only when the program
    // supplies no replacement for the libraries `noLib` removed. A project with
    // explicit `typeRoots` (e.g. roblox-ts's `@rbxts/types`) provides those
    // globals itself, so tsc reports none — we must not fabricate them. surge
    // does not model those replacement declarations, so the globals are treated
    // as present rather than checked individually.
    if !provides_global_lib {
        filtered.extend(project_no_lib_missing_global_type_diagnostics());
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(code: u32) -> Diagnostic {
        Diagnostic::new(
            DiagnosticCode::TypeScript(code),
            String::new(),
            String::new(),
        )
    }

    fn codes(diagnostics: &[Diagnostic]) -> Vec<u32> {
        diagnostics
            .iter()
            .filter_map(|d| match d.code {
                DiagnosticCode::TypeScript(code) => Some(code),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn no_lib_without_global_lib_drops_2304_and_fabricates_2318() {
        let out = apply_project_no_lib_compatibility_diagnostics(
            vec![ts(2304), ts(2304), ts(2339)],
            true,
            false,
            DiagnosticProfile::Tsc,
        );
        let out = codes(&out);
        assert!(!out.contains(&2304));
        assert!(out.contains(&2339));
        assert_eq!(out.iter().filter(|&&c| c == 2318).count(), 10);
    }

    // roblox-ts and other custom-lib projects set explicit `typeRoots` to supply
    // the globals `noLib` removed; tsc reports no missing-global-type diagnostics,
    // so neither should we.
    #[test]
    fn no_lib_with_global_lib_drops_2304_and_fabricates_no_2318() {
        let out = apply_project_no_lib_compatibility_diagnostics(
            vec![ts(2304), ts(2339)],
            true,
            true,
            DiagnosticProfile::Tsc,
        );
        let out = codes(&out);
        assert!(!out.contains(&2304));
        assert!(out.contains(&2339));
        assert!(!out.contains(&2318));
    }

    #[test]
    fn lib_present_passes_diagnostics_through_untouched() {
        let out = apply_project_no_lib_compatibility_diagnostics(
            vec![ts(2304), ts(2339)],
            false,
            false,
            DiagnosticProfile::Tsc,
        );
        assert_eq!(codes(&out), vec![2304, 2339]);
    }
}
