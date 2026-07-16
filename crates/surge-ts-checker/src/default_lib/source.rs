use std::cell::RefCell;

use surge_ts_types::fx::FxHashMap;

thread_local! {
    // Both name predicates are pure functions of the file name, and they sit
    // on per-type-resolution paths (`is_library_scoped_file`, interface
    // resolution), where the substring/suffix scans showed up in CPU profiles.
    // The memo is keyed by the full name; entries can never go stale.
    static DEFAULT_LIB_NAME_FLAGS: RefCell<FxHashMap<String, (bool, bool)>> =
        RefCell::new(FxHashMap::default());
}

/// `(is_generated_default_lib, is_physical_default_lib)` for `file_name`,
/// memoized per thread.
pub(crate) fn default_lib_name_flags(file_name: &str) -> (bool, bool) {
    DEFAULT_LIB_NAME_FLAGS.with(|cache| {
        if let Some(&flags) = cache.borrow().get(file_name) {
            return flags;
        }
        let flags = (
            is_generated_default_lib_file_name_uncached(file_name),
            crate::default_lib::physical::is_physical_default_lib_file_name_uncached(file_name),
        );
        cache.borrow_mut().insert(file_name.to_string(), flags);
        flags
    })
}

pub(crate) fn is_generated_default_lib_file_name(file_name: &str) -> bool {
    default_lib_name_flags(file_name).0
}

fn is_generated_default_lib_file_name_uncached(file_name: &str) -> bool {
    fn ends_with_ignore_case(haystack: &[u8], suffix: &[u8]) -> bool {
        haystack.len() >= suffix.len()
            && haystack[haystack.len() - suffix.len()..].eq_ignore_ascii_case(suffix)
    }
    let bytes = file_name.as_bytes();
    const DIR: &[u8] = b"/generated-libs/";
    bytes.len() >= DIR.len()
        && bytes
            .windows(DIR.len())
            .any(|w| w.eq_ignore_ascii_case(DIR))
        || ends_with_ignore_case(bytes, b".generated.d.ts")
        || ends_with_ignore_case(bytes, b".generated.d.mts")
        || ends_with_ignore_case(bytes, b".generated.d.cts")
}
