/// One vendored TypeScript standard-library declaration file, embedded in the
/// binary's read-only data.
pub struct EmbeddedLib {
    /// Normalized lib name (`es5`, `dom.iterable`, …) without the `lib.` prefix
    /// or the `.d.ts` suffix.
    pub name: &'static str,
    pub source: &'static str,
}

include!(concat!(env!("OUT_DIR"), "/embedded_libs.rs"));

/// Virtual directory the embedded libs are addressed under.
///
/// The angle brackets cannot appear in a path a project actually resolves, so
/// embedded identities never collide with user files, and they make it obvious
/// in a diagnostic that the declaration came from the bundled snapshot rather
/// than from somewhere on disk.
pub const EMBEDDED_LIB_DIR: &str = "<surge-lib>";

/// The TypeScript release the bundled snapshot was vendored from.
pub fn bundled_typescript_version() -> &'static str {
    BUNDLED_TYPESCRIPT_VERSION
}

pub fn embedded_lib_file_name(normalized_name: &str) -> String {
    format!("{EMBEDDED_LIB_DIR}/lib.{normalized_name}.d.ts")
}

/// The embedded source text for a normalized lib name, if the snapshot has one.
pub fn embedded_lib_source(normalized_name: &str) -> Option<&'static str> {
    // The build script emits the table sorted by name.
    EMBEDDED_LIBS
        .binary_search_by(|lib| lib.name.cmp(normalized_name))
        .ok()
        .map(|idx| EMBEDDED_LIBS[idx].source)
}

pub(crate) fn is_embedded_default_lib_file_name_uncached(file_name: &str) -> bool {
    file_name.starts_with(EMBEDDED_LIB_DIR)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embeds_the_core_libs() {
        for name in ["es5", "es2015.core", "es2022", "esnext", "dom", "dom.iterable"] {
            assert!(
                embedded_lib_source(name).is_some(),
                "missing embedded lib {name}"
            );
        }
    }

    #[test]
    fn embedded_sources_carry_the_upstream_copyright_header() {
        let es5 = embedded_lib_source("es5").expect("es5");
        assert!(es5.contains("Copyright (c) Microsoft Corporation"));
        assert!(es5.contains("Apache License"));
    }

    #[test]
    fn lookup_misses_are_not_fatal() {
        assert!(embedded_lib_source("es2015.core.d.ts").is_none());
        assert!(embedded_lib_source("definitely-not-a-lib").is_none());
    }

    #[test]
    fn table_is_sorted_so_binary_search_is_valid() {
        assert!(EMBEDDED_LIBS.windows(2).all(|w| w[0].name < w[1].name));
    }

    #[test]
    fn recognizes_embedded_file_names() {
        assert!(is_embedded_default_lib_file_name_uncached(
            "<surge-lib>/lib.es5.d.ts"
        ));
        assert!(!is_embedded_default_lib_file_name_uncached("/proj/src/a.ts"));
    }

    #[test]
    fn records_the_pinned_typescript_version() {
        let version = bundled_typescript_version();
        assert!(
            version.split('.').count() >= 2,
            "unexpected bundled version {version:?}"
        );
    }
}
