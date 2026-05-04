use std::path::PathBuf;

use crate::diagnostics::ConfigDiagnostic;

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
    pub no_lib: bool,
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
            no_lib: false,
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
