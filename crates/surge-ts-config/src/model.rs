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
    /// `compilerOptions.noImplicitReturns`. Independent of `strict`; defaults off.
    pub no_implicit_returns: bool,
    /// `compilerOptions.noFallthroughCasesInSwitch`. Independent of `strict`; defaults off.
    pub no_fallthrough_cases_in_switch: bool,
    /// `compilerOptions.noImplicitOverride`. Independent of `strict`; defaults off.
    pub no_implicit_override: bool,
    /// `compilerOptions.noPropertyAccessFromIndexSignature`. Independent of `strict`; defaults off.
    pub no_property_access_from_index_signature: bool,
    /// `compilerOptions.noUnusedLocals`. Independent of `strict`; defaults off.
    pub no_unused_locals: bool,
    /// `compilerOptions.noUnusedParameters`. Independent of `strict`; defaults off.
    pub no_unused_parameters: bool,
    pub target: ScriptTarget,
    pub module: ModuleKind,
    pub module_resolution: ModuleResolutionKind,
    pub jsx: Option<JsxMode>,
    pub allow_js: bool,
    pub check_js: bool,
    pub no_emit: bool,
    pub skip_lib_check: bool,
    pub es_module_interop: bool,
    pub allow_synthetic_default_imports: bool,
    /// `compilerOptions.allowUmdGlobalAccess`. When true, referencing a UMD
    /// global from a module is permitted and TS2686 is suppressed.
    pub allow_umd_global_access: bool,
    pub no_lib: bool,
    pub lib: Vec<String>,
    pub paths: Vec<PathMapping>,
    /// `compilerOptions.baseUrl`, resolved to an absolute path against the
    /// config directory. `paths` substitutions and non-relative bare-import
    /// fallback resolution are anchored here, matching `tsc`. `None` when unset.
    pub base_url: Option<PathBuf>,
    pub type_roots: Vec<PathBuf>,
    /// `compilerOptions.types`. Under the pinned TypeScript 6 behavior, absent
    /// and empty both include no type packages; `["*"]` opts into visible
    /// `@types` discovery.
    pub types: Option<Vec<String>>,
    /// `compilerOptions.resolvePackageJsonExports`. When false, the package
    /// `exports` field is bypassed during declaration resolution. Defaults to
    /// `true` for modern resolvers (node16/nodenext/bundler).
    pub resolve_package_json_exports: bool,
    /// `compilerOptions.resolvePackageJsonImports`. When false, the package
    /// `imports` (`#alias`) field is bypassed. Defaults to `true` for modern
    /// resolvers.
    pub resolve_package_json_imports: bool,
    /// `compilerOptions.customConditions`. Extra export/import conditions that
    /// participate in condition matching, in configured priority order.
    pub custom_conditions: Vec<String>,
}

impl Default for NormalizedCompilerOptions {
    fn default() -> Self {
        Self {
            strict: true,
            no_implicit_any: true,
            no_implicit_returns: false,
            no_fallthrough_cases_in_switch: false,
            no_implicit_override: false,
            no_property_access_from_index_signature: false,
            no_unused_locals: false,
            no_unused_parameters: false,
            target: ScriptTarget::ES2024,
            module: ModuleKind::Preserve,
            module_resolution: ModuleResolutionKind::Bundler,
            jsx: None,
            allow_js: false,
            check_js: false,
            no_emit: false,
            skip_lib_check: false,
            es_module_interop: false,
            allow_synthetic_default_imports: false,
            allow_umd_global_access: false,
            no_lib: false,
            lib: Vec::new(),
            paths: Vec::new(),
            base_url: None,
            type_roots: Vec::new(),
            types: None,
            resolve_package_json_exports: true,
            resolve_package_json_imports: true,
            custom_conditions: Vec::new(),
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
    Node20,
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
    Node20,
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
