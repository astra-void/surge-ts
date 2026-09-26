//! `CompilerOptions` as TypeScript's API spells them, and the translation to
//! and from the spelling a `tsconfig.json` uses. Normalization itself is
//! `surge-ts-config`'s, shared with the CLI: this module only renames.

use std::path::Path;

use serde_json::{Map, Value};
use surge_ts_config::{ConfigDiagnostic, ConfigDiagnosticCode};

/// TypeScript's `CompilerOptions`: option names as keys, enum-valued options
/// as TypeScript's enum numbers (`target: ScriptTarget.ESNext` is `99`), `lib`
/// as lib file names (`"lib.es2022.d.ts"`).
///
/// Enum-valued options also accept their `tsconfig.json` spelling
/// (`"esnext"`). Options surge does not implement are reported as option
/// diagnostics, never silently accepted.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CompilerOptions(Map<String, Value>);

const SCRIPT_TARGETS: &[(i64, &str)] = &[
    (0, "es3"),
    (1, "es5"),
    (2, "es2015"),
    (3, "es2016"),
    (4, "es2017"),
    (5, "es2018"),
    (6, "es2019"),
    (7, "es2020"),
    (8, "es2021"),
    (9, "es2022"),
    (10, "es2023"),
    (11, "es2024"),
    (12, "es2025"),
    (99, "esnext"),
];

const MODULE_KINDS: &[(i64, &str)] = &[
    (0, "none"),
    (1, "commonjs"),
    (2, "amd"),
    (3, "umd"),
    (4, "system"),
    (5, "es2015"),
    (6, "es2020"),
    (7, "es2022"),
    (99, "esnext"),
    (100, "node16"),
    (101, "node18"),
    (102, "node20"),
    (199, "nodenext"),
    (200, "preserve"),
];

const MODULE_RESOLUTION_KINDS: &[(i64, &str)] =
    &[(1, "classic"), (2, "node10"), (3, "node16"), (99, "nodenext"), (100, "bundler")];

const MODULE_DETECTION_KINDS: &[(i64, &str)] = &[(1, "legacy"), (2, "auto"), (3, "force")];

const JSX_EMITS: &[(i64, &str)] =
    &[(1, "preserve"), (2, "react"), (3, "react-native"), (4, "react-jsx"), (5, "react-jsxdev")];

const NEW_LINE_KINDS: &[(i64, &str)] = &[(0, "crlf"), (1, "lf")];

fn enum_table(option: &str) -> Option<&'static [(i64, &'static str)]> {
    Some(match option {
        "target" => SCRIPT_TARGETS,
        "module" => MODULE_KINDS,
        "moduleResolution" => MODULE_RESOLUTION_KINDS,
        "moduleDetection" => MODULE_DETECTION_KINDS,
        "jsx" => JSX_EMITS,
        "newLine" => NEW_LINE_KINDS,
        _ => return None,
    })
}

/// Options TypeScript's API sets on a parsed config that are not
/// `compilerOptions` a user writes.
const INTERNAL_OPTIONS: &[&str] = &["configFilePath", "pathsBasePath", "configFile"];

impl CompilerOptions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn from_json(options: Map<String, Value>) -> Self {
        Self(options)
    }

    pub fn as_json(&self) -> &Map<String, Value> {
        &self.0
    }

    pub fn get(&self, name: &str) -> Option<&Value> {
        self.0.get(name)
    }

    pub fn set(&mut self, name: impl Into<String>, value: Value) {
        self.0.insert(name.into(), value);
    }

    pub(crate) fn config_file_path(&self) -> Option<&str> {
        self.0.get("configFilePath").and_then(Value::as_str)
    }

    fn flag(&self, name: &str) -> bool {
        self.0.get(name).and_then(Value::as_bool).unwrap_or(false)
    }

    /// tsc's `getEmitDeclarations`.
    pub(crate) fn emits_declarations(&self) -> bool {
        self.flag("declaration") || self.flag("composite")
    }

    /// The options as a `tsconfig.json` would write them, with the
    /// diagnostics for values that have no such spelling.
    pub(crate) fn to_tsconfig_json(&self, config_dir: &Path) -> (Map<String, Value>, Vec<ConfigDiagnostic>) {
        let mut diagnostics = Vec::new();
        let mut out = Map::new();
        for (name, value) in &self.0 {
            if INTERNAL_OPTIONS.contains(&name.as_str()) || value.is_null() {
                continue;
            }
            let converted = match (enum_table(name), value) {
                (Some(table), Value::Number(number)) => {
                    match table.iter().find(|(code, _)| Some(*code) == number.as_i64()) {
                        Some((_, spelling)) => Value::String(spelling.to_string()),
                        None => {
                            diagnostics.push(ConfigDiagnostic {
                                code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
                                message: format!("unsupported value {number} for compiler option `{name}`"),
                                file_name: config_dir.to_path_buf(),
                            });
                            continue;
                        }
                    }
                }
                _ if name == "lib" => match value.as_array() {
                    Some(entries) => Value::Array(
                        entries
                            .iter()
                            .map(|entry| match entry.as_str() {
                                Some(file) => Value::String(lib_name_of_file(file)),
                                None => entry.clone(),
                            })
                            .collect(),
                    ),
                    None => value.clone(),
                },
                _ => value.clone(),
            };
            out.insert(name.clone(), converted);
        }
        (out, diagnostics)
    }

    /// TypeScript's spelling of options a `tsconfig.json` wrote, as
    /// `parseJsonConfigFileContent` returns them.
    pub(crate) fn from_tsconfig_json(options: &Map<String, Value>, config_dir: &Path) -> Self {
        let mut out = Map::new();
        for (name, value) in options {
            let converted = match (enum_table(name), value) {
                (Some(table), Value::String(spelling)) => {
                    let lower = spelling.to_ascii_lowercase();
                    let lower = match (name.as_str(), lower.as_str()) {
                        ("target" | "module", "es6") => "es2015".to_string(),
                        ("moduleResolution", "node") => "node10".to_string(),
                        _ => lower,
                    };
                    match table.iter().find(|(_, candidate)| *candidate == lower) {
                        Some((code, _)) => Value::from(*code),
                        None => value.clone(),
                    }
                }
                _ if name == "lib" => match value.as_array() {
                    Some(entries) => Value::Array(
                        entries
                            .iter()
                            .map(|entry| match entry.as_str() {
                                Some(lib) => Value::String(format!("lib.{}.d.ts", lib.to_ascii_lowercase())),
                                None => entry.clone(),
                            })
                            .collect(),
                    ),
                    None => value.clone(),
                },
                _ if matches!(name.as_str(), "baseUrl" | "rootDir" | "outDir" | "declarationDir" | "tsBuildInfoFile") => {
                    match value.as_str() {
                        Some(path) => Value::String(absolute_path(config_dir, path)),
                        None => value.clone(),
                    }
                }
                _ if matches!(name.as_str(), "typeRoots" | "rootDirs") => match value.as_array() {
                    Some(entries) => Value::Array(
                        entries
                            .iter()
                            .map(|entry| match entry.as_str() {
                                Some(path) => Value::String(absolute_path(config_dir, path)),
                                None => entry.clone(),
                            })
                            .collect(),
                    ),
                    None => value.clone(),
                },
                _ => value.clone(),
            };
            out.insert(name.clone(), converted);
        }
        if out.contains_key("paths") && !out.contains_key("baseUrl") {
            out.insert("pathsBasePath".to_string(), Value::String(config_dir.to_string_lossy().replace('\\', "/")));
        }
        Self(out)
    }
}

/// `"lib.es2022.d.ts"` (TypeScript's parsed `lib` entry) as the `"es2022"` a
/// tsconfig writes.
fn lib_name_of_file(file: &str) -> String {
    let base = file.rsplit(['/', '\\']).next().unwrap_or(file);
    base.strip_prefix("lib.")
        .and_then(|rest| rest.strip_suffix(".d.ts"))
        .map(str::to_string)
        .unwrap_or_else(|| file.to_string())
}

fn absolute_path(config_dir: &Path, path: &str) -> String {
    surge_ts_config::normalize_path_buf(&surge_ts_config::resolve_path(config_dir, path))
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn enum_numbers_become_tsconfig_spellings() {
        let options = CompilerOptions::from_json(
            json!({ "strict": true, "target": 99, "module": 199, "moduleResolution": 100, "jsx": 4,
                    "lib": ["lib.es2022.d.ts", "lib.dom.iterable.d.ts"] })
            .as_object()
            .unwrap()
            .clone(),
        );
        let (json, diagnostics) = options.to_tsconfig_json(Path::new("/p"));
        assert!(diagnostics.is_empty());
        assert_eq!(json["target"], "esnext");
        assert_eq!(json["module"], "nodenext");
        assert_eq!(json["moduleResolution"], "bundler");
        assert_eq!(json["jsx"], "react-jsx");
        assert_eq!(json["lib"], json!(["es2022", "dom.iterable"]));
    }

    #[test]
    fn tsconfig_spellings_become_enum_numbers() {
        let raw = json!({ "target": "ES2022", "module": "NodeNext", "lib": ["ES2022", "DOM"], "baseUrl": "./src" });
        let options = CompilerOptions::from_tsconfig_json(raw.as_object().unwrap(), Path::new("/p"));
        assert_eq!(options.get("target"), Some(&json!(9)));
        assert_eq!(options.get("module"), Some(&json!(199)));
        assert_eq!(options.get("lib"), Some(&json!(["lib.es2022.d.ts", "lib.dom.d.ts"])));
        assert_eq!(options.get("baseUrl"), Some(&json!("/p/src")));
    }

    #[test]
    fn an_unknown_enum_number_is_a_diagnostic() {
        let options = CompilerOptions::from_json(json!({ "target": 42 }).as_object().unwrap().clone());
        let (json, diagnostics) = options.to_tsconfig_json(Path::new("/p"));
        assert!(!json.contains_key("target"));
        assert_eq!(diagnostics.len(), 1);
    }
}
