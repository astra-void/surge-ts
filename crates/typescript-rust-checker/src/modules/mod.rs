//! Module system: resolution, export tables, import binding, and diagnostics.
//!
//! Split into focused submodules. Shared types live here; each submodule does
//! `use super::*` to reach siblings via the re-exports below.

use std::sync::Arc;

use typescript_rust_types::{Type, TypeCopyReason, with_type_copy_reason};

use crate::symbols::{SymbolInfo, SymbolTable, TypeDeclarationTable};

mod diagnostics;
mod exports;
mod imports;
mod node_builtins;
mod resolution;

pub(crate) use diagnostics::*;
pub(crate) use exports::*;
pub(crate) use imports::*;
pub(crate) use node_builtins::*;
pub(crate) use resolution::*;

#[derive(Debug, Clone)]
pub(crate) struct ModuleResolution {
    pub(crate) resolved_file_index: usize,
    #[allow(dead_code)]
    pub(crate) resolved_file_name: String,
}

#[derive(Debug, Default)]
pub(crate) struct ModuleExportTable {
    pub(crate) type_declarations: TypeDeclarationTable,
    pub(crate) symbols: SymbolTable,
    pub(crate) default_symbol: Option<Arc<SymbolInfo>>,
    /// The value bound by a declaration-lite `export = identifier`. Consumed by
    /// `import local = require("specifier")` to bind `local`. Kept distinct from
    /// `default_symbol` so plain ESM default imports do not resolve through it
    /// (no synthetic default / `esModuleInterop`).
    pub(crate) export_assignment_symbol: Option<Arc<SymbolInfo>>,
    pub(crate) namespace_export_object_type: Option<Type>,
    pub(crate) has_unresolved_star_export: bool,
    pub(crate) has_incomplete_declaration_surface: bool,
}

impl Clone for ModuleExportTable {
    fn clone(&self) -> Self {
        crate::program::record_module_export_table_clone_count();
        let entry_count =
            self.symbols.iter_shared().count() as u64 + u64::from(self.default_symbol.is_some());
        crate::program::record_module_export_entry_clone_count(entry_count);
        crate::program::record_module_export_symbol_handle_copy_count(entry_count);

        Self {
            type_declarations: self.type_declarations.clone(),
            symbols: self.symbols.clone(),
            default_symbol: self.default_symbol.clone(),
            export_assignment_symbol: self.export_assignment_symbol.clone(),
            namespace_export_object_type: self.namespace_export_object_type.clone(),
            has_unresolved_star_export: self.has_unresolved_star_export,
            has_incomplete_declaration_surface: self.has_incomplete_declaration_surface,
        }
    }
}

impl ModuleExportTable {
    pub(crate) fn clone_with_reason(&self, reason: TypeCopyReason) -> Self {
        with_type_copy_reason(reason, || self.clone())
    }

    pub(crate) fn get_shared_value(&self, name: &str) -> Option<Arc<SymbolInfo>> {
        if name == "default" {
            return self.default_symbol.as_ref().map(|symbol| {
                crate::program::record_module_export_borrowed_lookup_count();
                crate::program::record_module_export_symbol_handle_copy_count(1);
                symbol.clone()
            });
        }

        self.symbols.get_shared(name).map(|symbol| {
            crate::program::record_module_export_borrowed_lookup_count();
            crate::program::record_module_export_symbol_handle_copy_count(1);
            symbol
        })
    }
}

#[derive(Debug, Clone, Default)]
pub(crate) struct ModuleImportBindings {
    pub(crate) type_declarations: Arc<TypeDeclarationTable>,
    pub(crate) symbols: SymbolTable,
}

impl ModuleImportBindings {
    pub(crate) fn clone_with_reason(&self, reason: TypeCopyReason) -> Self {
        with_type_copy_reason(reason, || self.clone())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;
    use crate::context::{CheckerContext, FileKind};
    use crate::program::ParsedProgramFile;

    fn program(files: &[(&str, &str)]) -> Vec<ParsedProgramFile> {
        files
            .iter()
            .map(|(file_name, source_text)| {
                let parsed = typescript_rust_syntax::parse_source(source_text, file_name);
                ParsedProgramFile {
                    file_name: parsed.file_name,
                    has_export_default: source_text.contains("export default"),
                    statements: parsed.statements,
                    parser_errors: parsed.parser_errors,
                    is_module: parsed.is_module,
                    file_kind: FileKind::RootSource,
                }
            })
            .collect()
    }

    fn resolve_relative_module(
        importer_file_name: &str,
        specifier: &str,
        program_files: &[ParsedProgramFile],
    ) -> Option<ModuleResolution> {
        let file_index_by_identity = program_files
            .iter()
            .enumerate()
            .map(|(index, file)| (canonical_file_identity(&file.file_name).into(), index))
            .collect::<HashMap<Arc<str>, usize>>();

        super::resolve_relative_module(
            importer_file_name,
            specifier,
            program_files,
            &file_index_by_identity,
        )
    }

    #[test]
    fn module_resolver_relative_same_dir_extensionless() {
        let files = program(&[("src/index.ts", "export {}"), ("src/user.ts", "export {}")]);
        let resolved = resolve_relative_module("src/index.ts", "./user", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user.ts");
    }

    #[test]
    fn module_resolver_relative_same_dir_with_ts_extension() {
        let files = program(&[("src/index.ts", "export {}"), ("src/user.ts", "export {}")]);
        let resolved = resolve_relative_module("src/index.ts", "./user.ts", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user.ts");
    }

    #[test]
    fn module_resolver_relative_parent_dir() {
        let files = program(&[("src/index.ts", "export {}"), ("user.ts", "export {}")]);
        let resolved = resolve_relative_module("src/index.ts", "../user", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "user.ts");
    }

    #[test]
    fn module_resolver_relative_dot_segments() {
        let files = program(&[("src/index.ts", "export {}"), ("src/user.ts", "export {}")]);
        let resolved = resolve_relative_module("src/pages/index.ts", ".././user", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user.ts");
    }

    #[test]
    fn module_resolver_relative_windows_separators() {
        let files = program(&[("src\\index.ts", "export {}"), ("src/user.ts", "export {}")]);
        let resolved = resolve_relative_module("src\\index.ts", ".\\user", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user.ts");
    }

    #[test]
    fn module_resolver_non_relative_unsupported_loaded_files_only() {
        let files = program(&[("src/index.ts", "export {}")]);
        assert!(resolve_relative_module("src/index.ts", "pkg", &files).is_none());
    }

    #[test]
    fn module_resolver_missing_file() {
        let files = program(&[("src/index.ts", "export {}")]);
        assert!(resolve_relative_module("src/index.ts", "./missing", &files).is_none());
    }

    #[test]
    fn module_resolver_index_file_optional_policy() {
        let files = program(&[
            ("src/pages/index.ts", "export {}"),
            ("src/user.ts", "export {}"),
        ]);
        let resolved = resolve_relative_module("src/pages/index.ts", "../user", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user.ts");
    }

    #[test]
    fn module_resolver_directory_index_current_directory() {
        let files = program(&[
            ("src/index.ts", "export {}"),
            ("src/models/index.ts", "export {}"),
            ("src/pages/index.ts", "export {}"),
        ]);
        let resolved = resolve_relative_module("src/pages/index.ts", "..", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/index.ts");
    }

    #[test]
    fn module_resolver_directory_index_grandparent_directory() {
        let files = program(&[
            ("src/index.ts", "export {}"),
            ("src/models/index.ts", "export {}"),
            ("src/pages/nested/index.ts", "export {}"),
        ]);
        let resolved =
            resolve_relative_module("src/pages/nested/index.ts", "../..", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/index.ts");
    }

    #[test]
    fn module_resolver_resolves_script_file_target_for_side_effect() {
        let files = program(&[
            ("src/index.ts", "import \"./setup\";"),
            ("src/setup.ts", "let initialized = true;"),
        ]);
        let resolved = resolve_relative_module("src/index.ts", "./setup", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/setup.ts");
    }

    #[test]
    fn module_resolver_resolves_module_file_target_for_side_effect() {
        let files = program(&[
            ("src/index.ts", "import \"./setup\";"),
            ("src/setup.ts", "export {};"),
        ]);
        let resolved = resolve_relative_module("src/index.ts", "./setup", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/setup.ts");
    }

    #[test]
    fn module_resolver_named_import_from_script_file_is_resolved_but_not_exported() {
        let files = program(&[
            ("src/index.ts", "import { value } from \"./setup\";"),
            ("src/setup.ts", "let value = 1;"),
        ]);
        let resolved = resolve_relative_module("src/index.ts", "./setup", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/setup.ts");
    }

    #[test]
    fn module_resolver_marks_unresolved_star_exports() {
        let files = program(&[("src/index.ts", "export * from \"./missing\";")]);
        let mut file_kinds = HashMap::new();
        file_kinds.insert("src/index.ts".to_string(), FileKind::RootSource);
        let mut ctx =
            CheckerContext::new("src/index.ts".to_string(), Default::default(), file_kinds);
        let local_tables = files
            .iter()
            .map(|file| {
                let local_types = TypeDeclarationTable::new();
                let local_symbols = SymbolTable::new();
                Some(build_module_export_table(
                    file,
                    &local_types,
                    &local_symbols,
                    None,
                    &mut ctx,
                ))
            })
            .collect::<Vec<_>>();
        let resolved = resolve_module_export_tables(&files, &local_tables, &mut ctx);

        assert!(
            resolved[0]
                .as_ref()
                .map(|table| table.has_unresolved_star_export)
                .unwrap_or(false)
        );
    }

    #[test]
    fn resolved_module_index_domain_no_panic() {
        // A resolved module index from the global project domain
        // (`module_file_index_by_identity`) can exceed the length of a narrow
        // local file vector — exactly what the ambient-module binding path passes.
        // The export resolver must degrade to a conservative unresolved result
        // rather than index out of bounds and panic.
        let files = program(&[("src/index.ts", "export { foo } from \"target-pkg\";")]);

        let mut file_kinds = HashMap::new();
        file_kinds.insert("src/index.ts".to_string(), FileKind::RootSource);
        let mut options = crate::context::CheckerOptions::default();
        options
            .resolved_modules
            .insert("target-pkg".to_string(), "src/target.ts".to_string());
        let mut ctx = CheckerContext::new("src/index.ts".to_string(), options, file_kinds);

        let mut identity = HashMap::new();
        identity.insert(canonical_file_identity("src/target.ts").into(), 194usize);
        ctx.set_module_file_index_by_identity(identity);

        let local_tables = files
            .iter()
            .map(|file| {
                Some(build_module_export_table(
                    file,
                    &TypeDeclarationTable::new(),
                    &SymbolTable::new(),
                    None,
                    &mut ctx,
                ))
            })
            .collect::<Vec<_>>();

        let resolved = resolve_module_export_tables(&files, &local_tables, &mut ctx);
        assert!(resolved[0].is_some());
    }

    #[test]
    fn module_resolver_extensionless_ts() {
        let files = program(&[("src/index.ts", "export {}"), ("src/user.ts", "export {}")]);
        let resolved = resolve_relative_module("src/index.ts", "./user", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user.ts");
    }

    #[test]
    fn module_resolver_explicit_ts_exact() {
        let files = program(&[("src/index.ts", "export {}"), ("src/user.ts", "export {}")]);
        let resolved = resolve_relative_module("src/index.ts", "./user.ts", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user.ts");
    }

    #[test]
    fn module_resolver_parent_directory() {
        let files = program(&[
            ("src/pages/index.ts", "export {}"),
            ("src/user.ts", "export {}"),
        ]);
        let resolved = resolve_relative_module("src/pages/index.ts", "../user", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user.ts");
    }

    #[test]
    fn module_resolver_dot_segments() {
        let files = program(&[
            ("src/pages/index.ts", "export {}"),
            ("src/user.ts", "export {}"),
        ]);
        let resolved = resolve_relative_module("src/pages/index.ts", ".././user", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user.ts");
    }

    #[test]
    fn module_resolver_windows_importer_path() {
        let files = program(&[("src\\index.ts", "export {}"), ("src/user.ts", "export {}")]);
        let resolved = resolve_relative_module("src\\index.ts", "./user", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user.ts");
    }

    #[test]
    fn module_resolver_windows_specifier() {
        let files = program(&[("src/index.ts", "export {}"), ("src/user.ts", "export {}")]);
        let resolved = resolve_relative_module("src/index.ts", ".\\user", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user.ts");
    }

    #[test]
    fn module_resolver_index_file() {
        let files = program(&[
            ("src/pages/index.ts", "export {}"),
            ("src/user/index.ts", "export {}"),
        ]);
        let resolved = resolve_relative_module("src/pages/index.ts", "../user", &files).unwrap();
        assert_eq!(resolved.resolved_file_name, "src/user/index.ts");
    }

    #[test]
    fn module_resolver_non_relative_unsupported() {
        let files = program(&[("src/index.ts", "export {}")]);
        assert!(resolve_relative_module("src/index.ts", "pkg", &files).is_none());
    }

    #[test]
    fn module_resolver_missing_relative_file() {
        let files = program(&[("src/index.ts", "export {}")]);
        assert!(resolve_relative_module("src/index.ts", "./missing", &files).is_none());
    }

    #[test]
    fn module_resolver_does_not_read_disk() {
        let files = program(&[("src/index.ts", "export {}")]);
        assert!(resolve_relative_module("src/index.ts", "./missing", &files).is_none());
    }

    #[test]
    fn module_resolver_relative_js_specifier_matches_ts_source() {
        let files = program(&[
            ("src/index.ts", "export {}"),
            ("src/user.tsx", "export {}"),
            ("src/user.js", "export {}"),
            ("src/user.jsx", "export {}"),
            ("src/user.json", "export {}"),
            ("src/user.d.ts", "export {}"),
        ]);

        assert!(resolve_relative_module("src/index.ts", "./user.tsx", &files).is_none());
        assert!(resolve_relative_module("src/index.ts", "./user.js", &files).is_some());
        assert!(resolve_relative_module("src/index.ts", "./user.jsx", &files).is_none());
        assert!(resolve_relative_module("src/index.ts", "./user.json", &files).is_none());
        assert!(resolve_relative_module("src/index.ts", "./user.d.ts", &files).is_none());
    }
}
