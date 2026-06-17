use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use super::source::DefaultLibSelection;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum DefaultLibKind {
    Core,
    Dom,
}

#[derive(Debug, Clone)]
pub(crate) struct DefaultLibSource {
    pub(crate) kind: DefaultLibKind,
    pub(crate) file_name: PathBuf,
    pub(crate) source_text: String,
    #[allow(dead_code)]
    pub(crate) declared_names: &'static [&'static str],
}

static DEFAULT_LIB_SOURCES: OnceLock<Vec<DefaultLibSource>> = OnceLock::new();

pub(crate) fn selected_default_lib_sources(
    selection: DefaultLibSelection,
) -> Vec<DefaultLibSource> {
    if !selection.includes_anything() {
        return Vec::new();
    }

    default_lib_sources()
        .iter()
        .filter(|source| match source.kind {
            DefaultLibKind::Core => selection.include_core,
            DefaultLibKind::Dom => selection.include_dom,
        })
        .cloned()
        .collect()
}

fn default_lib_sources() -> &'static [DefaultLibSource] {
    DEFAULT_LIB_SOURCES
        .get_or_init(load_default_lib_sources)
        .as_slice()
}

fn load_default_lib_sources() -> Vec<DefaultLibSource> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let generated_dir = manifest_dir.join("generated-libs");

    vec![
        DefaultLibSource {
            kind: DefaultLibKind::Core,
            file_name: generated_dir.join("lib.es.generated.d.ts"),
            source_text: read_generated_source(&generated_dir.join("lib.es.generated.d.ts")),
            declared_names: &[
                "Array",
                "ArrayConstructor",
                "ReadonlyArray",
                "Promise",
                "PromiseConstructor",
                "PromiseLike",
                "Map",
                "Uint8Array",
                "String",
                "Number",
                "Boolean",
                "Date",
                "Math",
                "JSON",
                "decodeURIComponent",
                "isNaN",
                "Partial",
                "Pick",
                "Parameters",
                "Record",
                "Omit",
                "ReturnType",
                "Exclude",
                "Extract",
                "NonNullable",
            ],
        },
        DefaultLibSource {
            kind: DefaultLibKind::Dom,
            file_name: generated_dir.join("lib.dom.generated.d.ts"),
            source_text: read_generated_source(&generated_dir.join("lib.dom.generated.d.ts")),
            declared_names: &[
                "TextEncoder",
                "AuthenticatorTransport",
                "crypto",
                "console",
                "globalThis",
                "fetch",
            ],
        },
    ]
}

fn read_generated_source(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| {
        panic!(
            "failed to read generated default lib source {}: {error}. Run `pnpm run lib:generate`.",
            path.display()
        )
    })
}
