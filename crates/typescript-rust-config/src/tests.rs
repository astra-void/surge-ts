use super::*;
use std::{
    env, fs,
    path::{Path, PathBuf},
};

fn temp_dir(prefix: &str) -> PathBuf {
    let unique = format!(
        "{}-{}-{}",
        prefix,
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    );
    let path = env::temp_dir().join(unique);
    fs::create_dir_all(&path).unwrap();
    path
}

fn write_file(root: &Path, relative: &str, contents: &str) -> PathBuf {
    let path = root.join(relative);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(&path, contents).unwrap();
    path
}

fn load(project: impl AsRef<Path>) -> LoadedTsConfig {
    load_tsconfig(TsConfigLoadOptions {
        project: project.as_ref().to_path_buf(),
    })
}

fn diagnostic_codes(diagnostics: &[ConfigDiagnostic]) -> Vec<ConfigDiagnosticCode> {
    diagnostics.iter().map(|diag| diag.code).collect()
}

fn has_diagnostic(diagnostics: &[ConfigDiagnostic], code: ConfigDiagnosticCode) -> bool {
    diagnostics.iter().any(|diag| diag.code == code)
}

#[test]
fn parses_jsonc_with_comments_and_trailing_commas() {
    let root = temp_dir("jsonc");
    write_file(
        &root,
        "tsconfig.json",
        r#"
            {
              // comment
              "compilerOptions": {
                "strict": true,
                "noImplicitAny": true,
              },
              "include": ["src/**/*.ts",]
            }
            "#,
    );
    write_file(&root, "src/index.ts", "const ok: string = 'ok';");

    let loaded = load(root.join("tsconfig.json"));
    assert!(loaded.diagnostics.is_empty());
    assert_eq!(loaded.compiler_options.strict, true);
    assert_eq!(loaded.compiler_options.no_implicit_any, true);
    assert_eq!(loaded.files, vec![root.join("src/index.ts")]);
}

#[test]
fn strict_true_implies_no_implicit_any() {
    let root = temp_dir("strict");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": { "strict": true } }"#,
    );
    let loaded = load(root.join("tsconfig.json"));
    assert!(loaded.diagnostics.is_empty());
    assert!(loaded.compiler_options.strict);
    assert!(loaded.compiler_options.no_implicit_any);
}

#[test]
fn no_implicit_any_false_overrides_strict_true() {
    let root = temp_dir("strict-override");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": { "strict": true, "noImplicitAny": false } }"#,
    );
    let loaded = load(root.join("tsconfig.json"));
    assert!(loaded.diagnostics.is_empty());
    assert!(loaded.compiler_options.strict);
    assert!(!loaded.compiler_options.no_implicit_any);
}

#[test]
fn empty_config_uses_ts7_defaults() {
    let root = temp_dir("defaults");
    write_file(&root, "tsconfig.json", r#"{ "compilerOptions": {} }"#);

    let loaded = load(root.join("tsconfig.json"));
    assert!(loaded.diagnostics.is_empty());
    assert!(loaded.compiler_options.strict);
    assert!(loaded.compiler_options.no_implicit_any);
    assert_eq!(loaded.compiler_options.target, ScriptTarget::ES2024);
    assert_eq!(loaded.compiler_options.module, ModuleKind::Preserve);
    assert_eq!(
        loaded.compiler_options.module_resolution,
        ModuleResolutionKind::Bundler
    );
}

#[test]
fn relative_extends_merges_compiler_options() {
    let root = temp_dir("extends");
    write_file(
        &root,
        "tsconfig.base.json",
        r#"{ "compilerOptions": { "noImplicitAny": true, "module": "commonjs" } }"#,
    );
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "extends": "./tsconfig.base.json", "compilerOptions": { "strict": true } }"#,
    );

    let loaded = load(root.join("tsconfig.json"));
    assert!(loaded.diagnostics.is_empty());
    assert!(loaded.compiler_options.strict);
    assert!(loaded.compiler_options.no_implicit_any);
    assert_eq!(loaded.compiler_options.module, ModuleKind::CommonJS);
}

#[test]
fn package_extends_scoped_with_explicit_tsconfig_path_merges_compiler_options() {
    let root = temp_dir("package-extends-scoped-explicit");
    write_file(
        &root,
        "node_modules/@tsconfig/node24/tsconfig.json",
        r#"
            {
              "compilerOptions": {
                "strict": true,
                "target": "es2024",
                "module": "preserve",
                "moduleResolution": "bundler"
              }
            }
            "#,
    );
    write_file(
        &root,
        "tsconfig.json",
        r#"
            {
              "extends": "@tsconfig/node24/tsconfig.json",
              "compilerOptions": {
                "noImplicitAny": false
              }
            }
            "#,
    );

    let loaded = load(root.join("tsconfig.json"));
    assert!(loaded.diagnostics.is_empty());
    assert!(loaded.compiler_options.strict);
    assert!(!loaded.compiler_options.no_implicit_any);
    assert_eq!(loaded.compiler_options.target, ScriptTarget::ES2024);
    assert_eq!(loaded.compiler_options.module, ModuleKind::Preserve);
    assert_eq!(
        loaded.compiler_options.module_resolution,
        ModuleResolutionKind::Bundler
    );
}

#[test]
fn package_extends_scoped_default_tsconfig_resolves() {
    let root = temp_dir("package-extends-scoped-default");
    write_file(
        &root,
        "node_modules/@tsconfig/node24/tsconfig.json",
        r#"{ "compilerOptions": { "strict": true } }"#,
    );
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "extends": "@tsconfig/node24" }"#,
    );

    let loaded = load(root.join("tsconfig.json"));
    assert!(loaded.diagnostics.is_empty());
    assert!(loaded.compiler_options.strict);
}

#[test]
fn package_extends_unscoped_explicit_tsconfig_resolves() {
    let root = temp_dir("package-extends-unscoped-explicit");
    write_file(
        &root,
        "node_modules/my-config/tsconfig.json",
        r#"{ "compilerOptions": { "strict": true } }"#,
    );
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "extends": "my-config/tsconfig.json" }"#,
    );

    let loaded = load(root.join("tsconfig.json"));
    assert!(loaded.diagnostics.is_empty());
    assert!(loaded.compiler_options.strict);
}

#[test]
fn package_extends_searches_parent_node_modules() {
    let root = temp_dir("package-extends-parent-node-modules");
    write_file(
        &root,
        "node_modules/my-config/tsconfig.json",
        r#"{ "compilerOptions": { "strict": true } }"#,
    );
    write_file(
        &root,
        "packages/app/tsconfig.json",
        r#"{ "extends": "my-config" }"#,
    );

    let loaded = load(root.join("packages/app/tsconfig.json"));
    assert!(loaded.diagnostics.is_empty());
    assert!(loaded.compiler_options.strict);
}

#[test]
fn package_extends_missing_emits_diagnostic() {
    let root = temp_dir("package-extends-missing");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "extends": "@tsconfig/missing" }"#,
    );

    let loaded = load(root.join("tsconfig.json"));
    assert!(
        diagnostic_codes(&loaded.diagnostics).contains(&ConfigDiagnosticCode::ExtendsFileNotFound)
    );
}

#[test]
fn package_extends_cycle_emits_diagnostic() {
    let root = temp_dir("package-extends-cycle");
    write_file(&root, "tsconfig.json", r#"{ "extends": "pkg" }"#);
    write_file(
        &root,
        "node_modules/pkg/tsconfig.json",
        r#"{ "extends": "../../tsconfig.json" }"#,
    );

    let loaded = load(root.join("tsconfig.json"));
    assert!(diagnostic_codes(&loaded.diagnostics).contains(&ConfigDiagnosticCode::ExtendsCycle));
}

#[test]
fn extends_cycle_emits_diagnostic() {
    let root = temp_dir("cycle");
    write_file(
        &root,
        "a.json",
        r#"{ "extends": "./b.json", "compilerOptions": { "strict": true } }"#,
    );
    write_file(
        &root,
        "b.json",
        r#"{ "extends": "./a.json", "compilerOptions": { "noImplicitAny": true } }"#,
    );

    let loaded = load(root.join("a.json"));
    assert!(diagnostic_codes(&loaded.diagnostics).contains(&ConfigDiagnosticCode::ExtendsCycle));
}

#[test]
fn files_selects_exact_files() {
    let root = temp_dir("files");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "files": ["src/a.ts", "src/b.tsx"] }"#,
    );
    write_file(&root, "src/a.ts", "const a = 1;");
    write_file(&root, "src/b.tsx", "const b = 2;");
    write_file(&root, "src/c.js", "const c = 3;");

    let loaded = load(root.join("tsconfig.json"));
    assert_eq!(
        loaded.files,
        vec![root.join("src/a.ts"), root.join("src/b.tsx")]
    );
}

#[test]
fn include_selects_matching_ts_files() {
    let root = temp_dir("include");
    write_file(&root, "tsconfig.json", r#"{ "include": ["src/**/*.ts"] }"#);
    write_file(&root, "src/a.ts", "const a = 1;");
    write_file(&root, "src/b.tsx", "const b = 2;");
    write_file(&root, "src/c.js", "const c = 3;");

    let loaded = load(root.join("tsconfig.json"));
    assert_eq!(loaded.files, vec![root.join("src/a.ts")]);
}

#[test]
fn exclude_removes_matching_files() {
    let root = temp_dir("exclude");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "include": ["src/**/*.ts"], "exclude": ["src/skip/**"] }"#,
    );
    write_file(&root, "src/a.ts", "const a = 1;");
    write_file(&root, "src/skip/b.ts", "const b = 2;");

    let loaded = load(root.join("tsconfig.json"));
    assert_eq!(loaded.files, vec![root.join("src/a.ts")]);
}

#[test]
fn default_excludes_node_modules() {
    let root = temp_dir("default-exclude");
    write_file(&root, "tsconfig.json", r#"{ "include": ["**/*"] }"#);
    write_file(&root, "src/a.ts", "const a = 1;");
    write_file(&root, "node_modules/pkg/index.ts", "const b = 2;");

    let loaded = load(root.join("tsconfig.json"));
    assert_eq!(loaded.files, vec![root.join("src/a.ts")]);
}

#[test]
fn invalid_target_es5_emits_legacy_diagnostic_and_falls_back_to_es2015() {
    let root = temp_dir("target");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": { "target": "es5" } }"#,
    );

    let loaded = load(root.join("tsconfig.json"));
    assert_eq!(loaded.compiler_options.target, ScriptTarget::ES2015);
    assert!(has_diagnostic(
        &loaded.diagnostics,
        ConfigDiagnosticCode::UnsupportedLegacyCompilerOptionValue
    ));
}

#[test]
fn module_resolution_node_is_legacy_and_falls_back_to_bundler() {
    let root = temp_dir("module-resolution-node");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": { "moduleResolution": "node" } }"#,
    );

    let loaded = load(root.join("tsconfig.json"));
    assert_eq!(
        loaded.compiler_options.module_resolution,
        ModuleResolutionKind::Bundler
    );
    assert!(has_diagnostic(
        &loaded.diagnostics,
        ConfigDiagnosticCode::UnsupportedLegacyCompilerOptionValue
    ));
}

#[test]
fn module_resolution_node10_is_legacy_and_falls_back_to_bundler() {
    let root = temp_dir("module-resolution-node10");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": { "moduleResolution": "node10" } }"#,
    );

    let loaded = load(root.join("tsconfig.json"));
    assert_eq!(
        loaded.compiler_options.module_resolution,
        ModuleResolutionKind::Bundler
    );
    assert!(has_diagnostic(
        &loaded.diagnostics,
        ConfigDiagnosticCode::UnsupportedLegacyCompilerOptionValue
    ));
}

#[test]
fn module_resolution_classic_is_legacy_and_falls_back_to_bundler() {
    let root = temp_dir("module-resolution-classic");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": { "moduleResolution": "classic" } }"#,
    );

    let loaded = load(root.join("tsconfig.json"));
    assert_eq!(
        loaded.compiler_options.module_resolution,
        ModuleResolutionKind::Bundler
    );
    assert!(has_diagnostic(
        &loaded.diagnostics,
        ConfigDiagnosticCode::UnsupportedLegacyCompilerOptionValue
    ));
}

#[test]
fn module_resolution_bundler_is_valid() {
    let root = temp_dir("module-resolution-bundler");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": { "moduleResolution": "bundler" } }"#,
    );

    let loaded = load(root.join("tsconfig.json"));
    assert!(loaded.diagnostics.is_empty());
    assert_eq!(
        loaded.compiler_options.module_resolution,
        ModuleResolutionKind::Bundler
    );
}

#[test]
fn module_resolution_nodenext_is_valid() {
    let root = temp_dir("module-resolution-nodenext");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": { "moduleResolution": "nodenext" } }"#,
    );

    let loaded = load(root.join("tsconfig.json"));
    assert!(loaded.diagnostics.is_empty());
    assert_eq!(
        loaded.compiler_options.module_resolution,
        ModuleResolutionKind::NodeNext
    );
}

#[test]
fn module_none_is_legacy_and_falls_back_to_preserve() {
    let root = temp_dir("module-none");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": { "module": "none" } }"#,
    );

    let loaded = load(root.join("tsconfig.json"));
    assert_eq!(loaded.compiler_options.module, ModuleKind::Preserve);
    assert!(has_diagnostic(
        &loaded.diagnostics,
        ConfigDiagnosticCode::UnsupportedLegacyCompilerOptionValue
    ));
}

#[test]
fn module_legacy_values_are_rejected() {
    for module in ["amd", "umd", "system", "systemjs"] {
        let root = temp_dir(&format!("module-legacy-{module}"));
        write_file(
            &root,
            "tsconfig.json",
            &format!(
                r#"{{
                      "compilerOptions": {{
                        "module": "{module}"
                      }}
                    }}"#
            ),
        );

        let loaded = load(root.join("tsconfig.json"));
        assert_eq!(loaded.compiler_options.module, ModuleKind::Preserve);
        assert!(has_diagnostic(
            &loaded.diagnostics,
            ConfigDiagnosticCode::UnsupportedLegacyCompilerOptionValue
        ));
    }
}

#[test]
fn target_es5_is_legacy_and_falls_back_to_es2015() {
    let root = temp_dir("target-es5");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": { "target": "es5" } }"#,
    );

    let loaded = load(root.join("tsconfig.json"));
    assert_eq!(loaded.compiler_options.target, ScriptTarget::ES2015);
    assert!(has_diagnostic(
        &loaded.diagnostics,
        ConfigDiagnosticCode::UnsupportedLegacyCompilerOptionValue
    ));
}

#[test]
fn base_url_is_legacy_and_emits_config_diagnostic() {
    let root = temp_dir("base-url");
    write_file(
        &root,
        "tsconfig.json",
        r#"{ "compilerOptions": { "baseUrl": "." } }"#,
    );

    let loaded = load(root.join("tsconfig.json"));
    assert!(loaded.compiler_options.paths.is_empty());
    assert!(has_diagnostic(
        &loaded.diagnostics,
        ConfigDiagnosticCode::UnsupportedLegacyCompilerOption
    ));
}

#[test]
fn paths_are_accepted_without_base_url() {
    let root = temp_dir("paths");
    write_file(
        &root,
        "tsconfig.json",
        r#"
            {
              "compilerOptions": {
                "paths": {
                  "@app/*": ["src/*"]
                }
              }
            }
            "#,
    );

    let loaded = load(root.join("tsconfig.json"));
    assert!(loaded.diagnostics.is_empty());
    assert_eq!(
        loaded.compiler_options.paths,
        vec![PathMapping {
            pattern: "@app/*".to_string(),
            substitutions: vec!["src/*".to_string()],
        }]
    );
}
