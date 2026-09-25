//! What the unused-identifier check (in `surge_ts_tsc_syntax`, over tsc's own
//! parse and bind of the file) needs from the checker's options: the names a
//! JSX element resolves for its factory.

/// The names a file's JSX resolves without writing them (tsc's
/// `markJsxAliasReferenced`), for an element and for a fragment. An element
/// reads the factory's root — the `@jsx` pragma's, else `jsxFactory`'s, else
/// `reactNamespace`, else `React` — and a fragment the fragment factory's
/// (`@jsxFrag`, else `jsxFragmentFactory`, else that default namespace, never
/// the file's pragma) together with the file's factory. The automatic
/// runtime imports its factory from the runtime module instead.
pub(crate) fn jsx_factory_reads(
    uses: &surge_ts_syntax::JsxFactoryUses,
    options: &crate::CheckerOptions,
) -> (Vec<String>, Vec<String>) {
    let names = &options.jsx_factory_names;
    let runtime = uses.runtime_pragma.as_deref();
    let automatic = runtime != Some("classic")
        && (options.jsx_automatic_runtime
            || names.import_source.is_some()
            || uses.import_source_pragma.is_some()
            || runtime == Some("automatic"));
    if automatic {
        return (Vec::new(), Vec::new());
    }
    let default_namespace = names
        .factory
        .as_deref()
        .and_then(surge_ts_syntax::jsx_entity_root)
        .or_else(|| names.react_namespace.clone())
        .unwrap_or_else(|| "React".to_string());
    let factory = uses
        .factory_pragma
        .clone()
        .unwrap_or_else(|| default_namespace.clone());
    let fragment = uses
        .fragment_pragma
        .clone()
        .or_else(|| {
            names
                .fragment_factory
                .as_deref()
                .and_then(surge_ts_syntax::jsx_entity_root)
        })
        .unwrap_or(default_namespace);
    let mut fragment_reads = vec![factory.clone()];
    // `jsxFragmentFactory: "null"` names no binding.
    if fragment != "null" {
        fragment_reads.push(fragment);
    }
    (vec![factory], fragment_reads)
}
