use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::provider::LibSource;
use crate::SourceFileInput;

/// Filesystem I/O incurred while discovering and loading the physical lib graph.
///
/// Default-lib loading runs before `check_program` enables the program-wide
/// counters (which are also reset there), so these stats are returned by value
/// rather than recorded through the global counter table.
#[derive(Debug, Clone, Default)]
pub struct DefaultLibIoStats {
    pub read_io: Duration,
    pub files_read: u64,
    pub bytes_read: u64,
    pub existence_probes: u64,
    pub canonicalize_syscalls: u64,
}

/// Marks a file as a full-fidelity TypeScript default-lib declaration file.
///
/// Two identities qualify: the embedded snapshot under `<surge-lib>/`, and an
/// on-disk `.../typescript/lib/lib.<name>.d.ts` from an explicit override. Both
/// are real upstream lib text, so they take the same resolution path; the
/// separate `is_generated_default_lib_file_name` routing is the older degraded
/// subset and stays distinct. Neither shape can name a file in an ordinary
/// project, so this predicate cannot misfire on user sources.
pub fn is_physical_default_lib_file_name(file_name: &str) -> bool {
    crate::default_lib::source::default_lib_name_flags(file_name).1
}

pub(crate) fn is_physical_default_lib_file_name_uncached(file_name: &str) -> bool {
    // Allocation-free equivalent of lowercasing and normalizing `\` to `/`:
    // this predicate sits on per-type-resolution paths, so it must not build
    // a fresh `String` per call.
    fn norm(byte: u8) -> u8 {
        if byte == b'\\' {
            b'/'
        } else {
            byte.to_ascii_lowercase()
        }
    }
    fn ends_with_norm(haystack: &[u8], suffix: &[u8]) -> bool {
        haystack.len() >= suffix.len()
            && haystack[haystack.len() - suffix.len()..]
                .iter()
                .zip(suffix)
                .all(|(&h, &s)| norm(h) == s)
    }
    fn starts_with_norm(haystack: &[u8], prefix: &[u8]) -> bool {
        haystack.len() >= prefix.len()
            && haystack[..prefix.len()]
                .iter()
                .zip(prefix)
                .all(|(&h, &s)| norm(h) == s)
    }
    if crate::default_lib::embedded::is_embedded_default_lib_file_name_uncached(file_name) {
        return true;
    }
    let bytes = file_name.as_bytes();
    let Some(idx) = bytes.iter().rposition(|&b| b == b'/' || b == b'\\') else {
        return false;
    };
    let (dir, file) = bytes.split_at(idx);
    ends_with_norm(dir, b"/typescript/lib")
        && starts_with_norm(file, b"/lib.")
        && ends_with_norm(file, b".d.ts")
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
    /// Filesystem I/O incurred while discovering and loading the lib graph.
    pub io_stats: DefaultLibIoStats,
}

/// Locate an on-disk TypeScript `lib/` directory by walking up from `root_dir`.
///
/// Only the explicit override path uses this; the default source is the
/// embedded snapshot, so an installed `typescript` package never silently
/// changes which declarations a project is checked against.
pub fn find_typescript_lib_dir(root_dir: &Path) -> Option<PathBuf> {
    let mut io_stats = DefaultLibIoStats::default();
    find_typescript_lib_dir_from(root_dir, &mut io_stats)
}

/// Resolve and load the default libs for a project from `source`.
///
/// * `no_lib` short-circuits to an empty resolution.
/// * `lib_entries` mirrors `compilerOptions.lib`. When empty, `default_seed`
///   (typically the target's `.full` aggregate, e.g. `"es2024.full"`) is used,
///   matching how `tsc` derives the default lib from `target`.
/// * `referenced_libs` are the `/// <reference lib>` names of the program's
///   own files. tsc's file loader adds each one to the program on top of the
///   configured set; a name with no lib file is the directive's own error,
///   not the loader's, so it is skipped here.
pub(crate) fn resolve_default_libs_from_source(
    source: &dyn LibSource,
    no_lib: bool,
    lib_entries: &[String],
    default_seed: &str,
    referenced_libs: &[String],
    io_stats: DefaultLibIoStats,
) -> PhysicalLibResolution {
    if no_lib {
        return PhysicalLibResolution {
            io_stats,
            ..Default::default()
        };
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

    let mut loader = ReferenceGraphLoader::new(source, io_stats);
    for seed in &seeds {
        if !loader.enqueue_lib_name(seed) {
            unknown_libs.push(seed.clone());
        }
    }
    for referenced in referenced_libs {
        loader.enqueue_lib_name(referenced);
    }
    let (inputs, loaded_files, io_stats) = loader.run();

    PhysicalLibResolution {
        inputs,
        loaded_files,
        unknown_libs,
        io_stats,
    }
}

/// Map the configured `target` to the `lib.<name>.full.d.ts` aggregate that
/// `tsc` uses as the implicit default lib when `compilerOptions.lib` is unset.
pub fn default_full_lib_seed_for_target(target: &str) -> String {
    // Normalize "ES2022", "es2022", "ESNext" -> "es2022"/"esnext".
    let normalized = target.trim().to_ascii_lowercase();
    match normalized.as_str() {
        // Neither pre-ES2016 target has a `.full` aggregate: upstream names the
        // ES2015 one `lib.es6.d.ts`, and ES5's default set is `lib.es5.d.ts`
        // itself (which already pulls in DOM by reference).
        "es3" | "es5" => "es5".to_string(),
        "es6" | "es2015" => "es6".to_string(),
        other => format!("{other}.full"),
    }
}

fn find_typescript_lib_dir_from(
    start_dir: &Path,
    io_stats: &mut DefaultLibIoStats,
) -> Option<PathBuf> {
    let mut current: Option<&Path> = Some(start_dir);
    while let Some(dir) = current {
        let candidate = dir.join("node_modules").join("typescript").join("lib");
        io_stats.existence_probes += 1;
        if candidate.join("lib.es5.d.ts").is_file() {
            return Some(candidate);
        }
        current = dir.parent();
    }
    None
}

/// Loads a lib reference graph depth-first, deduping by source identity while
/// preserving deterministic first-seen order.
struct ReferenceGraphLoader<'a> {
    source: &'a dyn LibSource,
    /// Pending lib names to expand, in order.
    queue: Vec<String>,
    /// Source identities already loaded (dedupe).
    visited_files: BTreeSet<String>,
    /// Normalized lib names already enqueued (dedupe + cycle guard).
    seen_names: BTreeSet<String>,
    inputs: Vec<SourceFileInput>,
    loaded_files: Vec<String>,
    io_stats: DefaultLibIoStats,
}

impl<'a> ReferenceGraphLoader<'a> {
    fn new(source: &'a dyn LibSource, io_stats: DefaultLibIoStats) -> Self {
        Self {
            source,
            queue: Vec::new(),
            visited_files: BTreeSet::new(),
            seen_names: BTreeSet::new(),
            inputs: Vec::new(),
            loaded_files: Vec::new(),
            io_stats,
        }
    }

    /// Enqueue a lib name (e.g. `"es2022"`, `"dom.iterable"`). Returns `false`
    /// if the source has no such lib.
    fn enqueue_lib_name(&mut self, name: &str) -> bool {
        let normalized = normalize_lib_name(name);
        if !self.source.contains(&normalized, &mut self.io_stats) {
            return false;
        }
        if self.seen_names.insert(normalized.clone()) {
            self.queue.push(normalized);
        }
        true
    }

    /// Process the queue depth-first: each file is loaded, then its referenced
    /// libs are expanded before continuing, yielding dependency-first order
    /// that mirrors how `tsc` materializes the lib graph.
    fn run(mut self) -> (Vec<SourceFileInput>, Vec<String>, DefaultLibIoStats) {
        // Use an explicit work-list so references discovered while loading a
        // file are processed before the rest of the original queue (depth
        // first), matching `tsc`'s recursive include order.
        let initial: Vec<String> = std::mem::take(&mut self.queue);
        for name in initial {
            self.load_recursive(&name);
        }
        // tsc then orders the lib files by their position in its lib list
        // (`sortLibs`), which is the order their declarations merge in: a
        // `DateConstructor` signature from `es2015.core` follows `scripthost`'s.
        let mut ordered: Vec<(SourceFileInput, String)> =
            self.inputs.into_iter().zip(self.loaded_files).collect();
        ordered.sort_by_key(|(input, _)| default_lib_file_priority(&input.file_name));
        let (inputs, loaded_files) = ordered.into_iter().unzip();
        (inputs, loaded_files, self.io_stats)
    }

    fn load_recursive(&mut self, normalized_name: &str) {
        let Some((file_name, source_text)) =
            self.source.load(normalized_name, &mut self.io_stats)
        else {
            return;
        };
        if !self.visited_files.insert(file_name.clone()) {
            return;
        }

        // Expand referenced libs first so dependencies are emitted before the
        // file that requires them.
        for referenced in scan_reference_libs(&source_text) {
            let referenced_normalized = normalize_lib_name(&referenced);
            if !self
                .source
                .contains(&referenced_normalized, &mut self.io_stats)
            {
                continue;
            }
            if self.seen_names.insert(referenced_normalized.clone()) {
                self.load_recursive(&referenced_normalized);
            }
        }

        self.loaded_files.push(file_name.clone());
        self.inputs.push(SourceFileInput {
            file_name,
            source_text,
        });
    }
}

/// Normalize a lib name the way TypeScript does for known libs: trim, lowercase,
/// and accept either `lib.es2022.d.ts`, `es2022`, or `ES2022`.
/// tsc's lib list (`tsoptions.Libs`), in the order `sortLibs` ranks lib files by.
const LIB_ORDER: &[&str] = &[
    "es5",
    "es6",
    "es2015",
    "es7",
    "es2016",
    "es2017",
    "es2018",
    "es2019",
    "es2020",
    "es2021",
    "es2022",
    "es2023",
    "es2024",
    "es2025",
    "esnext",
    "dom",
    "dom.iterable",
    "dom.asynciterable",
    "webworker",
    "webworker.importscripts",
    "webworker.iterable",
    "webworker.asynciterable",
    "scripthost",
    "es2015.core",
    "es2015.collection",
    "es2015.generator",
    "es2015.iterable",
    "es2015.promise",
    "es2015.proxy",
    "es2015.reflect",
    "es2015.symbol",
    "es2015.symbol.wellknown",
    "es2016.array.include",
    "es2016.intl",
    "es2017.arraybuffer",
    "es2017.date",
    "es2017.object",
    "es2017.sharedmemory",
    "es2017.string",
    "es2017.intl",
    "es2017.typedarrays",
    "es2018.asyncgenerator",
    "es2018.asynciterable",
    "es2018.intl",
    "es2018.promise",
    "es2018.regexp",
    "es2019.array",
    "es2019.object",
    "es2019.string",
    "es2019.symbol",
    "es2019.intl",
    "es2020.bigint",
    "es2020.date",
    "es2020.promise",
    "es2020.sharedmemory",
    "es2020.string",
    "es2020.symbol.wellknown",
    "es2020.intl",
    "es2020.number",
    "es2021.promise",
    "es2021.string",
    "es2021.weakref",
    "es2021.intl",
    "es2022.array",
    "es2022.error",
    "es2022.intl",
    "es2022.object",
    "es2022.string",
    "es2022.regexp",
    "es2023.array",
    "es2023.collection",
    "es2023.intl",
    "es2024.arraybuffer",
    "es2024.collection",
    "es2024.object",
    "es2024.promise",
    "es2024.regexp",
    "es2024.sharedmemory",
    "es2024.string",
    "es2025.collection",
    "es2025.float16",
    "es2025.intl",
    "es2025.iterator",
    "es2025.promise",
    "es2025.regexp",
    "esnext.asynciterable",
    "esnext.symbol",
    "esnext.bigint",
    "esnext.weakref",
    "esnext.object",
    "esnext.regexp",
    "esnext.string",
    "esnext.float16",
    "esnext.iterator",
    "esnext.promise",
    "esnext.array",
    "esnext.collection",
    "esnext.date",
    "esnext.decorators",
    "esnext.disposable",
    "esnext.error",
    "esnext.intl",
    "esnext.sharedmemory",
    "esnext.temporal",
    "esnext.typedarrays",
    "decorators",
    "decorators.legacy",
];

/// tsc's `getDefaultLibFilePriority`: `lib.d.ts` first, then a lib's position in
/// [`LIB_ORDER`], and anything else after every listed lib.
fn default_lib_file_priority(file_name: &str) -> usize {
    let base_name = file_name.rsplit(['/', '\\']).next().unwrap_or(file_name);
    if base_name == "lib.d.ts" || base_name == "lib.es6.d.ts" {
        return 0;
    }
    let name = base_name
        .strip_prefix("lib.")
        .and_then(|name| name.strip_suffix(".d.ts"))
        .unwrap_or(base_name);
    LIB_ORDER
        .iter()
        .position(|lib| lib.eq_ignore_ascii_case(name))
        .map_or(LIB_ORDER.len() + 2, |index| index + 1)
}

fn normalize_lib_name(name: &str) -> String {
    let trimmed = name.trim().to_ascii_lowercase();
    let trimmed = trimmed
        .strip_prefix("lib.")
        .unwrap_or(&trimmed)
        .strip_suffix(".d.ts")
        .unwrap_or_else(|| trimmed.strip_prefix("lib.").unwrap_or(&trimmed));
    trimmed.to_string()
}

/// The `/// <reference lib="..." />` names a source file declares.
pub fn reference_lib_directives(source_text: &str) -> Vec<String> {
    scan_reference_libs(source_text)
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

    /// Every seed the target mapping can produce must exist in the bundled
    /// snapshot, or the target silently resolves to no standard library.
    #[test]
    fn every_target_seed_exists_in_the_bundled_snapshot() {
        let targets = [
            "ES5", "ES2015", "ES6", "ES2016", "ES2017", "ES2018", "ES2019", "ES2020", "ES2021",
            "ES2022", "ES2023", "ES2024", "ESNext",
        ];
        for target in targets {
            let seed = default_full_lib_seed_for_target(target);
            assert!(
                crate::default_lib::embedded::embedded_lib_source(&seed).is_some(),
                "target {target} seeds {seed:?}, which the bundled snapshot does not contain"
            );
        }
    }
}
