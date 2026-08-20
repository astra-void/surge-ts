use std::collections::HashSet;
use std::path::Path;

use serde_json::{Map, Value};

use crate::diagnostics::{ConfigDiagnostic, ConfigDiagnosticCode};
use crate::extends::load_merged_config;
use crate::files::resolve_source_files;
use crate::model::{LoadedTsConfig, TsConfigLoadOptions};
use crate::normalize::normalize_compiler_options;
use crate::paths::resolve_project_path;

#[derive(Debug, Clone, Default)]
pub(crate) struct RawTsConfig {
    pub(crate) compiler_options: Option<Map<String, Value>>,
    pub(crate) files: Option<Vec<Value>>,
    pub(crate) include: Option<Vec<Value>>,
    pub(crate) exclude: Option<Vec<Value>>,
}

pub fn load_tsconfig(options: TsConfigLoadOptions) -> LoadedTsConfig {
    let (config_path, root_dir) = resolve_project_path(&options.project);
    let mut diagnostics = Vec::new();
    let mut visited = HashSet::new();

    let merged = load_merged_config(&config_path, &mut diagnostics, &mut visited);
    let compiler_options = normalize_compiler_options(
        merged.compiler_options.as_ref(),
        &root_dir,
        &mut diagnostics,
    );
    let files = resolve_source_files(
        &root_dir,
        merged.files.as_ref(),
        merged.include.as_ref(),
        merged.exclude.as_ref(),
        &compiler_options,
        &mut diagnostics,
    );

    let removed_options = crate::removed_options::collect_removed_options(
        &config_path,
        merged.compiler_options.as_ref(),
    );

    LoadedTsConfig {
        config_path,
        root_dir,
        files,
        compiler_options,
        diagnostics,
        removed_options,
    }
}

pub(crate) fn parse_current_config(
    config_path: &Path,
    object: &Map<String, Value>,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) -> RawTsConfig {
    let compiler_options = match object.get("compilerOptions") {
        Some(Value::Object(options)) => Some(options.clone()),
        Some(_) => {
            diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
                message: "`compilerOptions` must be a JSON object".to_string(),
                file_name: config_path.to_path_buf(),
            });
            None
        }
        None => None,
    };

    let files = parse_string_array_entry(
        object.get("files"),
        config_path,
        ConfigDiagnosticCode::InvalidFilesEntry,
        diagnostics,
    );
    let include = parse_string_array_entry(
        object.get("include"),
        config_path,
        ConfigDiagnosticCode::InvalidIncludeEntry,
        diagnostics,
    );
    let exclude = parse_string_array_entry(
        object.get("exclude"),
        config_path,
        ConfigDiagnosticCode::InvalidExcludeEntry,
        diagnostics,
    );

    RawTsConfig {
        compiler_options,
        files,
        include,
        exclude,
    }
}

pub(crate) fn parse_string_array_entry(
    value: Option<&Value>,
    config_path: &Path,
    code: ConfigDiagnosticCode,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) -> Option<Vec<Value>> {
    let Some(value) = value else {
        return None;
    };

    let Some(items) = value.as_array() else {
        diagnostics.push(ConfigDiagnostic {
            code,
            message: "expected an array of strings".to_string(),
            file_name: config_path.to_path_buf(),
        });
        return None;
    };

    Some(items.clone())
}

pub(crate) fn merge_configs(base: Option<&RawTsConfig>, child: &RawTsConfig) -> RawTsConfig {
    let mut compiler_options = base
        .and_then(|base| base.compiler_options.clone())
        .unwrap_or_default();
    if let Some(child_options) = &child.compiler_options {
        compiler_options.extend(child_options.clone());
    }

    RawTsConfig {
        compiler_options: if compiler_options.is_empty() {
            None
        } else {
            Some(compiler_options)
        },
        files: child
            .files
            .clone()
            .or_else(|| base.and_then(|base| base.files.clone())),
        include: child
            .include
            .clone()
            .or_else(|| base.and_then(|base| base.include.clone())),
        exclude: child
            .exclude
            .clone()
            .or_else(|| base.and_then(|base| base.exclude.clone())),
    }
}
