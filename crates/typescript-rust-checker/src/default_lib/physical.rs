//! Physical TypeScript `lib*.d.ts` discovery and reference-graph loading.
//!
//! Unlike the generated default-lib subset (see `registry.rs`), this module
//! resolves the *real* declaration files shipped by the pinned local
//! `typescript` package and follows their `/// <reference lib="..." />` graph.
//! The resulting [`SourceFileInput`]s are parsed and lowered through the normal
//! ambient-global pipeline (routed via `FileKind::PhysicalDefaultLib`), so no
//! pre-baked snapshot is involved.
//!
//! Physical loading is opt-in (CLI `--physicalLibs`, a `.physicalLibs` marker
//! file beside the project, or the `TYPESCRIPT_RUST_PHYSICAL_LIBS` env var) and
//! requires the package to be installed. When the package cannot be found the
//! resolver returns `None` and callers fall back to the generated subset.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

use crate::SourceFileInput;

/// Marks a file path as a physical TypeScript default-lib declaration file.
///
/// Physical lib inputs always live under `.../node_modules/typescript/lib/` and
/// are named `lib.<name>.d.ts`. Classifying them by path keeps the routing
/// self-describing: when physical mode is off these files are simply never
/// injected, so this predicate cannot misfire on ordinary projects.
pub fn is_physical_default_lib_file_name(file_name: &str) -> bool {
    let lower = file_name.replace('\\', "/").to_ascii_lowercase();
    let Some(idx) = lower.rfind('/') else {
        return false;
    };
    let (dir, file) = lower.split_at(idx);
    dir.ends_with("/typescript/lib") && file.starts_with("/lib.") && file.ends_with(".d.ts")
}

/// Outcome of resolving the physical default libs for a project.
#[derive(Debug, Clone, Default)]
pub struct PhysicalLibResolution {
    /// Source inputs in deterministic, dependency-first load order.
    pub inputs: Vec<SourceFileInput>,
    /// Canonical paths actually loaded (for `--showConfig`/debug output).
    pub loaded_files: Vec<String>,
    /// Requested `compilerOptions.lib` entries that could not be mapped to a
    /// real `lib*.d.ts` file.
    pub unknown_libs: Vec<String>,
}

/// Resolve and load the physical default libs for a project rooted at
/// `root_dir`.
///
/// * `no_lib` short-circuits to an empty resolution (still `Some`, so callers
///   do not fall back to the generated subset).
/// * `lib_entries` mirrors `compilerOptions.lib`. When empty, `default_seed`
///   (typically the target's `.full` aggregate, e.g. `"es2024.full"`) is used,
///   matching how `tsc` derives the default lib from `target`.
///
/// Returns `None` only when the TypeScript package cannot be located, signalling
/// callers to fall back to the generated subset.
pub fn resolve_physical_default_libs(
    root_dir: &Path,
    no_lib: bool,
    lib_entries: &[String],
    default_seed: &str,
) -> Option<PhysicalLibResolution> {
    let lib_dir = find_typescript_lib_dir(root_dir)?;

    if no_lib {
        return Some(PhysicalLibResolution::default());
    }

    let mut seeds: Vec<String> = Vec::new();
    let mut unknown_libs: Vec<String> = Vec::new();

    if lib_entries.is_empty() {
        seeds.push(default_seed.to_string());
    } else {
        for entry in lib_entries {
            seeds.push(entry.to_string());
        }
    }

    let mut loader = ReferenceGraphLoader::new(lib_dir);
    for seed in &seeds {
        if !loader.enqueue_lib_name(seed) {
            unknown_libs.push(seed.clone());
        }
    }
    let (inputs, loaded_files) = loader.run();

    Some(PhysicalLibResolution {
        inputs,
        loaded_files,
        unknown_libs,
    })
}

/// Map the configured `target` to the `lib.<name>.full.d.ts` aggregate that
/// `tsc` uses as the implicit default lib when `compilerOptions.lib` is unset.
pub fn default_full_lib_seed_for_target(target: &str) -> String {
    // Normalize "ES2022", "es2022", "ESNext" -> "es2022"/"esnext".
    let normalized = target.trim().to_ascii_lowercase();
    let base = match normalized.as_str() {
        // Older targets share the es5/es6 base libs which have no `.full`
        // aggregate; fall through to es2015 behaviour is uncommon for noEmit
        // projects, so default to a broad recent aggregate.
        "es3" | "es5" => "es5",
        other => other,
    };
    if base == "es5" {
        // es5 has no `.full`; seed the es5 + dom set explicitly.
        return "es5".to_string();
    }
    format!("{base}.full")
}

/// Walk up from `root_dir` looking for `node_modules/typescript/lib`.
fn find_typescript_lib_dir(root_dir: &Path) -> Option<PathBuf> {
    let mut current: Option<&Path> = Some(root_dir);
    while let Some(dir) = current {
        let candidate = dir.join("node_modules").join("typescript").join("lib");
        if candidate.join("lib.es5.d.ts").is_file() {
            return Some(candidate);
        }
        current = dir.parent();
    }
    None
}

/// Loads a lib reference graph depth-first, deduping by canonical path while
/// preserving deterministic first-seen order.
struct ReferenceGraphLoader {
    lib_dir: PathBuf,
    /// Pending lib names to expand, in order.
    queue: Vec<String>,
    /// Canonical paths already loaded (dedupe).
    visited_paths: BTreeSet<PathBuf>,
    /// Normalized lib names already enqueued (dedupe + cycle guard).
    seen_names: BTreeSet<String>,
    inputs: Vec<SourceFileInput>,
    loaded_files: Vec<String>,
}

impl ReferenceGraphLoader {
    fn new(lib_dir: PathBuf) -> Self {
        Self {
            lib_dir,
            queue: Vec::new(),
            visited_paths: BTreeSet::new(),
            seen_names: BTreeSet::new(),
            inputs: Vec::new(),
            loaded_files: Vec::new(),
        }
    }

    /// Enqueue a lib name (e.g. `"es2022"`, `"dom.iterable"`). Returns `false`
    /// if the name does not map to an existing `lib*.d.ts` file.
    fn enqueue_lib_name(&mut self, name: &str) -> bool {
        let normalized = normalize_lib_name(name);
        if !self.lib_file_path(&normalized).is_file() {
            return false;
        }
        if self.seen_names.insert(normalized.clone()) {
            self.queue.push(normalized);
        }
        true
    }

    fn lib_file_path(&self, normalized_name: &str) -> PathBuf {
        self.lib_dir.join(format!("lib.{normalized_name}.d.ts"))
    }

    /// Process the queue depth-first: each file is loaded, then its referenced
    /// libs are expanded before continuing, yielding dependency-first order
    /// that mirrors how `tsc` materializes the lib graph.
    fn run(mut self) -> (Vec<SourceFileInput>, Vec<String>) {
        // Use an explicit work-list so references discovered while loading a
        // file are processed before the rest of the original queue (depth
        // first), matching `tsc`'s recursive include order.
        let initial: Vec<String> = std::mem::take(&mut self.queue);
        for name in initial {
            self.load_recursive(&name);
        }
        (self.inputs, self.loaded_files)
    }

    fn load_recursive(&mut self, normalized_name: &str) {
        let path = self.lib_file_path(normalized_name);
        let canonical = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());
        if !self.visited_paths.insert(canonical.clone()) {
            return;
        }

        let Ok(source_text) = fs::read_to_string(&path) else {
            return;
        };

        // Expand referenced libs first so dependencies are emitted before the
        // file that requires them.
        for referenced in scan_reference_libs(&source_text) {
            let referenced_normalized = normalize_lib_name(&referenced);
            if !self.lib_file_path(&referenced_normalized).is_file() {
                continue;
            }
            if self.seen_names.insert(referenced_normalized.clone()) {
                self.load_recursive(&referenced_normalized);
            } else {
                // Already enqueued/visited elsewhere; cycle-safe no-op.
            }
        }

        let file_name = canonical.to_string_lossy().into_owned();
        self.loaded_files.push(file_name.clone());
        self.inputs.push(SourceFileInput {
            file_name,
            source_text,
        });
    }
}

/// Normalize a lib name the way TypeScript does for known libs: trim, lowercase,
/// and accept either `lib.es2022.d.ts`, `es2022`, or `ES2022`.
fn normalize_lib_name(name: &str) -> String {
    let trimmed = name.trim().to_ascii_lowercase();
    let trimmed = trimmed
        .strip_prefix("lib.")
        .unwrap_or(&trimmed)
        .strip_suffix(".d.ts")
        .unwrap_or_else(|| trimmed.strip_prefix("lib.").unwrap_or(&trimmed));
    trimmed.to_string()
}

/// Narrow scanner for `/// <reference lib="..." />` directives.
///
/// Triple-slash directives must precede the first real statement, but TypeScript
/// lib files open with a multi-line `/*! ... */` license banner whose interior
/// lines do not start with `*`. The scanner therefore tracks block-comment
/// state explicitly, skips line/block comments, collects `<reference lib>`
/// directives, and stops at the first line of real code. `no-default-lib` is
/// ignored because the reference graph is already explicit.
fn scan_reference_libs(source_text: &str) -> Vec<String> {
    let mut libs = Vec::new();
    let mut in_block_comment = false;

    for line in source_text.lines() {
        let mut rest = line;

        loop {
            if in_block_comment {
                match rest.find("*/") {
                    Some(idx) => {
                        rest = &rest[idx + 2..];
                        in_block_comment = false;
                    }
                    None => break,
                }
            }

            let trimmed = rest.trim_start();
            if trimmed.is_empty() {
                break;
            }

            if let Some(after) = trimmed.strip_prefix("/*") {
                match after.find("*/") {
                    Some(idx) => {
                        rest = &after[idx + 2..];
                        continue;
                    }
                    None => {
                        in_block_comment = true;
                        break;
                    }
                }
            }

            if let Some(directive) = trimmed.strip_prefix("///") {
                let directive = directive.trim_start();
                if directive.starts_with("<reference") {
                    if let Some(lib) = extract_attribute(directive, "lib") {
                        libs.push(lib);
                    }
                }
                break;
            }

            if trimmed.starts_with("//") {
                break;
            }

            // First line of real declaration code: directives cannot follow, so
            // stop scanning the whole file.
            return libs;
        }
    }

    libs
}

/// Extract `attr="value"` (single or double quoted) from a triple-slash
/// directive body. The attribute name must sit on a token boundary so that
/// `lib=` does not spuriously match inside `no-default-lib="true"`.
fn extract_attribute(directive: &str, attr: &str) -> Option<String> {
    let key = format!("{attr}=");
    let bytes = directive.as_bytes();
    let mut search_from = 0usize;
    while let Some(rel) = directive[search_from..].find(&key) {
        let key_start = search_from + rel;
        let boundary_ok = key_start == 0
            || directive[..key_start]
                .chars()
                .next_back()
                .map(|c| !c.is_ascii_alphanumeric() && c != '-' && c != '_')
                .unwrap_or(true);
        let value_start = key_start + key.len();
        if boundary_ok {
            let quote = *bytes.get(value_start)?;
            if quote == b'"' || quote == b'\'' {
                let content_start = value_start + 1;
                if let Some(end_rel) = directive[content_start..].find(quote as char) {
                    return Some(directive[content_start..content_start + end_rel].to_string());
                }
            }
        }
        search_from = value_start;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_lib_names() {
        assert_eq!(normalize_lib_name("ES2022"), "es2022");
        assert_eq!(normalize_lib_name(" dom.iterable "), "dom.iterable");
        assert_eq!(normalize_lib_name("lib.es2015.core.d.ts"), "es2015.core");
        assert_eq!(normalize_lib_name("Dom"), "dom");
    }

    #[test]
    fn scans_reference_libs() {
        let src = "\
/// <reference no-default-lib=\"true\"/>\n\
/// <reference lib=\"es2021\" />\n\
/// <reference lib='es2022.array' />\n\
interface Foo {}\n\
/// <reference lib=\"ignored.after.code\" />\n";
        assert_eq!(scan_reference_libs(src), vec!["es2021", "es2022.array"]);
    }

    #[test]
    fn scans_reference_libs_after_license_banner() {
        // Mirrors the real lib.es2022.d.ts layout: a multi-line `/*! ... */`
        // banner whose interior lines do not start with `*`, followed by the
        // reference directives.
        let src = "\
/*! *****************************************************************************\n\
Copyright (c) Microsoft Corporation. All rights reserved.\n\
Licensed under the Apache License, Version 2.0 (the \"License\");\n\
***************************************************************************** */\n\
\n\
\n\
/// <reference lib=\"es2021\" />\n\
/// <reference lib=\"es2022.array\" />\n\
/// <reference lib=\"dom\" />\n";
        assert_eq!(
            scan_reference_libs(src),
            vec!["es2021", "es2022.array", "dom"]
        );
    }

    #[test]
    fn extracts_quoted_attribute() {
        assert_eq!(
            extract_attribute("<reference lib=\"es2022\" />", "lib").as_deref(),
            Some("es2022")
        );
        assert_eq!(
            extract_attribute("<reference lib='dom' />", "lib").as_deref(),
            Some("dom")
        );
        assert_eq!(extract_attribute("<reference path=\"x\" />", "lib"), None);
    }

    #[test]
    fn recognizes_physical_lib_paths() {
        assert!(is_physical_default_lib_file_name(
            "/proj/node_modules/typescript/lib/lib.es5.d.ts"
        ));
        assert!(is_physical_default_lib_file_name(
            "/proj/node_modules/typescript/lib/lib.dom.iterable.d.ts"
        ));
        assert!(!is_physical_default_lib_file_name(
            "/proj/node_modules/typescript/lib/typescript.d.ts"
        ));
        assert!(!is_physical_default_lib_file_name("/proj/src/lib.es5.d.ts"));
    }

    #[test]
    fn default_seed_uses_full_aggregate() {
        assert_eq!(default_full_lib_seed_for_target("ES2024"), "es2024.full");
        assert_eq!(default_full_lib_seed_for_target("esnext"), "esnext.full");
        assert_eq!(default_full_lib_seed_for_target("ES5"), "es5");
    }
}
