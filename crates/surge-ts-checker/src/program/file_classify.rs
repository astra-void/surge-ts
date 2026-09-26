
// Instrumentation lives in `metrics`; re-export it so existing callers that
// reference `crate::program::record_*` / `ProgramTimings` keep resolving and so
// the bare `record_*` calls throughout this file stay in scope.

use crate::context::{CheckerOptions, FileKind};

/// tsc's module detection past an import or export: `moduleDetection: force`
/// makes every file a module, and under `auto` so does one whose format
/// forces it (`isFileForcedToBeModuleByFormat`: `.mts`/`.cts`/`.mjs`/`.cjs`,
/// or an ESM-format file) and, under `jsx: react-jsx`, one with a JSX tag.
pub(crate) fn file_is_forced_module(file_name: &str, has_jsx_tag: bool, options: &CheckerOptions) -> bool {
    let detection = &options.module_detection;
    if detection.legacy {
        return false;
    }
    if detection.force {
        return true;
    }
    let lower = file_name.to_ascii_lowercase();
    [".mts", ".cts", ".mjs", ".cjs"].iter().any(|extension| lower.ends_with(extension))
        || options.esm_module_files.contains(file_name)
        || (options.jsx_automatic_runtime && has_jsx_tag)
}

/// Whether the file NAME classifies as a trusted library/dependency
/// declaration file. Unlike `CheckerContext::is_library_scoped_file` this is
/// consumer-independent: synthetic environment-recovered contexts can carry a
/// `file_kinds` map without dependency entries, and a cache-key eligibility
/// gate must not vary with the consumer.
pub(crate) fn is_library_classified_file_name(file_name: &str) -> bool {
    matches!(
        classify_file_kind(file_name),
        FileKind::DependencyDeclaration
            | FileKind::GeneratedDeclaration
            | FileKind::PhysicalDefaultLib
    )
}

/// `is_library_classified_file_name` asks for this on per-type-resolution
/// paths, so the predicates below stay allocation-free: lowercasing the path
/// and normalizing its separators into fresh `String`s per call showed up in
/// CPU profiles. Memoizing the result per thread was tried and is a measured
/// loss — hashing a long path costs more than the scans do (tRPC +1.6% CPU).
pub(super) fn classify_file_kind(file_name: &str) -> FileKind {
    if is_declaration_file_name(file_name) {
        if is_generated_declaration_file_name(file_name) {
            return FileKind::GeneratedDeclaration;
        }

        // Physical default libs live under `.../typescript/lib/lib.*.d.ts`.
        // Classify them ahead of the generic dependency-declaration check so
        // they route through the real ambient-lowering pipeline rather than
        // being skipped like ordinary `node_modules` declarations.
        if crate::default_lib::is_physical_default_lib_file_name(file_name) {
            return FileKind::PhysicalDefaultLib;
        }

        // Package exports, `typesVersions`, type conditions, `@types`, and
        // re-export chains all converge on a physical declaration path before
        // this classification. Only installed-package declarations get the
        // aggressive declaration-backed policy; path-mapped declarations and
        // project-reference outputs outside dependency roots stay
        // `RootDeclaration` and retain user-authored checking semantics.
        if contains_path_segment(file_name, "node_modules") {
            return FileKind::DependencyDeclaration;
        }

        return FileKind::RootDeclaration;
    }

    FileKind::RootSource
}

/// `"/<segment>/"` with `\` accepted as a separator, without normalizing the
/// path into a fresh `String` first.
pub(super) fn contains_path_segment(file_name: &str, segment: &str) -> bool {
    let is_separator = |byte: u8| byte == b'/' || byte == b'\\';
    let bytes = file_name.as_bytes();
    let segment = segment.as_bytes();
    let window = segment.len() + 2;
    bytes.len() >= window
        && bytes.windows(window).any(|candidate| {
            is_separator(candidate[0])
                && is_separator(candidate[window - 1])
                && &candidate[1..window - 1] == segment
        })
}

pub(super) fn ends_with_ignore_ascii_case(file_name: &str, suffix: &str) -> bool {
    let (bytes, suffix) = (file_name.as_bytes(), suffix.as_bytes());
    bytes.len() >= suffix.len() && bytes[bytes.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
}

pub(super) fn contains_ignore_ascii_case(file_name: &str, needle: &str) -> bool {
    let (bytes, needle) = (file_name.as_bytes(), needle.as_bytes());
    bytes.len() >= needle.len()
        && bytes
            .windows(needle.len())
            .any(|candidate| candidate.eq_ignore_ascii_case(needle))
}

pub(super) fn is_declaration_file_name(file_name: &str) -> bool {
    surge_ts_syntax::is_declaration_file_name(file_name)
}

pub(super) fn is_generated_declaration_file_name(file_name: &str) -> bool {
    contains_ignore_ascii_case(file_name, "/.nuxt/")
        || contains_ignore_ascii_case(file_name, "/.generated/")
        || contains_ignore_ascii_case(file_name, "/generated-libs/")
        || contains_ignore_ascii_case(file_name, "/generated/")
        || ends_with_ignore_ascii_case(file_name, ".generated.d.ts")
        || ends_with_ignore_ascii_case(file_name, ".generated.d.mts")
        || ends_with_ignore_ascii_case(file_name, ".generated.d.cts")
}

#[cfg(test)]
mod file_kind_tests {
    use super::{FileKind, classify_file_kind};

    #[test]
    fn dependency_declaration_extensions_are_classified_lazily() {
        for file_name in [
            "/repo/node_modules/pkg/index.d.ts",
            "/repo/node_modules/pkg/index.d.mts",
            "/repo/node_modules/pkg/index.d.cts",
            r"C:\repo\node_modules\@types\pkg\index.d.ts",
            "/repo/node_modules/.pnpm/pkg@1/node_modules/pkg/index.d.ts",
        ] {
            assert_eq!(
                classify_file_kind(file_name),
                FileKind::DependencyDeclaration,
                "{file_name}"
            );
        }
    }

    #[test]
    fn user_and_generated_declarations_keep_distinct_policies() {
        assert_eq!(
            classify_file_kind("/repo/types/path-mapped.d.ts"),
            FileKind::RootDeclaration
        );
        assert_eq!(
            classify_file_kind("/repo/project-reference/dist/index.d.ts"),
            FileKind::RootDeclaration
        );
        assert_eq!(
            classify_file_kind("/repo/.generated/router.d.ts"),
            FileKind::GeneratedDeclaration
        );
        assert_eq!(
            classify_file_kind("/repo/node_modules/pkg/index.ts"),
            FileKind::RootSource
        );
    }
}
