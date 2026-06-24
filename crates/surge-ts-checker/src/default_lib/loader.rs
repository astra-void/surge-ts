use std::path::{Path, PathBuf};

use crate::SourceFileInput;

use super::physical::{
    DefaultLibIoStats, default_full_lib_seed_for_target, resolve_default_libs_from_lib_dir,
    resolve_physical_default_libs,
};

pub struct DefaultLibRequest<'a> {
    /// `compilerOptions.noLib`. When true, no default libs are loaded at all.
    pub no_lib: bool,
    /// `compilerOptions.lib`. Empty means "derive the lib set from `target`".
    pub lib_entries: &'a [String],
    /// Project root used to locate `node_modules/typescript/lib`.
    pub root_dir: &'a Path,
    /// Target lib basename (e.g. `"es2022"`) used to derive the implicit
    /// `lib.<base>.full.d.ts` seed when `lib_entries` is empty.
    pub target_basename: &'a str,
}

#[derive(Debug, Default)]
pub struct DefaultLibLoad {
    /// Source inputs in deterministic, dependency-first load order.
    pub inputs: Vec<SourceFileInput>,
    /// True when a local TypeScript package supplied the libs.
    pub used_physical: bool,
    /// `compilerOptions.lib` entries with no matching `lib*.d.ts` file.
    pub unknown_libs: Vec<String>,
    /// Filesystem I/O incurred while resolving the lib graph.
    pub io_stats: DefaultLibIoStats,
}

pub fn load_default_lib_inputs(request: DefaultLibRequest<'_>) -> DefaultLibLoad {
    let seed = default_full_lib_seed_for_target(request.target_basename);
    if let Some(resolution) =
        resolve_physical_default_libs(request.root_dir, request.no_lib, request.lib_entries, &seed)
    {
        return DefaultLibLoad {
            inputs: resolution.inputs,
            used_physical: true,
            unknown_libs: resolution.unknown_libs,
            io_stats: resolution.io_stats,
        };
    }

    let fallback = load_generated_default_lib_resolution(
        request.no_lib,
        request.lib_entries,
        request.target_basename,
    );
    DefaultLibLoad {
        inputs: fallback.inputs,
        used_physical: false,
        unknown_libs: fallback.unknown_libs,
        io_stats: fallback.io_stats,
    }
}

pub fn load_generated_default_lib_inputs(
    no_lib: bool,
    lib_entries: Option<&[String]>,
) -> Vec<SourceFileInput> {
    load_generated_default_lib_resolution(no_lib, lib_entries.unwrap_or_default(), "es2024").inputs
}

fn load_generated_default_lib_resolution(
    no_lib: bool,
    lib_entries: &[String],
    target_basename: &str,
) -> super::physical::PhysicalLibResolution {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let generated_dir = manifest_dir.join("generated-libs");
    let seed = default_full_lib_seed_for_target(target_basename);
    resolve_default_libs_from_lib_dir(
        generated_dir,
        no_lib,
        lib_entries,
        &seed,
        DefaultLibIoStats::default(),
    )
}
