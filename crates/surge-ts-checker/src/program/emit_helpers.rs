//! tsc's `checkExternalEmitHelpers` under `importHelpers`: some constructs
//! emit calls to helpers imported from `tslib`, so the checker resolves `tslib`
//! from each file that needs one (`resolveHelpersModule`) and looks every
//! helper the file asks for up in its exports.

use std::sync::Arc;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_syntax::{EmitHelperOptions, EmitHelperRequest, external_emit_helpers};
use surge_ts_types::Type;
use surge_ts_types::fx::FxHashMap;

use super::ParsedProgramFile;
use crate::context::{CheckerContext, CheckerOptions, ModuleEmitKind};
use crate::modules::ModuleExportTable;
use crate::spans::diagnostic_with_syntax_span;
use crate::symbols::{SymbolInfo, SymbolKind, TypeDeclarationScope};

const HELPERS_MODULE: &str = "tslib";

/// The requests a source file's check makes, when `importHelpers` applies to
/// it: tsgo asks only in a file that is an external module
/// (`IsEffectiveExternalModule`).
pub(super) fn file_emit_helper_requests(
    file_name: &str,
    source_text: &str,
    external_module: bool,
    options: &CheckerOptions,
) -> Vec<EmitHelperRequest> {
    if !options.import_helpers || !external_module {
        return Vec::new();
    }
    surge_ts_syntax::emit_helper_requests(
        source_text,
        file_name,
        &EmitHelperOptions {
            language_version: options.language_version as u32,
            legacy_decorators: options.experimental_decorators,
            use_define_for_class_fields: options.use_define_for_class_fields,
            emit_standard_class_fields: options.emit_standard_class_fields(),
            decorator_metadata: options.emit_decorator_metadata && !options.verbatim_module_syntax,
            commonjs: emits_commonjs(options, file_name),
        },
    )
}

/// tsgo's `GetEmitModuleFormatOfFile` answering CommonJS. A package.json
/// `"type"` is read under node resolution alone, where `esm_module_files`
/// records it.
fn emits_commonjs(options: &CheckerOptions, file_name: &str) -> bool {
    if options.module_emit.is_node() {
        return !options.esm_module_files.contains(file_name);
    }
    let lower = file_name.to_ascii_lowercase();
    if lower.ends_with(".mts") || lower.ends_with(".mjs") {
        return false;
    }
    options.module_emit == ModuleEmitKind::CommonJS || lower.ends_with(".cts") || lower.ends_with(".cjs")
}

/// Each file's `checkExternalEmitHelpers` diagnostics, by file index: a
/// `tslib` nothing answers is TS2354 at the file's first request
/// (`resolveHelpersModule` reports once), a helper its exports lack is TS2343
/// at the first request for it, and a private-field helper declared with too
/// few parameters for the transform is TS2807.
pub(super) fn collect_emit_helper_diagnostics(
    parsed_files: &[ParsedProgramFile],
    module_export_tables: &[Option<ModuleExportTable>],
    module_resolution_scopes: &[Option<Arc<TypeDeclarationScope>>],
    ctx: &mut CheckerContext,
) -> Vec<Vec<Diagnostic>> {
    let mut diagnostics: Vec<Vec<Diagnostic>> = parsed_files.iter().map(|_| Vec::new()).collect();
    if !ctx.options.import_helpers {
        return diagnostics;
    }
    let legacy_decorators = ctx.options.experimental_decorators;
    let saved_file_name = ctx.file_name.clone();
    // Files resolving `tslib` to the same module share its lookups.
    let mut exports_by_module: FxHashMap<Option<usize>, Arc<HelperExports>> = FxHashMap::default();
    for (file_index, parsed_file) in parsed_files.iter().enumerate() {
        if parsed_file.emit_helper_requests.is_empty() {
            continue;
        }
        ctx.set_file_name(parsed_file.file_name.clone());
        let exports = match crate::modules::try_resolve_module(
            HELPERS_MODULE,
            ctx,
            parsed_files,
            module_export_tables,
            module_resolution_scopes,
        ) {
            Some((export_table, _, resolved_index)) => match exports_by_module.get(&resolved_index) {
                Some(exports) => Some(exports.clone()),
                None => {
                    let complete =
                        export_surface_is_complete(&export_table, resolved_index, parsed_files, ctx);
                    let exports = Arc::new(HelperExports::of(&export_table, complete));
                    exports_by_module.insert(resolved_index, exports.clone());
                    Some(exports)
                }
            },
            // A `tslib` resolving to a file surge has no declarations for is
            // an untyped module (`errorOnImplicitAnyModule`), which asks for
            // nothing further.
            None if ctx.options.stub_external_modules
                || ctx
                    .options
                    .resolved_module_for(&parsed_file.file_name, HELPERS_MODULE)
                    .is_some() =>
            {
                continue;
            }
            None => None,
        };
        let mut requested = 0u32;
        for request in &parsed_file.emit_helper_requests {
            if let Some(specifier) = &request.unless_unresolved
                && crate::modules::try_resolve_module(
                    specifier,
                    ctx,
                    parsed_files,
                    module_export_tables,
                    module_resolution_scopes,
                )
                .is_none()
            {
                continue;
            }
            let Some(exports) = &exports else {
                diagnostics[file_index].push(diagnostic_with_syntax_span(
                    Diagnostic::ts2354(HELPERS_MODULE, parsed_file.file_name.clone()),
                    Some(request.span),
                ));
                break;
            };
            let unchecked = request.helpers & !requested;
            requested |= request.helpers;
            let mut helper = external_emit_helpers::FIRST;
            while helper <= external_emit_helpers::LAST {
                if unchecked & helper != 0 {
                    for &name in external_emit_helpers::names(helper, legacy_decorators) {
                        let diagnostic = match exports.answer(name) {
                            HelperExport::Missing => {
                                Diagnostic::ts2343(HELPERS_MODULE, name, parsed_file.file_name.clone())
                            }
                            HelperExport::TooFewParameters(count) => Diagnostic::ts2807(
                                HELPERS_MODULE,
                                name,
                                count,
                                parsed_file.file_name.clone(),
                            ),
                            HelperExport::Present => continue,
                        };
                        diagnostics[file_index]
                            .push(diagnostic_with_syntax_span(diagnostic, Some(request.span)));
                    }
                }
                helper <<= 1;
            }
        }
    }
    ctx.set_file_name(saved_file_name);
    diagnostics
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HelperExport {
    Present,
    Missing,
    /// `hasSignatureWithArityGreaterThan` fails; the count the transform
    /// passes.
    TooFewParameters(usize),
}

/// What a resolved `tslib` answers for each helper name.
struct HelperExports {
    answers: FxHashMap<&'static str, HelperExport>,
}

/// Whether surge saw the module's whole export list, as it must to report a
/// name the list lacks (the gate it reports TS2305 under): an `export *` it
/// could not follow, a declaration it could not lower, an `export =` whose
/// members it cannot list, or a JavaScript module's assignments may hold any
/// helper.
fn export_surface_is_complete(
    table: &ModuleExportTable,
    resolved_index: Option<usize>,
    parsed_files: &[ParsedProgramFile],
    ctx: &CheckerContext,
) -> bool {
    if table.has_unresolved_star_export
        || table.has_incomplete_declaration_surface
        || table.writes_export_assignment
    {
        return false;
    }
    let Some(index) = resolved_index else {
        return true;
    };
    parsed_files.get(index).is_some_and(|file| {
        file.json_module_type.is_none()
            && !surge_ts_syntax::is_javascript_file_name(&file.file_name)
            && !crate::modules::module_has_unresolved_star_export(
                index,
                parsed_files,
                &ctx.module_file_index_by_identity,
            )
    })
}

impl HelperExports {
    fn of(table: &ModuleExportTable, complete: bool) -> Self {
        let mut answers = FxHashMap::default();
        let mut helper = external_emit_helpers::FIRST;
        while helper <= external_emit_helpers::LAST {
            for legacy_decorators in [false, true] {
                for &name in external_emit_helpers::names(helper, legacy_decorators) {
                    answers
                        .entry(name)
                        .or_insert_with(|| helper_export(table, name, helper, complete));
                }
            }
            helper <<= 1;
        }
        Self { answers }
    }

    fn answer(&self, name: &str) -> HelperExport {
        self.answers.get(name).copied().unwrap_or(HelperExport::Present)
    }
}

/// `getSymbol(getExportsOfModule(helpersModule), name, SymbolFlagsValue)`, and
/// for the private-field helpers whether some signature takes more parameters
/// than the pre-4.3 helpers did (`__classPrivateFieldGet` four,
/// `__classPrivateFieldSet` five).
fn helper_export(table: &ModuleExportTable, name: &str, helper: u32, complete: bool) -> HelperExport {
    let Some(symbol) = crate::modules::lookup_value_export(table, name) else {
        return if complete { HelperExport::Missing } else { HelperExport::Present };
    };
    let arity = match helper {
        external_emit_helpers::CLASS_PRIVATE_FIELD_GET => 3,
        external_emit_helpers::CLASS_PRIVATE_FIELD_SET => 4,
        _ => return HelperExport::Present,
    };
    match declared_parameter_count(&symbol) {
        Some(count) if count <= arity => HelperExport::TooFewParameters(arity + 1),
        _ => HelperExport::Present,
    }
}

/// `getParameterCount` over a function export's signatures, the most any
/// overload takes; `None` when surge cannot count them as tsc does (not a
/// function declaration, or a rest parameter).
fn declared_parameter_count(symbol: &SymbolInfo) -> Option<usize> {
    let (SymbolKind::Function, Type::Function(function)) = (symbol.kind, &symbol.ty) else {
        return None;
    };
    let mut overloads = Vec::new();
    function.push_overload_members(&mut overloads);
    let mut count = 0;
    for signature in std::iter::once(function).chain(overloads.iter()) {
        if signature.is_variadic() {
            return None;
        }
        count = count.max(signature.parameters().len());
    }
    Some(count)
}
