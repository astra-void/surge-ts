use surge_ts_checker::check_source;

#[test]
fn unmodelled_calls_still_check_callback_bodies() {
    for call in ["obj.method", "obj?.method"] {
        let source = format!(
            "declare const fnValue: unknown; declare const obj: {{ method: unknown }}; {call}(() => missingName);"
        );
        let diagnostics = check_source(&source, "test.ts");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic.code.to_string() == "TS2304"
                && diagnostic.message.contains("missingName")),
            "{call}: {diagnostics:?}"
        );
    }
}
