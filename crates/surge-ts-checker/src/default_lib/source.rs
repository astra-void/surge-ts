pub(crate) fn is_generated_default_lib_file_name(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.contains("/generated-libs/")
        || lower.ends_with(".generated.d.ts")
        || lower.ends_with(".generated.d.mts")
        || lower.ends_with(".generated.d.cts")
}
