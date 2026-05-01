use std::{
    collections::HashSet,
    env, fs,
    path::{Path, PathBuf},
};

use globset::{Glob, GlobSet, GlobSetBuilder};
use jsonc_parser::parse_to_serde_value;
use serde_json::{Map, Value};
use walkdir::WalkDir;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TsConfigLoadOptions {
    pub project: PathBuf,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoadedTsConfig {
    pub config_path: PathBuf,
    pub root_dir: PathBuf,
    pub files: Vec<PathBuf>,
    pub compiler_options: NormalizedCompilerOptions,
    pub diagnostics: Vec<ConfigDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NormalizedCompilerOptions {
    pub strict: bool,
    pub no_implicit_any: bool,
    pub target: ScriptTarget,
    pub module: ModuleKind,
    pub module_resolution: ModuleResolutionKind,
    pub jsx: Option<JsxMode>,
    pub allow_js: bool,
    pub check_js: bool,
    pub no_emit: bool,
    pub skip_lib_check: bool,
    pub paths: Vec<PathMapping>,
    pub type_roots: Vec<PathBuf>,
    pub types: Vec<String>,
}

impl Default for NormalizedCompilerOptions {
    fn default() -> Self {
        Self {
            strict: true,
            no_implicit_any: true,
            target: ScriptTarget::ES2024,
            module: ModuleKind::Preserve,
            module_resolution: ModuleResolutionKind::Bundler,
            jsx: None,
            allow_js: false,
            check_js: false,
            no_emit: false,
            skip_lib_check: false,
            paths: Vec::new(),
            type_roots: Vec::new(),
            types: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScriptTarget {
    ES2015,
    ES2016,
    ES2017,
    ES2018,
    ES2019,
    ES2020,
    ES2021,
    ES2022,
    ES2023,
    ES2024,
    ESNext,
}

impl Default for ScriptTarget {
    fn default() -> Self {
        Self::ES2015
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleKind {
    CommonJS,
    ES2015,
    ES2020,
    ES2022,
    ESNext,
    Node16,
    Node18,
    NodeNext,
    Preserve,
}

impl Default for ModuleKind {
    fn default() -> Self {
        Self::Preserve
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleResolutionKind {
    Node16,
    NodeNext,
    Bundler,
}

impl Default for ModuleResolutionKind {
    fn default() -> Self {
        Self::Bundler
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsxMode {
    Preserve,
    React,
    ReactJsx,
    ReactJsxDev,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathMapping {
    pub pattern: String,
    pub substitutions: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TsConfigOptionSupport {
    Supported,
    KnownNoop,
    UnsupportedLegacy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TsConfigOptionValueKind {
    Boolean,
    String,
    StringArray,
    StringMapToStringArray,
    ObjectArray,
}

#[derive(Debug, Clone, Copy)]
pub struct TsConfigOptionDefinition {
    pub name: &'static str,
    pub value_kind: TsConfigOptionValueKind,
    pub support: TsConfigOptionSupport,
}

static TS_CONFIG_OPTION_DEFINITIONS: &[TsConfigOptionDefinition] = &[
    TsConfigOptionDefinition {
        name: "strict",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::Supported,
    },
    TsConfigOptionDefinition {
        name: "noImplicitAny",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::Supported,
    },
    TsConfigOptionDefinition {
        name: "target",
        value_kind: TsConfigOptionValueKind::String,
        support: TsConfigOptionSupport::Supported,
    },
    TsConfigOptionDefinition {
        name: "module",
        value_kind: TsConfigOptionValueKind::String,
        support: TsConfigOptionSupport::Supported,
    },
    TsConfigOptionDefinition {
        name: "moduleResolution",
        value_kind: TsConfigOptionValueKind::String,
        support: TsConfigOptionSupport::Supported,
    },
    TsConfigOptionDefinition {
        name: "jsx",
        value_kind: TsConfigOptionValueKind::String,
        support: TsConfigOptionSupport::Supported,
    },
    TsConfigOptionDefinition {
        name: "allowJs",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::Supported,
    },
    TsConfigOptionDefinition {
        name: "checkJs",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::Supported,
    },
    TsConfigOptionDefinition {
        name: "noEmit",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::Supported,
    },
    TsConfigOptionDefinition {
        name: "skipLibCheck",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::Supported,
    },
    TsConfigOptionDefinition {
        name: "paths",
        value_kind: TsConfigOptionValueKind::StringMapToStringArray,
        support: TsConfigOptionSupport::Supported,
    },
    TsConfigOptionDefinition {
        name: "typeRoots",
        value_kind: TsConfigOptionValueKind::StringArray,
        support: TsConfigOptionSupport::Supported,
    },
    TsConfigOptionDefinition {
        name: "types",
        value_kind: TsConfigOptionValueKind::StringArray,
        support: TsConfigOptionSupport::Supported,
    },
    TsConfigOptionDefinition {
        name: "rootDir",
        value_kind: TsConfigOptionValueKind::String,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "outDir",
        value_kind: TsConfigOptionValueKind::String,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "declaration",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "declarationMap",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "emitDeclarationOnly",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "sourceMap",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "inlineSourceMap",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "removeComments",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "importHelpers",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "isolatedModules",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "verbatimModuleSyntax",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "moduleDetection",
        value_kind: TsConfigOptionValueKind::String,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "resolveJsonModule",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "esModuleInterop",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "allowSyntheticDefaultImports",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "forceConsistentCasingInFileNames",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "noUncheckedIndexedAccess",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "exactOptionalPropertyTypes",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "strictNullChecks",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "strictFunctionTypes",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "strictBindCallApply",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "strictPropertyInitialization",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "noImplicitThis",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "alwaysStrict",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "noImplicitReturns",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "noFallthroughCasesInSwitch",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "noUnusedLocals",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "noUnusedParameters",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "allowUnreachableCode",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "allowUnusedLabels",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "lib",
        value_kind: TsConfigOptionValueKind::StringArray,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "plugins",
        value_kind: TsConfigOptionValueKind::ObjectArray,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "incremental",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "composite",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "tsBuildInfoFile",
        value_kind: TsConfigOptionValueKind::String,
        support: TsConfigOptionSupport::KnownNoop,
    },
    TsConfigOptionDefinition {
        name: "baseUrl",
        value_kind: TsConfigOptionValueKind::String,
        support: TsConfigOptionSupport::UnsupportedLegacy,
    },
    TsConfigOptionDefinition {
        name: "downlevelIteration",
        value_kind: TsConfigOptionValueKind::Boolean,
        support: TsConfigOptionSupport::UnsupportedLegacy,
    },
    TsConfigOptionDefinition {
        name: "outFile",
        value_kind: TsConfigOptionValueKind::String,
        support: TsConfigOptionSupport::UnsupportedLegacy,
    },
];

pub fn find_tsconfig_option(name: &str) -> Option<&'static TsConfigOptionDefinition> {
    TS_CONFIG_OPTION_DEFINITIONS
        .iter()
        .find(|definition| definition.name == name)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigDiagnostic {
    pub code: ConfigDiagnosticCode,
    pub message: String,
    pub file_name: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigDiagnosticCode {
    ConfigFileNotFound,
    ConfigParseError,
    ExtendsCycle,
    ExtendsFileNotFound,
    UnknownCompilerOption,
    InvalidCompilerOptionValue,
    UnsupportedLegacyCompilerOptionValue,
    UnsupportedLegacyCompilerOption,
    InvalidFilesEntry,
    InvalidIncludeEntry,
    InvalidExcludeEntry,
}

impl std::fmt::Display for ConfigDiagnosticCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let code = match self {
            Self::ConfigFileNotFound => "ConfigFileNotFound",
            Self::ConfigParseError => "ConfigParseError",
            Self::ExtendsCycle => "ExtendsCycle",
            Self::ExtendsFileNotFound => "ExtendsFileNotFound",
            Self::UnknownCompilerOption => "UnknownCompilerOption",
            Self::InvalidCompilerOptionValue => "InvalidCompilerOptionValue",
            Self::UnsupportedLegacyCompilerOptionValue => "UnsupportedLegacyCompilerOptionValue",
            Self::UnsupportedLegacyCompilerOption => "UnsupportedLegacyCompilerOption",
            Self::InvalidFilesEntry => "InvalidFilesEntry",
            Self::InvalidIncludeEntry => "InvalidIncludeEntry",
            Self::InvalidExcludeEntry => "InvalidExcludeEntry",
        };
        f.write_str(code)
    }
}

impl std::fmt::Display for ConfigDiagnostic {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} {}: {}",
            self.code,
            self.file_name.display(),
            self.message
        )
    }
}

#[derive(Debug, Clone, Default)]
struct RawTsConfig {
    compiler_options: Option<Map<String, Value>>,
    files: Option<Vec<Value>>,
    include: Option<Vec<Value>>,
    exclude: Option<Vec<Value>>,
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

    LoadedTsConfig {
        config_path,
        root_dir,
        files,
        compiler_options,
        diagnostics,
    }
}

fn resolve_project_path(project: &Path) -> (PathBuf, PathBuf) {
    let project = absolutize(project);

    if project.exists() && project.is_file() {
        let root_dir = project
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| project.clone());
        return (project, root_dir);
    }

    if project.exists() && project.is_dir() {
        let config_path = project.join("tsconfig.json");
        return (config_path, project);
    }

    if project
        .file_name()
        .is_some_and(|name| name == "tsconfig.json")
        || project.extension().is_some_and(|ext| ext == "json")
    {
        let root_dir = project
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
        (project, root_dir)
    } else {
        let config_path = project.join("tsconfig.json");
        (config_path, project)
    }
}

fn load_merged_config(
    config_path: &Path,
    diagnostics: &mut Vec<ConfigDiagnostic>,
    visited: &mut HashSet<PathBuf>,
) -> RawTsConfig {
    let config_key = cycle_key(config_path);
    if !visited.insert(config_key.clone()) {
        diagnostics.push(ConfigDiagnostic {
            code: ConfigDiagnosticCode::ExtendsCycle,
            message: "detected tsconfig extends cycle".to_string(),
            file_name: config_path.to_path_buf(),
        });
        return RawTsConfig::default();
    }

    let text = match fs::read_to_string(config_path) {
        Ok(text) => text,
        Err(_) => {
            diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::ConfigFileNotFound,
                message: format!("could not read {}", config_path.display()),
                file_name: config_path.to_path_buf(),
            });
            visited.remove(&config_key);
            return RawTsConfig::default();
        }
    };

    let value = match parse_to_serde_value::<Value>(&text, &Default::default()) {
        Ok(value) => value,
        Err(error) => {
            diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::ConfigParseError,
                message: error.to_string(),
                file_name: config_path.to_path_buf(),
            });
            visited.remove(&config_key);
            return RawTsConfig::default();
        }
    };

    let Some(object) = value.as_object() else {
        diagnostics.push(ConfigDiagnostic {
            code: ConfigDiagnosticCode::ConfigParseError,
            message: "tsconfig.json must contain a JSON object".to_string(),
            file_name: config_path.to_path_buf(),
        });
        visited.remove(&config_key);
        return RawTsConfig::default();
    };

    let parsed = parse_current_config(config_path, object, diagnostics);
    let base = match object.get("extends").and_then(Value::as_str) {
        Some(extends_spec) => match resolve_extends(config_path, extends_spec) {
            Ok(Some(base_path)) => Some(load_merged_config(&base_path, diagnostics, visited)),
            Ok(None) => None,
            Err(message) => {
                diagnostics.push(ConfigDiagnostic {
                    code: ConfigDiagnosticCode::ExtendsFileNotFound,
                    message,
                    file_name: config_path.to_path_buf(),
                });
                None
            }
        },
        None => None,
    };

    visited.remove(&config_key);
    merge_configs(base.as_ref(), &parsed)
}

fn parse_current_config(
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

fn parse_string_array_entry(
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

fn resolve_extends(config_path: &Path, extends_spec: &str) -> Result<Option<PathBuf>, String> {
    if Path::new(extends_spec).is_absolute() {
        let absolute = PathBuf::from(extends_spec);
        return if absolute.exists() {
            Ok(Some(absolute))
        } else {
            Err(format!(
                "Cannot resolve extends '{}' from {}",
                extends_spec,
                config_path.display()
            ))
        };
    }

    if extends_spec.starts_with("./")
        || extends_spec.starts_with("../")
        || extends_spec.starts_with('/')
    {
        return resolve_relative_extends(config_path, extends_spec);
    }

    let from_dir = config_path.parent().unwrap_or_else(|| Path::new("."));
    if let Some(resolved) = resolve_package_extends(extends_spec, from_dir) {
        return Ok(Some(resolved));
    }

    Err(format!(
        "Cannot resolve extends '{}' from {}",
        extends_spec,
        config_path.display()
    ))
}

fn resolve_relative_extends(
    config_path: &Path,
    extends_spec: &str,
) -> Result<Option<PathBuf>, String> {
    let base_dir = config_path.parent().unwrap_or_else(|| Path::new("."));
    let primary = base_dir.join(extends_spec);
    if primary.exists() {
        return Ok(Some(primary));
    }

    let json_candidate = if primary.extension().is_some() {
        primary.clone()
    } else {
        primary.with_extension("json")
    };
    if json_candidate.exists() {
        return Ok(Some(json_candidate));
    }

    Err(format!(
        "Cannot resolve extends '{}' from {}",
        extends_spec,
        config_path.display()
    ))
}

fn resolve_package_extends(specifier: &str, from_dir: &Path) -> Option<PathBuf> {
    let (package_name, subpath) = split_package_specifier(specifier)?;

    for ancestor in from_dir.ancestors() {
        let package_root = ancestor.join("node_modules").join(&package_name);
        let candidate = package_root.join(&subpath);
        if let Some(resolved) = resolve_existing_config_candidate(&candidate) {
            return Some(resolved);
        }
    }

    None
}

fn split_package_specifier(specifier: &str) -> Option<(String, String)> {
    let mut segments = specifier.split('/').filter(|segment| !segment.is_empty());
    let first = segments.next()?;

    let mut package_name = first.to_string();
    let mut subpath_segments = Vec::new();

    if first.starts_with('@') {
        let second = segments.next()?;
        package_name.push('/');
        package_name.push_str(second);
    }

    subpath_segments.extend(segments);

    let subpath = if subpath_segments.is_empty() {
        "tsconfig.json".to_string()
    } else {
        subpath_segments.join("/")
    };

    Some((package_name, subpath))
}

fn resolve_existing_config_candidate(candidate: &Path) -> Option<PathBuf> {
    if candidate.exists() && candidate.is_file() {
        return Some(canonicalize_if_exists(candidate));
    }

    if candidate.extension().is_none() {
        let json_candidate = candidate.with_extension("json");
        if json_candidate.exists() && json_candidate.is_file() {
            return Some(canonicalize_if_exists(&json_candidate));
        }
    }

    if candidate.exists() && candidate.is_dir() {
        let directory_default = candidate.join("tsconfig.json");
        if directory_default.exists() && directory_default.is_file() {
            return Some(canonicalize_if_exists(&directory_default));
        }
    }

    None
}

fn canonicalize_if_exists(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

fn merge_configs(base: Option<&RawTsConfig>, child: &RawTsConfig) -> RawTsConfig {
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

fn normalize_compiler_options(
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
            "paths" => {
                normalized.paths = parse_paths_option(value, diagnostics, config_dir);
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

fn resolve_source_files(
    root_dir: &Path,
    files: Option<&Vec<Value>>,
    include: Option<&Vec<Value>>,
    exclude: Option<&Vec<Value>>,
    compiler_options: &NormalizedCompilerOptions,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) -> Vec<PathBuf> {
    if let Some(files) = files {
        return resolve_explicit_files(root_dir, files, diagnostics);
    }

    let include_patterns = match include {
        Some(entries) => parse_pattern_list(
            entries,
            root_dir,
            ConfigDiagnosticCode::InvalidIncludeEntry,
            diagnostics,
        ),
        None => vec!["**/*".to_string()],
    };
    let mut exclude_patterns = vec![
        "**/node_modules".to_string(),
        "**/node_modules/**".to_string(),
        "**/bower_components".to_string(),
        "**/bower_components/**".to_string(),
        "**/jspm_packages".to_string(),
        "**/jspm_packages/**".to_string(),
    ];
    if let Some(entries) = exclude {
        exclude_patterns.extend(parse_pattern_list(
            entries,
            root_dir,
            ConfigDiagnosticCode::InvalidExcludeEntry,
            diagnostics,
        ));
    }

    let include_set = build_globset(&include_patterns, diagnostics, root_dir);
    let exclude_set = build_globset(&exclude_patterns, diagnostics, root_dir);

    let mut files = Vec::new();
    for entry in WalkDir::new(root_dir)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| !should_prune(entry.path(), root_dir, exclude_set.as_ref()))
    {
        let Ok(entry) = entry else {
            continue;
        };

        if !entry.file_type().is_file() {
            continue;
        }

        let path = entry.path();
        let Some(relative) = path.strip_prefix(root_dir).ok() else {
            continue;
        };

        if let Some(set) = include_set.as_ref() {
            if !set.is_match(relative) {
                continue;
            }
        }

        if is_supported_source_file(path, compiler_options.allow_js) {
            files.push(path.to_path_buf());
        }
    }

    files.sort();
    files.dedup();
    files
}

fn resolve_explicit_files(
    root_dir: &Path,
    files: &[Value],
    diagnostics: &mut Vec<ConfigDiagnostic>,
) -> Vec<PathBuf> {
    let mut results = Vec::new();
    let mut seen = HashSet::new();

    for file in files {
        let Some(raw) = file.as_str() else {
            diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::InvalidFilesEntry,
                message: "files entries must be strings".to_string(),
                file_name: root_dir.to_path_buf(),
            });
            continue;
        };

        let candidate = resolve_path(root_dir, raw);
        if !candidate.exists() || !candidate.is_file() {
            diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::InvalidFilesEntry,
                message: format!("file `{raw}` does not exist"),
                file_name: root_dir.to_path_buf(),
            });
            continue;
        }

        if !is_supported_source_file(&candidate, false) {
            continue;
        }

        if seen.insert(candidate.clone()) {
            results.push(candidate);
        }
    }

    results
}

fn parse_pattern_list(
    values: &[Value],
    file_name: &Path,
    code: ConfigDiagnosticCode,
    diagnostics: &mut Vec<ConfigDiagnostic>,
) -> Vec<String> {
    let mut patterns = Vec::new();
    for value in values {
        let Some(pattern) = value.as_str() else {
            diagnostics.push(ConfigDiagnostic {
                code,
                message: "pattern entries must be strings".to_string(),
                file_name: file_name.to_path_buf(),
            });
            continue;
        };
        patterns.push(pattern.to_string());
    }
    patterns
}

fn build_globset(
    patterns: &[String],
    diagnostics: &mut Vec<ConfigDiagnostic>,
    file_name: &Path,
) -> Option<GlobSet> {
    let mut builder = GlobSetBuilder::new();
    for pattern in patterns {
        match Glob::new(pattern) {
            Ok(glob) => {
                builder.add(glob);
            }
            Err(error) => diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
                message: format!("invalid glob `{pattern}`: {error}"),
                file_name: file_name.to_path_buf(),
            }),
        }
    }

    match builder.build() {
        Ok(set) => Some(set),
        Err(error) => {
            diagnostics.push(ConfigDiagnostic {
                code: ConfigDiagnosticCode::InvalidCompilerOptionValue,
                message: format!("failed to build globset: {error}"),
                file_name: file_name.to_path_buf(),
            });
            None
        }
    }
}

fn should_prune(path: &Path, root_dir: &Path, exclude_set: Option<&GlobSet>) -> bool {
    if path == root_dir {
        return false;
    }

    let Some(relative) = path.strip_prefix(root_dir).ok() else {
        return false;
    };

    match exclude_set {
        Some(set) => set.is_match(relative),
        None => false,
    }
}

fn is_supported_source_file(path: &Path, allow_js: bool) -> bool {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    if file_name.ends_with(".d.ts") {
        return false;
    }

    let Some(ext) = path.extension().and_then(|ext| ext.to_str()) else {
        return false;
    };

    match ext.to_ascii_lowercase().as_str() {
        "ts" | "tsx" => true,
        "js" | "jsx" => allow_js,
        _ => false,
    }
}

fn resolve_path(base_dir: &Path, raw: &str) -> PathBuf {
    let path = Path::new(raw);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base_dir.join(path)
    }
}

fn absolutize(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()
            .map(|cwd| cwd.join(path))
            .unwrap_or_else(|_| path.to_path_buf())
    }
}

fn cycle_key(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| absolutize(path))
}

#[cfg(test)]
mod tests {
    use super::*;

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
            diagnostic_codes(&loaded.diagnostics)
                .contains(&ConfigDiagnosticCode::ExtendsFileNotFound)
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
        assert!(
            diagnostic_codes(&loaded.diagnostics).contains(&ConfigDiagnosticCode::ExtendsCycle)
        );
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
        assert!(
            diagnostic_codes(&loaded.diagnostics).contains(&ConfigDiagnosticCode::ExtendsCycle)
        );
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
}
