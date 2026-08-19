//! Shared classification of module specifiers.
//!
//! `"."` and `".."` are relative specifiers even though they carry no `/`
//! separator; classifying them as bare package names routes them through
//! package resolution, whose results are cached per specifier text rather than
//! per importer.

pub(crate) fn is_relative_specifier(specifier: &str) -> bool {
    specifier == "."
        || specifier == ".."
        || specifier.starts_with("./")
        || specifier.starts_with("../")
        || specifier.starts_with(".\\")
        || specifier.starts_with("..\\")
}

pub(crate) fn is_external_specifier(specifier: &str) -> bool {
    !is_relative_specifier(specifier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bare_dot_forms_are_relative_not_package_names() {
        assert!(is_relative_specifier("."));
        assert!(is_relative_specifier(".."));
        assert!(!is_external_specifier("."));
        assert!(!is_external_specifier(".."));
    }

    #[test]
    fn prefixed_relative_forms_stay_relative() {
        for specifier in ["./x", "../x", ".\\x", "..\\x"] {
            assert!(is_relative_specifier(specifier), "{specifier}");
        }
    }

    #[test]
    fn dot_leading_package_names_stay_external() {
        for specifier in [".pnpm", "..weird", "pkg", "@scope/pkg", "pkg/sub"] {
            assert!(is_external_specifier(specifier), "{specifier}");
        }
    }
}
