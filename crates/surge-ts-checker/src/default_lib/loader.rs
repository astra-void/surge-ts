use std::path::{Path, PathBuf};

use crate::SourceFileInput;

use super::embedded::bundled_typescript_version;
use super::physical::{
    DefaultLibIoStats, default_full_lib_seed_for_target, find_typescript_lib_dir,
    resolve_default_libs_from_source,
};
use super::provider::{DirectoryLibSource, EmbeddedLibSource, LibSource};

/// Which declarations the standard library is loaded from.
///
/// Precedence is explicit and deterministic: an override the user asked for
/// wins, and otherwise the bundled snapshot is used. An installed `typescript`
/// package is never picked up implicitly, so a given surge release checks a
/// given project against the same declarations on every machine.
#[derive(Debug, Clone, Default)]
pub enum LibSourceChoice {
    /// The version-pinned snapshot compiled into the binary.
    #[default]
    Bundled,
    /// An explicit lib directory (`--typescript-lib-path`).
    Directory(PathBuf),
    /// Explicitly opt back in to the project's installed TypeScript
    /// (`--physicalLibs`), discovered by walking up from the project root.
    InstalledTypeScript,
}

pub struct DefaultLibRequest<'a> {
    /// `compilerOptions.noLib`. When true, no default libs are loaded at all.
    pub no_lib: bool,
    /// `compilerOptions.lib`. Empty means "derive the lib set from `target`".
    pub lib_entries: &'a [String],
    /// Project root, used to discover an installed TypeScript when asked.
    pub root_dir: &'a Path,
    /// Target lib basename (e.g. `"es2022"`) used to derive the implicit
    /// `lib.<base>.full.d.ts` seed when `lib_entries` is empty.
    pub target_basename: &'a str,
    /// Where the declarations come from.
    pub source: LibSourceChoice,
}

#[derive(Debug, Default)]
pub struct DefaultLibLoad {
    /// Source inputs in deterministic, dependency-first load order.
    pub inputs: Vec<SourceFileInput>,
    /// True when the bundled snapshot supplied the libs.
    pub used_bundled: bool,
    /// Human-readable description of the source actually used.
    pub source_description: String,
    /// Set when an override was requested but could not be used, explaining why
    /// the bundled snapshot was used instead.
    pub override_error: Option<String>,
    /// `compilerOptions.lib` entries with no matching `lib*.d.ts` file.
    pub unknown_libs: Vec<String>,
    /// Filesystem I/O incurred while resolving the lib graph.
    pub io_stats: DefaultLibIoStats,
}

pub fn load_default_lib_inputs(request: DefaultLibRequest<'_>) -> DefaultLibLoad {
    let seed = default_full_lib_seed_for_target(request.target_basename);

    if request.no_lib {
        // Nothing is loaded, so resolving a source would only cost a
        // filesystem walk and produce a warning the caller must suppress.
        return DefaultLibLoad {
            used_bundled: true,
            source_description: "none (noLib)".to_string(),
            ..Default::default()
        };
    }

    let (source, override_error): (Option<DirectoryLibSource>, Option<String>) = match &request
        .source
    {
        LibSourceChoice::Bundled => (None, None),
        LibSourceChoice::Directory(dir) => {
            if dir.join("lib.es5.d.ts").is_file() {
                (Some(DirectoryLibSource::new(dir.clone())), None)
            } else {
                (
                    None,
                    Some(format!(
                        "--typescript-lib-path {} has no lib.es5.d.ts; using the bundled TypeScript {} libs",
                        dir.display(),
                        bundled_typescript_version()
                    )),
                )
            }
        }
        LibSourceChoice::InstalledTypeScript => match find_typescript_lib_dir(request.root_dir) {
            Some(dir) => (Some(DirectoryLibSource::new(dir)), None),
            None => (
                None,
                Some(format!(
                    "--physicalLibs requested but no TypeScript package with lib*.d.ts was found under node_modules; using the bundled TypeScript {} libs",
                    bundled_typescript_version()
                )),
            ),
        },
    };

    let embedded = EmbeddedLibSource;
    let active: &dyn LibSource = match &source {
        Some(directory) => directory,
        None => &embedded,
    };

    let resolution = resolve_default_libs_from_source(
        active,
        request.no_lib,
        request.lib_entries,
        &seed,
        DefaultLibIoStats::default(),
    );

    DefaultLibLoad {
        inputs: resolution.inputs,
        used_bundled: source.is_none(),
        source_description: active.describe(),
        override_error,
        unknown_libs: resolution.unknown_libs,
        io_stats: resolution.io_stats,
    }
}

/// Load the bundled default libs directly, for callers with no project config
/// (single-file mode and the fixture drivers).
pub fn load_generated_default_lib_inputs(
    no_lib: bool,
    lib_entries: Option<&[String]>,
) -> Vec<SourceFileInput> {
    resolve_default_libs_from_source(
        &EmbeddedLibSource,
        no_lib,
        lib_entries.unwrap_or_default(),
        &default_full_lib_seed_for_target("es2024"),
        DefaultLibIoStats::default(),
    )
    .inputs
}
