//! Centralized relative-path candidate generation (extension substitution).
//!
//! Every resolver layer — import-graph expansion, `paths` mapping, and checker
//! module binding — derives its filesystem/loaded-file candidates from these
//! functions so the extension-substitution matrix cannot drift between call
//! sites. The matrix mirrors tsc's `tryAddingExtensions`:
//!
//! * extensionless → `.ts`, `.tsx`, `.d.ts` (plus `index.*` of the same set)
//! * `.js` / `.jsx` → `.ts`, `.tsx`, `.d.ts`
//! * `.mjs` → `.mts`, `.d.mts`
//! * `.cjs` → `.cts`, `.d.cts`
//!
//! Explicit `.js`/`.jsx`/`.mjs`/`.cjs` specifiers are file-shaped runtime
//! paths: substitution replaces the extension in place and never turns the
//! path into a directory-index lookup. Extensionless specifiers never probe
//! the `.mts`/`.cts` flavors — tsc reaches those only through explicit
//! `.mjs`/`.cjs` specifiers.

/// How a relative specifier's final segment shapes candidate generation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RelativeSpecifierShape {
    /// `./x.ts` — resolves to exactly that file.
    ExplicitTs,
    /// `./x.js` or `./x.jsx` — runtime path; substitutes `.ts`/`.tsx`/`.d.ts`.
    ExplicitJs,
    /// `./x.mjs` — substitutes `.mts`/`.d.mts`.
    ExplicitMjs,
    /// `./x.cjs` — substitutes `.cts`/`.d.cts`.
    ExplicitCjs,
    /// `./x` (or `.` / `..`) — implicit extensions plus directory index.
    Extensionless,
    /// Explicit `.tsx`/`.mts`/`.cts`/`.d.*`/`.json` specifiers (pinned
    /// unsupported: `allowImportingTsExtensions` / `resolveJsonModule`).
    Unsupported,
}

pub fn classify_relative_specifier(specifier: &str) -> RelativeSpecifierShape {
    let last_segment = specifier.rsplit('/').next().unwrap_or(specifier);

    if last_segment == "." || last_segment == ".." {
        return RelativeSpecifierShape::Extensionless;
    }

    if last_segment.ends_with(".tsx")
        || last_segment.ends_with(".mts")
        || last_segment.ends_with(".cts")
        || last_segment.ends_with(".d.ts")
        || last_segment.ends_with(".d.mts")
        || last_segment.ends_with(".d.cts")
        || last_segment.ends_with(".json")
    {
        return RelativeSpecifierShape::Unsupported;
    }

    if last_segment.ends_with(".ts") {
        return RelativeSpecifierShape::ExplicitTs;
    }

    if last_segment.ends_with(".js") || last_segment.ends_with(".jsx") {
        return RelativeSpecifierShape::ExplicitJs;
    }

    if last_segment.ends_with(".mjs") {
        return RelativeSpecifierShape::ExplicitMjs;
    }

    if last_segment.ends_with(".cjs") {
        return RelativeSpecifierShape::ExplicitCjs;
    }

    RelativeSpecifierShape::Extensionless
}

/// Candidate paths for a relative import whose importer-joined, normalized
/// path is `joined` and whose original specifier is `specifier`. Returns
/// `None` for unsupported specifier shapes. Candidates are ordered by tsc
/// probe priority; the exact `joined` path leads each substitution list so a
/// literally-matching loaded file still wins.
pub fn relative_import_candidates(joined: &str, specifier: &str) -> Option<Vec<String>> {
    match classify_relative_specifier(specifier) {
        RelativeSpecifierShape::ExplicitTs => Some(vec![joined.to_string()]),
        RelativeSpecifierShape::ExplicitJs => {
            let stem = strip_extension(joined);
            Some(with_exact(joined, substitution_candidates_js(&stem)))
        }
        RelativeSpecifierShape::ExplicitMjs => {
            let stem = strip_extension(joined);
            Some(with_exact(joined, substitution_candidates_mjs(&stem)))
        }
        RelativeSpecifierShape::ExplicitCjs => {
            let stem = strip_extension(joined);
            Some(with_exact(joined, substitution_candidates_cjs(&stem)))
        }
        RelativeSpecifierShape::Extensionless => Some(extensionless_candidates(joined)),
        RelativeSpecifierShape::Unsupported => None,
    }
}

/// Candidate paths for a `paths`-mapping substitution target (already joined
/// against its base). Targets are file paths, not import specifiers, so a
/// recognized extension is substituted in place and everything else gets the
/// extensionless treatment.
pub fn mapped_target_candidates(target: &str) -> Vec<String> {
    let lower = target.to_ascii_lowercase();

    if lower.ends_with(".d.ts") || lower.ends_with(".d.mts") || lower.ends_with(".d.cts") {
        return vec![target.to_string()];
    }
    if lower.ends_with(".ts") || lower.ends_with(".tsx") {
        return vec![target.to_string()];
    }
    if lower.ends_with(".js") || lower.ends_with(".jsx") {
        return substitution_candidates_js(&strip_extension(target));
    }
    if lower.ends_with(".mjs") {
        return substitution_candidates_mjs(&strip_extension(target));
    }
    if lower.ends_with(".cjs") {
        return substitution_candidates_cjs(&strip_extension(target));
    }

    extensionless_candidates(target)
}

/// Implicit-extension candidates for an extensionless path: the path itself
/// (a literally-matching loaded file wins), `.ts`/`.tsx`/`.d.ts`, then the
/// directory-index set.
pub fn extensionless_candidates(base: &str) -> Vec<String> {
    let mut candidates = vec![
        base.to_string(),
        format!("{base}.ts"),
        format!("{base}.tsx"),
        format!("{base}.d.ts"),
    ];
    candidates.extend(directory_index_candidates(base));
    candidates
}

/// `index.*` probes for a directory path. tsc tries `.ts`/`.tsx`/`.d.ts`
/// only; the `.mts`/`.cts` flavors are never reachable through an index
/// lookup.
pub fn directory_index_candidates(base: &str) -> Vec<String> {
    vec![
        format!("{base}/index.ts"),
        format!("{base}/index.tsx"),
        format!("{base}/index.d.ts"),
    ]
}

fn substitution_candidates_js(stem: &str) -> Vec<String> {
    vec![
        format!("{stem}.ts"),
        format!("{stem}.tsx"),
        format!("{stem}.d.ts"),
    ]
}

fn substitution_candidates_mjs(stem: &str) -> Vec<String> {
    vec![format!("{stem}.mts"), format!("{stem}.d.mts")]
}

fn substitution_candidates_cjs(stem: &str) -> Vec<String> {
    vec![format!("{stem}.cts"), format!("{stem}.d.cts")]
}

fn with_exact(exact: &str, mut substitutions: Vec<String>) -> Vec<String> {
    let mut candidates = Vec::with_capacity(substitutions.len() + 1);
    candidates.push(exact.to_string());
    candidates.append(&mut substitutions);
    candidates
}

pub fn strip_extension(path: &str) -> String {
    match path.rsplit_once('.') {
        Some((head, _)) => head.to_string(),
        None => path.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn explicit_js_never_produces_directory_index() {
        let candidates = relative_import_candidates("src/foo.js", "./foo.js").unwrap();
        assert_eq!(
            candidates,
            vec!["src/foo.js", "src/foo.ts", "src/foo.tsx", "src/foo.d.ts"]
        );
        assert!(!candidates.iter().any(|c| c.contains("/index.")));
    }

    #[test]
    fn explicit_mjs_substitutes_only_m_flavor() {
        let candidates = relative_import_candidates("src/foo.mjs", "./foo.mjs").unwrap();
        assert_eq!(
            candidates,
            vec!["src/foo.mjs", "src/foo.mts", "src/foo.d.mts"]
        );
    }

    #[test]
    fn explicit_cjs_substitutes_only_c_flavor() {
        let candidates = relative_import_candidates("src/foo.cjs", "./foo.cjs").unwrap();
        assert_eq!(
            candidates,
            vec!["src/foo.cjs", "src/foo.cts", "src/foo.d.cts"]
        );
    }

    #[test]
    fn jsx_substitutes_like_js() {
        let candidates = relative_import_candidates("src/foo.jsx", "./foo.jsx").unwrap();
        assert_eq!(
            candidates,
            vec!["src/foo.jsx", "src/foo.ts", "src/foo.tsx", "src/foo.d.ts"]
        );
    }

    #[test]
    fn extensionless_never_probes_m_c_flavors() {
        let candidates = relative_import_candidates("src/foo", "./foo").unwrap();
        assert_eq!(
            candidates,
            vec![
                "src/foo",
                "src/foo.ts",
                "src/foo.tsx",
                "src/foo.d.ts",
                "src/foo/index.ts",
                "src/foo/index.tsx",
                "src/foo/index.d.ts",
            ]
        );
    }

    #[test]
    fn explicit_ts_is_exact_only() {
        assert_eq!(
            relative_import_candidates("src/foo.ts", "./foo.ts").unwrap(),
            vec!["src/foo.ts"]
        );
    }

    #[test]
    fn unsupported_shapes_yield_none() {
        for specifier in [
            "./x.tsx",
            "./x.mts",
            "./x.cts",
            "./x.d.ts",
            "./x.d.mts",
            "./x.d.cts",
            "./x.json",
        ] {
            assert!(
                relative_import_candidates("src/x", specifier).is_none(),
                "{specifier}"
            );
        }
    }

    #[test]
    fn dotted_directory_segments_do_not_confuse_classification() {
        // `./v1.2/foo` — the dot lives in a directory segment, not an extension.
        assert_eq!(
            classify_relative_specifier("./v1.2/foo"),
            RelativeSpecifierShape::Extensionless
        );
        // `./foo.test.js` strips only the final extension.
        let candidates = relative_import_candidates("src/foo.test.js", "./foo.test.js").unwrap();
        assert!(candidates.contains(&"src/foo.test.ts".to_string()));
    }

    #[test]
    fn mapped_target_shapes() {
        assert_eq!(mapped_target_candidates("/r/src/x.ts"), vec!["/r/src/x.ts"]);
        assert_eq!(
            mapped_target_candidates("/r/src/x.js"),
            vec!["/r/src/x.ts", "/r/src/x.tsx", "/r/src/x.d.ts"]
        );
        assert_eq!(
            mapped_target_candidates("/r/src/x.mjs"),
            vec!["/r/src/x.mts", "/r/src/x.d.mts"]
        );
        assert_eq!(
            mapped_target_candidates("/r/src/x.cjs"),
            vec!["/r/src/x.cts", "/r/src/x.d.cts"]
        );
        let extensionless = mapped_target_candidates("/r/src/x");
        assert!(extensionless.contains(&"/r/src/x/index.d.ts".to_string()));
        assert!(
            !extensionless
                .iter()
                .any(|c| c.ends_with(".mts") || c.ends_with(".cts"))
        );
    }
}
