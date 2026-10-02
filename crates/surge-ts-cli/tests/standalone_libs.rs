use std::path::Path;

/// Diagnostics that point into a bundled declaration must name the virtual
/// directory, not a path that looks like it exists on disk.
#[test]
fn bundled_declarations_use_a_virtual_path() {
    assert_eq!(surge_ts_checker::lowlevel::EMBEDDED_LIB_DIR, "<surge-lib>");

    let inputs = surge_ts_checker::lowlevel::load_generated_default_lib_inputs(false, None);
    assert!(!inputs.is_empty(), "bundled snapshot loaded no files");
    for input in &inputs {
        assert!(
            input.file_name.starts_with("<surge-lib>/lib.")
                && input.file_name.ends_with(".d.ts"),
            "unexpected bundled file identity: {}",
            input.file_name
        );
        assert!(
            !Path::new(&input.file_name).exists(),
            "bundled identity collides with a real file: {}",
            input.file_name
        );
    }
}
