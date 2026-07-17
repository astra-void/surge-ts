//! Pins cross-file dependency name resolution: a dependency .d.ts resolves
//! type names against its own lexical environment even when the consuming
//! module declares the same name with an incompatible shape. Mirrors the
//! checked-in fixture tests/compat-projects/complexity-dependency-lexical-scope.

use surge_ts_checker::{CheckerOptions, SourceFileInput, check_program_with_options};
use surge_ts_diagnostics::Diagnostic;

const DEP_DECLARATION: &str = "interface Payload {\n\
     \x20 kind: \"dep\";\n\
     \x20 size: number;\n\
     }\n\
     \n\
     export declare function getPayload(): Payload;\n";

fn codes(diagnostics: &[Diagnostic]) -> Vec<String> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.to_string())
        .collect()
}

fn check_with_dep(consumer_source: &str) -> Vec<Diagnostic> {
    let mut options = CheckerOptions::default();
    options
        .resolved_modules
        .insert("dep".to_string(), "node_modules/dep/index.d.ts".to_string());
    check_program_with_options(
        vec![
            SourceFileInput {
                file_name: "node_modules/dep/index.d.ts".to_string(),
                source_text: DEP_DECLARATION.to_string(),
            },
            SourceFileInput {
                file_name: "src/index.ts".to_string(),
                source_text: consumer_source.to_string(),
            },
        ],
        options,
    )
}

#[test]
fn dependency_dts_type_names_resolve_in_dependency_lexical_scope() {
    // The consumer's conflicting `Payload` must not leak into the dependency:
    // `fromDep.size` is only a `number` if dep's own `Payload` was used.
    let diagnostics = check_with_dep(
        "import { getPayload } from \"dep\";\n\
         interface Payload { kind: \"local\"; size: string; }\n\
         const fromDep = getPayload();\n\
         const size: number = fromDep.size;\n\
         const local: Payload = { kind: \"local\", size: \"here\" };\n\
         export { size, local };\n",
    );
    assert!(diagnostics.is_empty(), "{:?}", codes(&diagnostics));
}

#[test]
fn dependency_dts_collision_assignment_is_reported() {
    let diagnostics = check_with_dep(
        "import { getPayload } from \"dep\";\n\
         interface Payload { kind: \"local\"; size: string; }\n\
         const collided: Payload = getPayload();\n\
         export { collided };\n",
    );
    assert_eq!(codes(&diagnostics), vec!["TS2322".to_string()]);
}
