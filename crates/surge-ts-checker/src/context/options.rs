use std::hash::Hash;

use surge_ts_types::fx::FxHashMap;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum DiagnosticProfile {
    #[default]
    Tsc,
    Native,
}

/// tsgo's `GetEmitModuleKind`, which grammar checks on `import =`/`export =`
/// read. `Preserve` is also what a caller that does not say gets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ModuleEmitKind {
    CommonJS,
    ES2015,
    ES2020,
    ES2022,
    ESNext,
    Node16,
    Node18,
    Node20,
    NodeNext,
    #[default]
    Preserve,
}

impl ModuleEmitKind {
    /// The `module` option's name, as tsgo's `ModuleKind.String()` prints it.
    pub(crate) fn option_name(self) -> &'static str {
        match self {
            Self::CommonJS => "CommonJS",
            Self::ES2015 => "ES2015",
            Self::ES2020 => "ES2020",
            Self::ES2022 => "ES2022",
            Self::ESNext => "ESNext",
            Self::Node16 => "Node16",
            Self::Node18 => "Node18",
            Self::Node20 => "Node20",
            Self::NodeNext => "NodeNext",
            Self::Preserve => "Preserve",
        }
    }

    /// `ES2015 <= kind <= ESNext`.
    pub fn is_ecmascript(self) -> bool {
        matches!(self, Self::ES2015 | Self::ES2020 | Self::ES2022 | Self::ESNext)
    }

    pub fn is_node(self) -> bool {
        matches!(self, Self::Node16 | Self::Node18 | Self::Node20 | Self::NodeNext)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum FileKind {
    RootSource,
    RootDeclaration,
    DependencyDeclaration,
    GeneratedDeclaration,
    /// A physical TypeScript `lib*.d.ts` default-lib file loaded from the
    /// installed `typescript` package (the default in project mode). Lowered
    /// through the real ambient-global pipeline, but its own diagnostics are
    /// suppressed like any other trusted upstream library file.
    PhysicalDefaultLib,
}

impl FileKind {
    pub(crate) fn is_declaration(self) -> bool {
        matches!(
            self,
            FileKind::RootDeclaration
                | FileKind::DependencyDeclaration
                | FileKind::GeneratedDeclaration
                | FileKind::PhysicalDefaultLib
        )
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CompatibilityStats {
    pub suppressed_diagnostics_total: usize,
    pub suppressed_declaration_diagnostics_total: usize,
    pub suppressed_rust_only_diagnostics_total: usize,
    /// Count of non-relative (package) import/export specifiers that failed every
    /// resolution path — the subset of `externalModuleStubs` references that were
    /// actually unresolved (and either stubbed or reported TS2307), rather than
    /// resolved via a dependency declaration. This is the parity-relevant figure:
    /// a resolved external reference is benign, an unresolved one is the risk.
    pub external_modules_unresolved_total: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckerOptions {
    pub no_implicit_any: bool,
    /// `noImplicitThis`: a `this` whose type is implicitly `any` is TS2683.
    pub no_implicit_this: bool,
    pub module_emit: ModuleEmitKind,
    /// tsgo's `GetUseDefineForClassFields`; off, a static `name`/`length`
    /// member collides with the constructor function's own (TS2699).
    pub use_define_for_class_fields: bool,
    /// `target` is ES2022 or later. Below it a static initializer's `super.x`
    /// is emitted through `Reflect` (TS2818), and together with
    /// `use_define_for_class_fields` it is tsgo's `GetEmitStandardClassFields`.
    pub target_es2022: bool,
    /// `noEmit`: tsgo drops the diagnostics it marks `SkippedOnNoEmit`, the
    /// emit-time name collisions (TS2441, TS2818, TS1216).
    pub no_emit: bool,
    /// `moduleResolution` is `node16` or `nodenext`, where an ESM import of a
    /// relative path must spell its extension (TS2834/TS2835).
    pub node_module_resolution: bool,
    /// Under node16/nodenext resolution, the files whose implied format is
    /// ESM (an `.mts`, or a `.ts` under a `"type": "module"` package.json).
    pub esm_module_files: std::collections::HashSet<String>,
    /// `strictNullChecks`. Off, `null` and `undefined` belong to every type:
    /// they drop out of unions and are assignable anywhere.
    pub strict_null_checks: bool,
    /// `strictPropertyInitialization`: an instance property with no initializer
    /// must be definitely assigned in the constructor.
    pub strict_property_initialization: bool,
    pub use_unknown_in_catch_variables: bool,
    pub no_implicit_returns: bool,
    pub no_fallthrough_cases_in_switch: bool,
    pub no_implicit_override: bool,
    pub no_property_access_from_index_signature: bool,
    pub no_unchecked_indexed_access: bool,
    pub allow_importing_ts_extensions: bool,
    pub no_unused_locals: bool,
    pub no_unused_parameters: bool,
    /// `allowUnreachableCode: true`; unset and `false` both leave it off.
    pub allow_unreachable_code: bool,
    pub stub_external_modules: bool,
    pub resolved_modules: FxHashMap<String, String>,
    /// Importer-scoped module resolutions: canonical importer file name →
    /// specifier → resolved file. Consulted before [`Self::resolved_modules`]
    /// so two importers can resolve the same bare specifier to different files
    /// (nested `node_modules`, package `#imports` scopes, self-name imports).
    /// `resolved_modules` stays as the project-wide fallback: `paths`/`baseUrl`
    /// mappings are importer-independent, and older callers only populate the
    /// flat map.
    pub resolved_modules_by_importer: FxHashMap<String, FxHashMap<String, String>>,
    /// Effective type-package names included in the program. When the project's
    /// `compilerOptions.types` used the `"*"` wildcard, the literal `"*"` is kept
    /// in this list as a sentinel (see [`Self::types_uses_wildcard`]); it never
    /// matches a real `@types` package path, so the other consumers ignore it.
    pub types: Vec<String>,
    pub no_lib: bool,
    pub skip_lib_check: bool,
    /// `jsx: react-jsx`/`react-jsxdev`: the JSX namespace resolves through the
    /// automatic runtime module, so intrinsic elements type-check without a
    /// `React` binding in scope. Under `preserve`/classic modes tsc requires the
    /// factory namespace to be visible, so the runtime fallback must not fire.
    pub jsx_automatic_runtime: bool,
    /// `jsx: react` exactly. It is the only mode in which tsc resolves the JSX
    /// factory namespace (`React`) with error reporting enabled, so it is the
    /// only mode where a JSX tag can report on that implicit reference —
    /// `preserve` and `react-native` resolve it silently, and the automatic
    /// runtime never names it.
    pub jsx_classic_react: bool,
    /// `compilerOptions.allowUmdGlobalAccess`: suppresses TS2686 entirely.
    /// tsc downgrades the diagnostic to a suggestion, a channel surge does not
    /// emit on, so the option reads as full suppression here.
    pub allow_umd_global_access: bool,
    /// `compilerOptions.resolveJsonModule`. Off, a `.json` specifier is not a
    /// module and the import reports `TS2732` instead of `TS2307`.
    pub resolve_json_module: bool,
    pub diagnostic_profile: DiagnosticProfile,
}

impl CheckerOptions {
    pub const ALLOW_SYNTHETIC_DEFAULT_IMPORTS_SENTINEL: &'static str =
        "\0allowSyntheticDefaultImports";

    /// Whether `compilerOptions.types` contained the `"*"` wildcard. Selects the
    /// node install-hint variant (TS2580 with a wildcard, TS2591 without),
    /// matching TypeScript's `usesWildcardTypes` branch.
    pub(crate) fn types_uses_wildcard(&self) -> bool {
        self.types.iter().any(|name| name == "*")
    }

    /// tsgo's `GetEmitStandardClassFields`: class fields are emitted as
    /// [[Define]] semantics, which lifts the constructor-emit rules
    /// (TS2301, TS2376, TS2401).
    pub(crate) fn emit_standard_class_fields(&self) -> bool {
        self.use_define_for_class_fields && self.target_es2022
    }

    pub(crate) fn allow_synthetic_default_imports(&self) -> bool {
        self.resolved_modules
            .contains_key(Self::ALLOW_SYNTHETIC_DEFAULT_IMPORTS_SENTINEL)
    }

    /// Resolve `specifier` from `importer_file`, preferring the importer-scoped
    /// map. Falls back to the project-wide map so paths/baseUrl mappings and
    /// flat-map callers keep working.
    pub(crate) fn resolved_module_for(
        &self,
        importer_file: &str,
        specifier: &str,
    ) -> Option<&String> {
        if let Some(per_importer) = self.resolved_modules_by_importer.get(importer_file) {
            if let Some(resolved) = per_importer.get(specifier) {
                return Some(resolved);
            }
        }
        self.resolved_modules.get(specifier)
    }
}

impl Default for CheckerOptions {
    fn default() -> Self {
        Self {
            no_implicit_any: false,
            no_implicit_this: false,
            module_emit: ModuleEmitKind::Preserve,
            use_define_for_class_fields: true,
            target_es2022: true,
            no_emit: false,
            node_module_resolution: false,
            esm_module_files: Default::default(),
            strict_null_checks: true,
            strict_property_initialization: false,
            use_unknown_in_catch_variables: false,
            no_implicit_returns: false,
            no_fallthrough_cases_in_switch: false,
            no_implicit_override: false,
            no_property_access_from_index_signature: false,
            no_unchecked_indexed_access: false,
            allow_importing_ts_extensions: false,
            no_unused_locals: false,
            no_unused_parameters: false,
            allow_unreachable_code: false,
            stub_external_modules: false,
            resolved_modules: FxHashMap::default(),
            resolved_modules_by_importer: FxHashMap::default(),
            types: Vec::new(),
            no_lib: false,
            skip_lib_check: false,
            jsx_automatic_runtime: false,
            jsx_classic_react: false,
            allow_umd_global_access: false,
            resolve_json_module: true,
            diagnostic_profile: DiagnosticProfile::default(),
        }
    }
}
