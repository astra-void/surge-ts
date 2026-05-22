use std::path::{Path, PathBuf};

use serde_json::{Map, Value};

use crate::diagnostics::{ConfigDiagnostic, ConfigDiagnosticCode};
use crate::model::{
    JsxMode, ModuleKind, ModuleResolutionKind, NormalizedCompilerOptions, PathMapping, ScriptTarget,
};
use crate::options::{
    TsConfigOptionDefinition, TsConfigOptionSupport, TsConfigOptionValueKind, find_tsconfig_option,
};
use crate::paths::resolve_path;

pub(crate) fn normalize_compiler_options(
    compiler_options: Option<&Map<String, Value>>,
    config_dir: &Path,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) -> NormalizedCompilerOptions {
    let mut normalized = NormalizedCompilerOptions::default();
    let Some(compiler_options) = compiler_options else {
        return normalized;
    };

    let mut explicit_no_implicit_any = None;

    for (key, value) in compiler_options {
        match key.as_str() {
            "strict" => {
                if let Some(strict) = parse_bool_option(key, value, config_dir, diagnostics) {
                    normalized.strict = strict;
                }
            }
            "noImplicitAny" => {
                explicit_no_implicit_any = parse_bool_option(key, value, config_dir, diagnostics);
                if let Some(no_implicit_any) = explicit_no_implicit_any {
                    normalized.no_implicit_any = no_implicit_any;
                }
            }
            "target" => {
                normalized.target = parse_target_option(value, config_dir, diagnostics);
            }
            "module" => {
                normalized.module = parse_module_option(value, config_dir, diagnostics);
            }
            "moduleResolution" => {
                normalized.module_resolution =
                    parse_module_resolution_option(value, config_dir, diagnostics);
            }
            "jsx" => {
                normalized.jsx = parse_jsx_option(value, config_dir, diagnostics);
            }
            "allowJs" => {
                normalized.allow_js = parse_bool_option(key, value, config_dir, diagnostics)
                    .unwrap_or(normalized.allow_js);
            }
            "checkJs" => {
                normalized.check_js = parse_bool_option(key, value, config_dir, diagnostics)
                    .unwrap_or(normalized.check_js);
            }
            "noEmit" => {
                normalized.no_emit = parse_bool_option(key, value, config_dir, diagnostics)
                    .unwrap_or(normalized.no_emit);
            }
            "skipLibCheck" => {
                normalized.skip_lib_check = parse_bool_option(key, value, config_dir, diagnostics)
                    .unwrap_or(normalized.skip_lib_check);
            }
            "noLib" => {
                normalized.no_lib = parse_bool_option(key, value, config_dir, diagnostics)
                    .unwrap_or(normalized.no_lib);
            }
            "paths" => {
                normalized.paths = parse_paths_option(value, diagnostics, config_dir);
            }
            "lib" => {
                normalized.lib = parse_string_list_option(value, diagnostics, config_dir);
            }
            "typeRoots" => {
                normalized.type_roots = parse_path_list_option(value, config_dir, diagnostics);
            }
            "types" => {
                normalized.types = parse_string_list_option(value, diagnostics, config_dir);
            }
            other => match find_tsconfig_option(other) {
                Some(definition) => match definition.support {
                    TsConfigOptionSupport::KnownNoop => {
                        validate_tsconfig_option_value(definition, value, config_dir, diagnostics);
                    }
                    TsConfigOptionSupport::UnsupportedLegacy => {
                        handle_legacy_tsconfig_option(definition, value, config_dir, diagnostics);
                    }
                    TsConfigOptionSupport::Supported => {
                        diagnostics.push(ConfigDiagnostic {
                            code: ConfigDiagnosticCode::UnknownCompilerOption,
                            message: format!("unknown compiler option `{other}`"),
                            file_name: config_dir.to_path_buf(),
                        });
                    }
                },
                None => {
                    diagnostics.push(ConfigDiagnostic {
                        code: ConfigDiagnosticCode::UnknownCompilerOption,
                        message: format!("unknown compiler option `{other}`"),
                        file_name: config_dir.to_path_buf(),
                    });
                }
            },
        }
    }

    normalized.no_implicit_any = explicit_no_implicit_any.unwrap_or(normalized.strict);

    normalized
}

fn parse_bool_option(
    key: &str,
    value: &Value,
    file_name: &Path,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) -> Option<bool> {
    match value.as_bool() {
        Some(bool) => Some(bool),
        None => {
            diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
                message: format!("`{key}` must be a boolean"),
                file_name: file_name.to_path_buf(),
            });
            None
        }
    }
}

fn parse_target_option(
    value: &Value,
    file_name: &Path,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) -> ScriptTarget {
    let Some(raw) = value.as_str() else {
        diagnostics.push(ConfigDiagnostic {
            code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
            message: "`target` must be a string".to_string(),
            file_name: file_name.to_path_buf(),
        });
        return ScriptTarget::ES2015;
    };

    match raw.to_ascii_lowercase().as_str() {
        "es2015" => ScriptTarget::ES2015,
        "es2016" => ScriptTarget::ES2016,
        "es2017" => ScriptTarget::ES2017,
        "es2018" => ScriptTarget::ES2018,
        "es2019" => ScriptTarget::ES2019,
        "es2020" => ScriptTarget::ES2020,
        "es2021" => ScriptTarget::ES2021,
        "es2022" => ScriptTarget::ES2022,
        "es2023" => ScriptTarget::ES2023,
        "es2024" => ScriptTarget::ES2024,
        "esnext" => ScriptTarget::ESNext,
        "es3" | "es5" => {
            diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::UnsupportedLegacyCompilerOptionValue,
                message: format!("legacy target `{raw}` is not supported; using es2015"),
                file_name: file_name.to_path_buf(),
            });
            ScriptTarget::ES2015
        }
        other => {
            diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
                message: format!("unsupported target `{other}`"),
                file_name: file_name.to_path_buf(),
            });
            ScriptTarget::ES2015
        }
    }
}

fn parse_module_option(
    value: &Value,
    file_name: &Path,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) -> ModuleKind {
    let Some(raw) = value.as_str() else {
        diagnostics.push(ConfigDiagnostic {
            code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
            message: "`module` must be a string".to_string(),
            file_name: file_name.to_path_buf(),
        });
        return ModuleKind::Preserve;
    };

    match raw.to_ascii_lowercase().as_str() {
        "commonjs" => ModuleKind::CommonJS,
        "es2015" => ModuleKind::ES2015,
        "es2020" => ModuleKind::ES2020,
        "es2022" => ModuleKind::ES2022,
        "esnext" => ModuleKind::ESNext,
        "node16" => ModuleKind::Node16,
        "node18" => ModuleKind::Node18,
        "nodenext" => ModuleKind::NodeNext,
        "preserve" => ModuleKind::Preserve,
        "amd" | "umd" | "system" | "systemjs" | "none" => {
            diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::UnsupportedLegacyCompilerOptionValue,
                message: format!("legacy module kind `{raw}` is not supported; using preserve"),
                file_name: file_name.to_path_buf(),
            });
            ModuleKind::Preserve
        }
        other => {
            diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
                message: format!("unsupported module kind `{other}`"),
                file_name: file_name.to_path_buf(),
            });
            ModuleKind::Preserve
        }
    }
}

fn parse_module_resolution_option(
    value: &Value,
    file_name: &Path,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) -> ModuleResolutionKind {
    let Some(raw) = value.as_str() else {
        diagnostics.push(ConfigDiagnostic {
            code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
            message: "`moduleResolution` must be a string".to_string(),
            file_name: file_name.to_path_buf(),
        });
        return ModuleResolutionKind::Bundler;
    };

    match raw.to_ascii_lowercase().as_str() {
        "node16" => ModuleResolutionKind::Node16,
        "nodenext" => ModuleResolutionKind::NodeNext,
        "bundler" => ModuleResolutionKind::Bundler,
        "classic" | "node" | "node10" => {
            diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::UnsupportedLegacyCompilerOptionValue,
                message: format!("legacy moduleResolution `{raw}` is not supported; using bundler"),
                file_name: file_name.to_path_buf(),
            });
            ModuleResolutionKind::Bundler
        }
        other => {
            diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
                message: format!("unsupported moduleResolution `{other}`"),
                file_name: file_name.to_path_buf(),
            });
            ModuleResolutionKind::Bundler
        }
    }
}

fn parse_jsx_option(
    value: &Value,
    file_name: &Path,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) -> Option<JsxMode> {
    let Some(raw) = value.as_str() else {
        diagnostics.push(ConfigDiagnostic {
            code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
            message: "`jsx` must be a string".to_string(),
            file_name: file_name.to_path_buf(),
        });
        return None;
    };

    match raw.to_ascii_lowercase().as_str() {
        "preserve" => Some(JsxMode::Preserve),
        "react" => Some(JsxMode::React),
        "react-jsx" => Some(JsxMode::ReactJsx),
        "react-jsxdev" => Some(JsxMode::ReactJsxDev),
        other => {
            diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
                message: format!("unsupported jsx mode `{other}`"),
                file_name: file_name.to_path_buf(),
            });
            None
        }
    }
}

fn validate_tsconfig_option_value(
    definition: &TsConfigOptionDefinition,
    value: &Value,
    config_dir: &Path,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) {
    match definition.value_kind {
        TsConfigOptionValueKind::Boolean => {
            let _ = parse_bool_option(definition.name, value, config_dir, diagnostics);
        }
        TsConfigOptionValueKind::String => {
            if value.as_str().is_none() {
                diagnostics.push(ConfigDiagnostic {
                    code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
                    message: format!("`{}` must be a string", definition.name),
                    file_name: config_dir.to_path_buf(),
                });
            }
        }
        TsConfigOptionValueKind::StringArray => {
            let _ = parse_string_list_option(value, diagnostics, config_dir);
        }
        TsConfigOptionValueKind::StringMapToStringArray => {
            let _ = parse_paths_option(value, diagnostics, config_dir);
        }
        TsConfigOptionValueKind::ObjectArray => {
            let Some(entries) = value.as_array() else {
                diagnostics.push(ConfigDiagnostic {
                    code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
                    message: format!("`{}` must be an array of objects", definition.name),
                    file_name: config_dir.to_path_buf(),
                });
                return;
            };

            for entry in entries {
                if !entry.is_object() {
                    diagnostics.push(ConfigDiagnostic {
                        code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
                        message: format!("`{}` entries must be objects", definition.name),
                        file_name: config_dir.to_path_buf(),
                    });
                    return;
                }
            }
        }
    }
}

fn handle_legacy_tsconfig_option(
    definition: &TsConfigOptionDefinition,
    value: &Value,
    config_dir: &Path,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) {
    match definition.name {
        "baseUrl" | "outFile" => {
            if value.as_str().is_none() {
                diagnostics.push(ConfigDiagnostic {
                    code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
                    message: format!("`{}` must be a string", definition.name),
                    file_name: config_dir.to_path_buf(),
                });
                return;
            }
        }
        "downlevelIteration" => {
            if value.as_bool().is_none() {
                diagnostics.push(ConfigDiagnostic {
                    code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
                    message: "`downlevelIteration` must be a boolean".to_string(),
                    file_name: config_dir.to_path_buf(),
                });
                return;
            }
        }
        _ => {
            validate_tsconfig_option_value(definition, value, config_dir, diagnostics);
            return;
        }
    }

    diagnostics.push(ConfigDiagnostic {
        code: ConfigDiagnosticCode::UnsupportedLegacyCompilerOption,
        message: format!(
            "legacy compiler option `{}` is not supported",
            definition.name
        ),
        file_name: config_dir.to_path_buf(),
    });
}

fn parse_path_list_option(
    value: &Value,
    config_dir: &Path,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) -> Vec<PathBuf> {
    let Some(entries) = value.as_array() else {
        diagnostics.push(ConfigDiagnostic {
            code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
            message: "`typeRoots` must be an array of strings".to_string(),
            file_name: config_dir.to_path_buf(),
        });
        return Vec::new();
    };

    let mut paths = Vec::new();
    for entry in entries {
        let Some(raw) = entry.as_str() else {
            diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
                message: "`typeRoots` entries must be strings".to_string(),
                file_name: config_dir.to_path_buf(),
            });
            continue;
        };
        paths.push(resolve_path(config_dir, raw));
    }
    paths
}

fn parse_string_list_option(
    value: &Value,
    diagnostics: &mut Vec<ConfigDiagnostic>,
    file_name: &Path,
) -> Vec<String> {
    let Some(entries) = value.as_array() else {
        diagnostics.push(ConfigDiagnostic {
            code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
            message: "expected an array of strings".to_string(),
            file_name: file_name.to_path_buf(),
        });
        return Vec::new();
    };

    let mut values = Vec::new();
    for entry in entries {
        let Some(raw) = entry.as_str() else {
            diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
                message: "array entries must be strings".to_string(),
                file_name: file_name.to_path_buf(),
            });
            continue;
        };
        values.push(raw.to_string());
    }
    values
}

fn parse_paths_option(
    value: &Value,
    diagnostics: &mut Vec<ConfigDiagnostic>,
    file_name: &Path,
) -> Vec<PathMapping> {
    let Some(object) = value.as_object() else {
        diagnostics.push(ConfigDiagnostic {
            code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
            message: "`paths` must be an object".to_string(),
            file_name: file_name.to_path_buf(),
        });
        return Vec::new();
    };

    let mut paths = Vec::new();
    for (pattern, substitutions_value) in object {
        let Some(substitutions_array) = substitutions_value.as_array() else {
            diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
                message: format!("paths entry `{pattern}` must be an array of strings"),
                file_name: file_name.to_path_buf(),
            });
            continue;
        };

        let mut substitutions = Vec::new();
        for substitution in substitutions_array {
            let Some(raw) = substitution.as_str() else {
                diagnostics.push(ConfigDiagnostic {
                    code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
                    message: format!("paths entry `{pattern}` contains a non-string substitution"),
                    file_name: file_name.to_path_buf(),
                });
                continue;
            };
            substitutions.push(raw.to_string());
        }

        paths.push(PathMapping {
            pattern: pattern.clone(),
            substitutions,
        });
    }

    paths
}
