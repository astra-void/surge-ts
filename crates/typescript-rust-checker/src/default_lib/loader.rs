use std::path::Path;

use crate::SourceFileInput;

use super::physical::{default_full_lib_seed_for_target, resolve_physical_default_libs};
use super::registry::selected_default_lib_sources;
use super::source::default_lib_selection_from_tsconfig;

/// Inputs for resolving a project's default libs.
///
/// Default-lib loading prefers the real physical TypeScript `lib*.d.ts` graph
/// shipped by the installed `typescript` package and only falls back to the
/// generated subset when that package cannot be located.
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

/// Result of default-lib resolution, including metadata used for diagnostics.
#[derive(Debug, Default)]
pub struct DefaultLibLoad {
    /// Source inputs in deterministic, dependency-first load order.
    pub inputs: Vec<SourceFileInput>,
    /// True when the real TypeScript package supplied the libs; false when the
    /// generated subset was used because the package was unavailable.
    pub used_physical: bool,
    /// `compilerOptions.lib` entries with no matching `lib*.d.ts` file.
    pub unknown_libs: Vec<String>,
}

/// Load the default libs for a project, preferring the real physical TypeScript
/// `lib*.d.ts` graph and falling back to the generated subset only when the
/// TypeScript package cannot be located. `noLib` is honored on both paths.
pub fn load_default_lib_inputs(request: DefaultLibRequest<'_>) -> DefaultLibLoad {
    let seed = default_full_lib_seed_for_target(request.target_basename);
    if let Some(resolution) =
        resolve_physical_default_libs(request.root_dir, request.no_lib, request.lib_entries, &seed)
    {
        return DefaultLibLoad {
            inputs: resolution.inputs,
            used_physical: true,
            unknown_libs: resolution.unknown_libs,
        };
    }

    DefaultLibLoad {
        inputs: load_generated_default_lib_inputs(request.no_lib, Some(request.lib_entries)),
        used_physical: false,
        unknown_libs: Vec::new(),
    }
}

/// Load the generated default-lib subset.
///
/// This is the fallback path when the physical TypeScript package is missing and
/// the source of default libs for single-file checks, which have no project root
/// from which to resolve `node_modules/typescript`.
pub fn load_generated_default_lib_inputs(
    no_lib: bool,
    lib_entries: Option<&[String]>,
) -> Vec<SourceFileInput> {
    let selection = default_lib_selection_from_tsconfig(no_lib, lib_entries);
    selected_default_lib_sources(selection)
        .into_iter()
        .map(|source| SourceFileInput {
            file_name: source.file_name.to_string_lossy().into_owned(),
            source_text: source.source_text,
        })
        .collect()
}
