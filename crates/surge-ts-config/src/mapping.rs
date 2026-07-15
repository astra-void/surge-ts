//! Shared `compilerOptions.paths` pattern selection.
//!
//! Mirrors tsc's `matchPatternOrExact` + `tryLoadModuleUsingPaths`: an exact
//! (starless) key wins outright; otherwise the single-`*` pattern with the
//! longest literal prefix wins (first-in-config order breaks prefix-length
//! ties). JSON insertion order never decides between patterns of different
//! specificity. Substitutions of the winning pattern are returned in author
//! order with the captured `*` text spliced in.
//!
//! Both the import-graph expander and the path-mapping pass consume this, so
//! graph expansion and checker binding select the same mapping.

use crate::model::PathMapping;

/// Split a pattern with exactly one `*` into `(prefix, suffix)`. Keys with
/// zero or more than one `*` yield `None` (tsc rejects multi-`*` patterns).
fn single_star_parts(pattern: &str) -> Option<(&str, &str)> {
    let first = pattern.find('*')?;
    if pattern[first + 1..].contains('*') {
        return None;
    }
    Some((&pattern[..first], &pattern[first + 1..]))
}

/// Select the winning `paths` mapping for `specifier` and return its
/// substitution targets in author order, `*` already replaced with the
/// captured text. `None` when no pattern matches.
pub fn select_path_mapping_targets(
    specifier: &str,
    mappings: &[PathMapping],
) -> Option<Vec<String>> {
    // Exact starless keys win over every wildcard pattern.
    for mapping in mappings {
        if !mapping.pattern.contains('*') && mapping.pattern == specifier {
            return Some(mapping.substitutions.clone());
        }
    }

    let mut best: Option<(usize, &PathMapping, &str)> = None;
    for mapping in mappings {
        let Some((prefix, suffix)) = single_star_parts(&mapping.pattern) else {
            continue;
        };
        if specifier.len() < prefix.len() + suffix.len()
            || !specifier.starts_with(prefix)
            || !specifier.ends_with(suffix)
        {
            continue;
        }
        let captured = &specifier[prefix.len()..specifier.len() - suffix.len()];
        let more_specific = match best {
            Some((best_prefix_len, _, _)) => prefix.len() > best_prefix_len,
            None => true,
        };
        if more_specific {
            best = Some((prefix.len(), mapping, captured));
        }
    }

    let (_, mapping, captured) = best?;
    Some(
        mapping
            .substitutions
            .iter()
            .filter(|substitution| substitution.matches('*').count() <= 1)
            .map(|substitution| substitution.replace('*', captured))
            .collect(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mapping(pattern: &str, substitutions: &[&str]) -> PathMapping {
        PathMapping {
            pattern: pattern.to_string(),
            substitutions: substitutions.iter().map(|s| s.to_string()).collect(),
        }
    }

    #[test]
    fn longest_literal_prefix_wins_regardless_of_config_order() {
        let mappings = [
            mapping("@/*", &["fallback/*"]),
            mapping("@/core/*", &["core/*"]),
        ];
        assert_eq!(
            select_path_mapping_targets("@/core/value", &mappings),
            Some(vec!["core/value".to_string()])
        );

        let reversed = [
            mapping("@/core/*", &["core/*"]),
            mapping("@/*", &["fallback/*"]),
        ];
        assert_eq!(
            select_path_mapping_targets("@/core/value", &reversed),
            Some(vec!["core/value".to_string()])
        );
    }

    #[test]
    fn exact_pattern_beats_wildcard() {
        let mappings = [
            mapping("lib/*", &["src/*"]),
            mapping("lib/special", &["src/the-special-one"]),
        ];
        assert_eq!(
            select_path_mapping_targets("lib/special", &mappings),
            Some(vec!["src/the-special-one".to_string()])
        );
    }

    #[test]
    fn substitution_order_is_preserved() {
        let mappings = [mapping("@/*", &["generated/*", "src/*"])];
        assert_eq!(
            select_path_mapping_targets("@/x", &mappings),
            Some(vec!["generated/x".to_string(), "src/x".to_string()])
        );
    }

    #[test]
    fn no_match_yields_none() {
        let mappings = [mapping("@/*", &["src/*"])];
        assert_eq!(select_path_mapping_targets("other/x", &mappings), None);
    }

    #[test]
    fn multi_star_patterns_and_targets_are_skipped() {
        let mappings = [
            mapping("@/*/*", &["src/*"]),
            mapping("@/*", &["bad/*/*", "src/*"]),
        ];
        assert_eq!(
            select_path_mapping_targets("@/a/b", &mappings),
            Some(vec!["src/a/b".to_string()])
        );
    }

    #[test]
    fn empty_capture_matches() {
        let mappings = [mapping("prefix*", &["src/*"])];
        assert_eq!(
            select_path_mapping_targets("prefix", &mappings),
            Some(vec!["src/".to_string()])
        );
    }
}
