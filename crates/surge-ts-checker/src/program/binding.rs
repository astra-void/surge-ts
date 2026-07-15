//! Module type/import binding collection across the multi-pass fixpoint.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Instant;

use surge_ts_diagnostics::Diagnostic;
use surge_ts_types::Type;

use super::*;

use crate::context::{CheckerContext, FileKind};
use crate::driver::{collect_type_declarations, lower_global_augmentation_values_from_statements};
use crate::modules::{
    ModuleExportTable, ModuleImportBindings, build_module_export_table,
    resolve_module_export_tables, resolve_module_imports,
};
use crate::symbols::{
    SymbolTable, TypeDeclarationInfo, TypeDeclarationScope, TypeDeclarationTable,
};

pub(crate) fn collect_preliminary_module_type_bindings(
    parsed_files: &[ParsedProgramFile],
    ctx: &mut CheckerContext,
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
) -> (
    Vec<Option<Arc<TypeDeclarationTable>>>,
    Vec<Option<ModuleImportBindings>>,
    Vec<Diagnostic>,
) {
    let mut local_type_declarations_by_module = Vec::with_capacity(parsed_files.len());
    let mut preliminary_local_export_tables = Vec::with_capacity(parsed_files.len());
    let mut preliminary_type_diagnostics = Vec::new();
    let initial_diagnostics_len = ctx.diagnostics().len();

    for parsed_file in parsed_files {
        if !parsed_file.is_module && parsed_file.file_kind != FileKind::DependencyDeclaration {
            local_type_declarations_by_module.push(None);
            preliminary_local_export_tables.push(None);
            continue;
        }

        ctx.set_file_name(parsed_file.file_name.clone());
        let saved_type_declaration_scope = ctx.type_declaration_scope.clone();
        ctx.type_declaration_scope = None;

        let diagnostics_before_collect = ctx.diagnostics().len();
        let saved_type_declarations =
            std::mem::replace(&mut ctx.type_declarations, TypeDeclarationTable::new());
        let saved_symbols = std::mem::replace(&mut ctx.symbols, SymbolTable::new());
        let collect_start = Instant::now();
        collect_type_declarations(&parsed_file.statements, ctx);
        let raw_local_type_declarations = Arc::new(std::mem::take(&mut ctx.type_declarations));
        let preliminary_raw_scope = Arc::new(TypeDeclarationScope::new(vec![
            raw_local_type_declarations.clone(),
        ]));
        let local_type_declarations = Arc::new(attach_resolution_scope_to_declarations(
            raw_local_type_declarations.as_ref(),
            preliminary_raw_scope,
        ));
        let lowered_type_declarations = local_type_declarations.len() as u64;
        let collect_duration = collect_start.elapsed();
        preliminary_type_diagnostics.extend(
            ctx.diagnostics()[diagnostics_before_collect..]
                .iter()
                .cloned(),
        );
        record_program_file_timing(timings, &parsed_file.file_name, |metrics| {
            metrics.collect_type_declarations_passes += 1;
            metrics.lowered_type_declarations += lowered_type_declarations;
            metrics.collect_type_declarations_duration += collect_duration;
        });

        let preliminary_resolution_scope = Arc::new(TypeDeclarationScope::new(vec![
            local_type_declarations.clone(),
        ]));
        let preliminary_export_table = build_module_export_table(
            parsed_file,
            local_type_declarations.as_ref(),
            &SymbolTable::new(), // empty symbols
            &SymbolTable::new(), // imports not resolved yet in the preliminary pass
            Some(preliminary_resolution_scope),
            ctx,
        );

        ctx.type_declarations = saved_type_declarations;
        ctx.symbols = saved_symbols;
        ctx.type_declaration_scope = saved_type_declaration_scope;

        local_type_declarations_by_module.push(Some(local_type_declarations));
        preliminary_local_export_tables.push(Some(preliminary_export_table));
    }

    let preliminary_export_resolution_start = Instant::now();
    let preliminary_module_export_tables =
        resolve_module_export_tables(parsed_files, &preliminary_local_export_tables, ctx);
    record_program_timing(timings, |timings| {
        timings.preliminary_export_table_resolution += preliminary_export_resolution_start.elapsed()
    });

    let preliminary_scope_start = Instant::now();
    let preliminary_module_resolution_scopes = local_type_declarations_by_module
        .iter()
        .map(|local_type_declarations| {
            let Some(local_type_declarations) = local_type_declarations else {
                return None;
            };

            Some(Arc::new(TypeDeclarationScope::new(vec![
                local_type_declarations.clone(),
            ])))
        })
        .collect::<Vec<_>>();
    record_program_timing(timings, |timings| {
        timings.module_resolution_scope_construction += preliminary_scope_start.elapsed()
    });

    let mut preliminary_module_import_bindings = Vec::with_capacity(parsed_files.len());
    let preliminary_import_resolution_start = Instant::now();
    for (_file_index, parsed_file) in parsed_files.iter().enumerate() {
        if !parsed_file.is_module && parsed_file.file_kind != FileKind::DependencyDeclaration {
            preliminary_module_import_bindings.push(None);
            continue;
        }

        ctx.set_file_name(parsed_file.file_name.clone());
        let imported_bindings = resolve_module_imports(
            parsed_file,
            parsed_files,
            &preliminary_module_export_tables,
            &preliminary_module_resolution_scopes,
            &SymbolTable::new(), // empty local symbols
            ctx,
        );
        preliminary_module_import_bindings.push(Some(imported_bindings));
    }
    record_program_timing(timings, |timings| {
        timings.import_binding_resolution += preliminary_import_resolution_start.elapsed()
    });

    let replayable_preliminary_diagnostics = ctx.diagnostics()[initial_diagnostics_len..]
        .iter()
        .filter(|diagnostic| should_replay_preliminary_diagnostic(diagnostic, ctx))
        .cloned()
        .collect::<Vec<_>>();
    ctx.truncate_diagnostics(initial_diagnostics_len);
    preliminary_type_diagnostics.extend(replayable_preliminary_diagnostics);

    (
        local_type_declarations_by_module,
        preliminary_module_import_bindings,
        preliminary_type_diagnostics,
    )
}

fn attach_resolution_scope_to_declarations(
    declarations: &TypeDeclarationTable,
    scope: Arc<TypeDeclarationScope>,
) -> TypeDeclarationTable {
    let mut attached = TypeDeclarationTable::new();
    for (name, declaration) in declarations.iter() {
        let declaration = match declaration.clone() {
            TypeDeclarationInfo::Alias(mut alias) => {
                alias.resolution_scope = Some(scope.clone());
                TypeDeclarationInfo::Alias(alias)
            }
            TypeDeclarationInfo::Interface(mut interface) => {
                interface.resolution_scope = Some(scope.clone());
                TypeDeclarationInfo::Interface(interface)
            }
        };
        let _ = attached.insert(name.as_ref(), declaration);
    }
    attached
}

/// Opt-in per-module memory trace (`SURGE_TRACE_MODULE_MEMORY=1`, or a byte
/// threshold instead of `1`): emits one JSON line per module whose analysis
/// raised the macOS physical-footprint high-water mark by at least the threshold
/// (default 8MB; RSS is used on platforms without `phys_footprint`),
/// identifying the module active at the high-water mark. Round numbers
/// distinguish the preliminary (1) and final (2) analysis passes.
fn module_memory_trace_threshold() -> Option<u64> {
    static THRESHOLD: std::sync::OnceLock<Option<u64>> = std::sync::OnceLock::new();
    *THRESHOLD.get_or_init(|| {
        let value = std::env::var("SURGE_TRACE_MODULE_MEMORY").ok()?;
        Some(
            value
                .parse::<u64>()
                .ok()
                .filter(|&threshold| threshold > 1)
                .unwrap_or(8 * 1024 * 1024),
        )
    })
}

pub(crate) fn collect_module_analyses_with_bindings(
    parsed_files: &[ParsedProgramFile],
    local_type_declarations_by_module: &[Option<Arc<TypeDeclarationTable>>],
    preliminary_module_import_bindings: &[Option<ModuleImportBindings>],
    ctx: &mut CheckerContext,
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
) -> Vec<Option<ModuleAnalysis>> {
    let mut analyses = Vec::with_capacity(parsed_files.len());
    let memory_trace_threshold = module_memory_trace_threshold();
    let mut type_dedup_cache = TypeDedupCache::new();
    let analysis_round = {
        static ROUND: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        ROUND.fetch_add(1, std::sync::atomic::Ordering::Relaxed) + 1
    };

    for (file_index, parsed_file) in parsed_files.iter().enumerate() {
        if !parsed_file.is_module && parsed_file.file_kind != FileKind::DependencyDeclaration {
            analyses.push(None);
            continue;
        }

        let eq_probe_start = super::eq_probe_enabled().then(Instant::now);
        let degraded_before = super::degraded_resolution_count();
        let memory_before = memory_trace_threshold.map(|_| {
            (
                Instant::now(),
                crate::metrics::rss::current_rss_bytes().unwrap_or(0),
                crate::metrics::rss::peak_rss_bytes().unwrap_or(0),
                crate::metrics::rss::current_footprint_bytes(),
                crate::metrics::rss::peak_footprint_bytes(),
            )
        });

        record_program_counter(|c| {
            c.module_analysis_total_calls += 1;
            c.module_analysis_unique_files += 1;
        });

        ctx.set_file_name(parsed_file.file_name.clone());
        let saved_type_declaration_scope = ctx.type_declaration_scope.clone();
        ctx.type_declaration_scope = None;
        let Some(local_type_declarations) = local_type_declarations_by_module[file_index].as_ref()
        else {
            analyses.push(None);
            ctx.type_declaration_scope = saved_type_declaration_scope;
            continue;
        };

        // Set up the full type environment for function signature collection
        let merge_start = Instant::now();
        let imported_layers = preliminary_module_import_bindings[file_index]
            .as_ref()
            .map(|bindings| bindings.scope_layers());
        let mut scope_layers = vec![local_type_declarations.clone()];
        if let Some(imported_layers) = imported_layers {
            scope_layers.extend(imported_layers);
        }
        let full_type_declarations_scope = Arc::new(TypeDeclarationScope::new(scope_layers));
        ctx.type_declarations = local_type_declarations.as_ref().clone();
        ctx.type_declaration_scope = Some(full_type_declarations_scope.clone());
        record_program_timing(timings, |timings| {
            timings.declaration_table_merging_cloning += merge_start.elapsed()
        });

        // `typeof <value>` inside a parameter annotation resolves against the
        // module's value bindings — imports and `const`s alike — which signature
        // collection alone never sees (function declarations hoist above them).
        // Collect the signatures inside a seeded environment (imported bindings +
        // the module's inferred value symbols, mirroring the check phase's merged
        // environment) so the exported function types carry real parameter types.
        // The seed is environment-only: `local_symbols` keeps just what collection
        // itself declared (the file's functions and classes) — a leaked seed would
        // re-report the declaration as TS2451 when the check phase declares it
        // again. Declaration files skip the seeding: their exports resolve through
        // the declaration tables, and running initializer inference over large
        // dependency `.d.ts` files here would be pure cost.
        // The per-file scope fallback (`module_scope_by_file`) is live only for
        // SIGNATURE collection: a parameter typed through a local alias of an
        // imported qualified type (`type BtnProps = React.ComponentProps<…>`)
        // must not bake a degraded signature into the module's symbols and
        // export table (the alias's attached scope carries no import layers).
        // Value collection and export-table construction stay map-less: with the
        // fallback live they eagerly materialize every exported initializer of a
        // large cyclic program (zod: +11s/+380MB), and their degraded shapes are
        // re-resolved lazily by the check phase anyway.
        let saved_module_scope_by_file = std::mem::take(&mut ctx.module_scope_by_file);
        let mut signature_env = SymbolTable::new();
        let mut seeded_names: std::collections::HashSet<Arc<str>> =
            std::collections::HashSet::new();
        if !parsed_file.file_kind.is_declaration() {
            let mut import_seed = SymbolTable::new();
            if let Some(bindings) = preliminary_module_import_bindings[file_index].as_ref() {
                for (name, symbol) in bindings.symbols.iter_shared() {
                    let _ = import_seed.insert_shared(name.clone(), symbol.clone());
                }
            }
            let value_env = crate::modules::collect_exportable_value_symbols(
                &parsed_file.statements,
                local_type_declarations.as_ref(),
                &import_seed,
                None,
                ctx,
            );
            for (name, symbol) in value_env.iter_shared() {
                let _ = signature_env.insert_shared(name.clone(), symbol.clone());
                seeded_names.insert(name.clone());
            }
        }
        ctx.module_scope_by_file = saved_module_scope_by_file;
        // The per-location signature map is only needed by the global path
        // (`collect_global_function_signatures`); module analysis consumes the
        // signatures through `signature_env` → `local_symbols`, so this one is
        // discarded.
        let mut discarded_function_signatures = HashMap::new();
        let diagnostics_before_signatures = ctx.diagnostics().len();
        let consults_before_signatures = super::scope_fallback_consult_count();
        with_dts_expansion_reason(DtsExpansionReason::ModuleExportCollection, || {
            collect_function_signatures_from_statements(
                &parsed_file.statements,
                file_index,
                &mut signature_env,
                &mut discarded_function_signatures,
                ctx,
            )
        });
        let signature_scope_consults =
            super::scope_fallback_consult_count() - consults_before_signatures;
        let saved_module_scope_by_file = std::mem::take(&mut ctx.module_scope_by_file);
        let mut local_symbols = SymbolTable::new();
        for (name, symbol) in signature_env.iter_shared() {
            if !seeded_names.contains(name) {
                let _ = local_symbols.insert_shared(name.clone(), symbol.clone());
            }
        }
        ctx.truncate_diagnostics(diagnostics_before_signatures);
        ctx.resolved_named_types = Arc::new(Mutex::new(HashMap::new()));

        // Lower this module's `declare global` augmentation values now that its
        // type environment (local declarations + import scope) is active. The
        // augmentation types were merged globally before binding, so a value such
        // as `var Buffer: BufferConstructor` sees the fully-merged interface while
        // `var x: ImportedType` still resolves through the module's imports.
        lower_global_augmentation_values_from_statements(&parsed_file.statements, ctx);

        let imported_symbols = preliminary_module_import_bindings[file_index]
            .as_ref()
            .map(|bindings| &bindings.symbols);
        let empty_imported_symbols = SymbolTable::new();
        let export_table =
            with_dts_expansion_reason(DtsExpansionReason::ModuleExportCollection, || {
                build_module_export_table(
                    parsed_file,
                    local_type_declarations.as_ref(),
                    &local_symbols,
                    imported_symbols.unwrap_or(&empty_imported_symbols),
                    Some(full_type_declarations_scope),
                    ctx,
                )
            });
        ctx.module_scope_by_file = saved_module_scope_by_file;

        let mut analysis = ModuleAnalysis {
            local_type_declarations: local_type_declarations.clone(),
            local_symbols,
            local_export_table: export_table,
        };
        if parsed_file.file_kind.is_declaration() && module_type_dedup_enabled() {
            with_dts_expansion_reason(DtsExpansionReason::ModuleDedup, || {
                dedup_module_analysis_types(&mut analysis, &mut type_dedup_cache)
            });
        }
        record_retained_export_nodes(
            &parsed_file.file_name,
            retained_module_analysis_type_nodes(&analysis),
        );
        analyses.push(Some(analysis));
        ctx.type_declaration_scope = saved_type_declaration_scope;
        if let Some(start) = eq_probe_start {
            super::record_eq_probe_visit(
                file_index,
                super::EqProbeVisit {
                    elapsed: start.elapsed(),
                    signature_scope_consults,
                    degraded_resolutions: super::degraded_resolution_count() - degraded_before,
                    augmentation_insertions_after: super::augmentation_value_insertion_count(),
                },
            );
        }
        if let Some((start, rss_before, peak_before, footprint_before, footprint_peak_before)) =
            memory_before
        {
            let peak_after = crate::metrics::rss::peak_rss_bytes().unwrap_or(0);
            let footprint_after = crate::metrics::rss::current_footprint_bytes();
            let footprint_peak_after = crate::metrics::rss::peak_footprint_bytes();
            let high_water_before = footprint_peak_before.unwrap_or(peak_before);
            let high_water_after = footprint_peak_after.unwrap_or(peak_after);
            if high_water_after.saturating_sub(high_water_before)
                >= memory_trace_threshold.unwrap_or(u64::MAX)
            {
                let rss_after = crate::metrics::rss::current_rss_bytes().unwrap_or(0);
                eprintln!(
                    "{{\"moduleMemory\":\"{}\",\"round\":{analysis_round},\"peakDeltaBytes\":{},\
                     \"rssBeforeBytes\":{rss_before},\"rssAfterBytes\":{rss_after},\
                     \"peakAfterBytes\":{peak_after},\"footprintBeforeBytes\":{},\
                     \"footprintAfterBytes\":{},\"peakFootprintAfterBytes\":{},\
                     \"elapsedMs\":{:.3}}}",
                    parsed_file.file_name,
                    high_water_after - high_water_before,
                    footprint_before.map_or_else(|| "null".to_string(), |v| v.to_string()),
                    footprint_after.map_or_else(|| "null".to_string(), |v| v.to_string()),
                    footprint_peak_after.map_or_else(|| "null".to_string(), |v| v.to_string()),
                    start.elapsed().as_secs_f64() * 1e3,
                );
            }
        }
    }

    analyses
}

/// Experiment toggle while the dedup is being validated: `SURGE_MODULE_TYPE_DEDUP=0`
/// disables the retained-type dedup without rebuilding.
fn module_type_dedup_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("SURGE_MODULE_TYPE_DEDUP").as_deref() != Ok("0"))
}

/// Node budget for the "is this type large enough to dedup" walk: a type whose
/// bounded traversal exhausts the budget is a dedup candidate. Small types are
/// cheaper to keep private than to hash.
const TYPE_DEDUP_NODE_BUDGET: usize = 64;

/// Bound on structurally-distinct types sharing one display key, so a
/// pathological same-display family cannot make every lookup a deep-equality
/// scan of an unbounded bucket.
const TYPE_DEDUP_BUCKET_CAP: usize = 8;

/// Per-round dedup store for retained module-analysis symbol types, keyed by
/// a bounded structural fingerprint. See [`dedup_module_analysis_types`].
type TypeDedupCache = HashMap<u64, Vec<Type>>;

/// Node budget for the fingerprint walk. Rendering (`Type::name`) is not usable
/// as a key here: displays render a shared DAG as a tree, so a heavily-shared
/// expansion explodes exponentially. The fingerprint instead hashes a bounded
/// DFS prefix — structure plus the display fields equality excludes
/// (`alias_name`, reference display) so payload sharing cannot change
/// diagnostic text within the fingerprinted prefix.
const TYPE_DEDUP_FINGERPRINT_NODE_BUDGET: usize = 4096;

fn type_dedup_fingerprint(ty: &Type) -> u64 {
    use std::hash::Hasher;
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    let mut budget = TYPE_DEDUP_FINGERPRINT_NODE_BUDGET;
    fingerprint_type(ty, &mut hasher, &mut budget);
    hasher.finish()
}

fn fingerprint_type(ty: &Type, hasher: &mut impl std::hash::Hasher, budget: &mut usize) {
    use std::hash::Hash;
    if *budget == 0 {
        return;
    }
    *budget -= 1;
    std::mem::discriminant(ty).hash(hasher);
    match ty {
        Type::StringLiteral(value) => value.hash(hasher),
        Type::NumberLiteral(value) => value.value.hash(hasher),
        Type::BooleanLiteral(value) => value.hash(hasher),
        Type::Object(object) => {
            object.alias_name.hash(hasher);
            object.alias_id.hash(hasher);
            object.properties.len().hash(hasher);
            for (name, property) in object.properties.iter() {
                name.hash(hasher);
                property.is_optional().hash(hasher);
                fingerprint_type(&property.ty, hasher, budget);
                if *budget == 0 {
                    return;
                }
            }
            if let Some(index) = object.string_index_type.as_deref() {
                fingerprint_type(index, hasher, budget);
            }
            if let Some(call) = object.call_signature() {
                fingerprint_function(call, hasher, budget);
            }
            if let Some(construct) = object.construct_signature() {
                fingerprint_function(construct, hasher, budget);
            }
        }
        Type::Function(function) => fingerprint_function(function, hasher, budget),
        Type::Array(element) => fingerprint_type(element, hasher, budget),
        Type::Tuple(elements) => {
            elements.len().hash(hasher);
            for element in elements {
                fingerprint_type(element, hasher, budget);
                if *budget == 0 {
                    return;
                }
            }
        }
        Type::Union(union) => {
            union.types().len().hash(hasher);
            for member in union.types() {
                fingerprint_type(member, hasher, budget);
                if *budget == 0 {
                    return;
                }
            }
        }
        Type::Reference(reference) => {
            reference.id.hash(hasher);
            reference.display.hash(hasher);
            reference.arguments.len().hash(hasher);
            for argument in reference.arguments.iter() {
                fingerprint_type(argument, hasher, budget);
                if *budget == 0 {
                    return;
                }
            }
        }
        _ => {}
    }
}

fn fingerprint_function(
    function: &surge_ts_types::FunctionType,
    hasher: &mut impl std::hash::Hasher,
    budget: &mut usize,
) {
    use std::hash::Hash;
    function.parameters().len().hash(hasher);
    for parameter in function.parameters() {
        fingerprint_type(parameter, hasher, budget);
        if *budget == 0 {
            return;
        }
    }
    fingerprint_type(function.return_type(), hasher, budget);
}

/// Full equality for dedup candidates. `Type::PartialEq` is unusable here: the
/// retained expansions are heavily-shared DAGs, and a naive recursive compare
/// of two separately-built equal DAGs expands them into trees (exponential
/// blow-up). This comparator memoizes visited payload-pointer pairs so every
/// distinct subtree pair is compared once, and — unlike `PartialEq` — includes
/// the display fields (`alias_name`, reference `display`), so a shared payload
/// can never change rendered diagnostic text.
fn types_identical_for_dedup(
    a: &Type,
    b: &Type,
    seen: &mut HashSet<(usize, usize)>,
    nodes_visited: &mut u64,
) -> bool {
    *nodes_visited += 1;
    fn pair_visited(seen: &mut HashSet<(usize, usize)>, a: usize, b: usize) -> bool {
        a == b || !seen.insert((a, b))
    }

    match (a, b) {
        (Type::Object(a), Type::Object(b)) => {
            if a.alias_name != b.alias_name
                || a.alias_id != b.alias_id
                || a.properties.len() != b.properties.len()
            {
                return false;
            }
            // The pointer-pair memo covers only the property-list recursion:
            // objects can share a properties `Arc` while differing in the
            // sibling index/call/construct fields (`open_if_unmodelled`), so
            // those are always compared.
            let (pa, pb) = (
                Arc::as_ptr(&a.properties) as usize,
                Arc::as_ptr(&b.properties) as usize,
            );
            if !pair_visited(seen, pa, pb) {
                for ((name_a, prop_a), (name_b, prop_b)) in
                    a.properties.iter().zip(b.properties.iter())
                {
                    if name_a != name_b
                        || prop_a.is_optional() != prop_b.is_optional()
                        || !types_identical_for_dedup(&prop_a.ty, &prop_b.ty, seen, nodes_visited)
                    {
                        return false;
                    }
                }
            }
            match (
                a.string_index_type.as_deref(),
                b.string_index_type.as_deref(),
            ) {
                (None, None) => {}
                (Some(ia), Some(ib)) => {
                    if !types_identical_for_dedup(ia, ib, seen, nodes_visited) {
                        return false;
                    }
                }
                _ => return false,
            }
            match (a.call_signature(), b.call_signature()) {
                (None, None) => {}
                (Some(ca), Some(cb)) => {
                    if !functions_identical_for_dedup(ca, cb, seen, nodes_visited) {
                        return false;
                    }
                }
                _ => return false,
            }
            match (a.construct_signature(), b.construct_signature()) {
                (None, None) => true,
                (Some(ca), Some(cb)) => functions_identical_for_dedup(ca, cb, seen, nodes_visited),
                _ => false,
            }
        }
        (Type::Function(a), Type::Function(b)) => {
            functions_identical_for_dedup(a, b, seen, nodes_visited)
        }
        (Type::Array(a), Type::Array(b)) => types_identical_for_dedup(a, b, seen, nodes_visited),
        (Type::Tuple(a), Type::Tuple(b)) => {
            a.len() == b.len()
                && a.iter()
                    .zip(b.iter())
                    .all(|(ea, eb)| types_identical_for_dedup(ea, eb, seen, nodes_visited))
        }
        (Type::Union(a), Type::Union(b)) => {
            let (ta, tb) = (a.types(), b.types());
            if ta.len() != tb.len() {
                return false;
            }
            // Empty slices share a dangling pointer, so only non-empty payloads
            // are valid identity keys (same below for reference arguments).
            if !ta.is_empty() && pair_visited(seen, ta.as_ptr() as usize, tb.as_ptr() as usize) {
                return true;
            }
            ta.iter()
                .zip(tb.iter())
                .all(|(ma, mb)| types_identical_for_dedup(ma, mb, seen, nodes_visited))
        }
        (Type::Reference(a), Type::Reference(b)) => {
            if a.id != b.id || a.display != b.display || a.arguments.len() != b.arguments.len() {
                return false;
            }
            if !a.arguments.is_empty()
                && pair_visited(
                    seen,
                    a.arguments.as_ptr() as usize,
                    b.arguments.as_ptr() as usize,
                )
            {
                return true;
            }
            a.arguments
                .iter()
                .zip(b.arguments.iter())
                .all(|(aa, ab)| types_identical_for_dedup(aa, ab, seen, nodes_visited))
        }
        (Type::StringLiteral(a), Type::StringLiteral(b)) => a == b,
        (Type::NumberLiteral(a), Type::NumberLiteral(b)) => a == b,
        (Type::BooleanLiteral(a), Type::BooleanLiteral(b)) => a == b,
        _ => std::mem::discriminant(a) == std::mem::discriminant(b),
    }
}

fn functions_identical_for_dedup(
    a: &surge_ts_types::FunctionType,
    b: &surge_ts_types::FunctionType,
    seen: &mut HashSet<(usize, usize)>,
    nodes_visited: &mut u64,
) -> bool {
    if a.is_variadic() != b.is_variadic()
        || a.required_parameter_count() != b.required_parameter_count()
        || a.parameters().len() != b.parameters().len()
    {
        return false;
    }
    if pair_visited_fn(seen, a, b) {
        return true;
    }
    a.parameters()
        .iter()
        .zip(b.parameters().iter())
        .all(|(pa, pb)| types_identical_for_dedup(pa, pb, seen, nodes_visited))
        && types_identical_for_dedup(a.return_type(), b.return_type(), seen, nodes_visited)
}

fn pair_visited_fn(
    seen: &mut HashSet<(usize, usize)>,
    a: &surge_ts_types::FunctionType,
    b: &surge_ts_types::FunctionType,
) -> bool {
    if a.parameters().is_empty() {
        return false;
    }
    let (pa, pb) = (
        a.parameters().as_ptr() as usize,
        b.parameters().as_ptr() as usize,
    );
    pa == pb || !seen.insert((pa, pb))
}

/// Walks `ty` decrementing `budget` per node; returns `true` (large) once the
/// budget is exhausted. References count their arguments but are never
/// resolved, so the walk cannot force an expansion.
fn type_dedup_budget_exhausted(ty: &Type, budget: &mut usize) -> bool {
    if *budget == 0 {
        return true;
    }
    *budget -= 1;
    match ty {
        Type::Object(object) => {
            object
                .properties
                .values()
                .any(|property| type_dedup_budget_exhausted(&property.ty, budget))
                || object
                    .string_index_type
                    .as_deref()
                    .is_some_and(|index| type_dedup_budget_exhausted(index, budget))
                || object
                    .call_signature()
                    .is_some_and(|call| function_dedup_budget_exhausted(call, budget))
                || object
                    .construct_signature()
                    .is_some_and(|construct| function_dedup_budget_exhausted(construct, budget))
        }
        Type::Function(function) => function_dedup_budget_exhausted(function, budget),
        Type::Array(element) => type_dedup_budget_exhausted(element, budget),
        Type::Tuple(elements) => elements
            .iter()
            .any(|element| type_dedup_budget_exhausted(element, budget)),
        Type::Union(union) => union
            .types()
            .iter()
            .any(|member| type_dedup_budget_exhausted(member, budget)),
        Type::Reference(reference) => reference
            .arguments
            .iter()
            .any(|argument| type_dedup_budget_exhausted(argument, budget)),
        _ => false,
    }
}

fn function_dedup_budget_exhausted(
    function: &surge_ts_types::FunctionType,
    budget: &mut usize,
) -> bool {
    function
        .parameters()
        .iter()
        .any(|parameter| type_dedup_budget_exhausted(parameter, budget))
        || type_dedup_budget_exhausted(function.return_type(), budget)
}

/// Returns the canonical equal type when `ty` duplicates one already retained
/// this round, sharing its payload instead of keeping a private copy.
///
/// Equality is `Type == Type` (structural/nominal) *plus* an equal display
/// fingerprint (the cache key covers `alias_name`/reference display), so a
/// shared payload cannot change a diagnostic's text within the fingerprinted
/// prefix. Sharing an equal `Type::Reference` is consistent with the program
/// instantiation interner, which already assumes `(declaration, resolved
/// arguments)` determines an expansion. The cache lives for one analysis round:
/// duplicates within a round (e.g. hundreds of icon `.d.ts` modules whose
/// identical annotation degrades and therefore never interns) collapse to one
/// payload, while cross-round sharing — where deferred-resolution snapshots
/// differ — is never attempted.
fn dedup_retained_type(ty: &Type, cache: &mut TypeDedupCache) -> Option<Type> {
    let mut budget = TYPE_DEDUP_NODE_BUDGET;
    if !type_dedup_budget_exhausted(ty, &mut budget) {
        return None;
    }
    let key = type_dedup_fingerprint(ty);
    let bucket = cache.entry(key).or_default();
    for existing in bucket.iter() {
        let nominal = matches!((existing, ty), (Type::Reference(_), Type::Reference(_)));
        let mut nodes_visited = 0;
        let identical =
            types_identical_for_dedup(existing, ty, &mut HashSet::new(), &mut nodes_visited);
        record_program_counter(|c| {
            c.module_type_dedup_structural_nodes_visited_count += nodes_visited;
            if !nominal {
                c.module_type_dedup_structural_comparison_count += 1;
            }
        });
        if identical {
            record_program_counter(|c| {
                c.module_type_dedup_hit_count += 1;
                if nominal {
                    c.module_type_dedup_nominal_match_count += 1;
                }
            });
            return Some(existing.clone());
        }
    }
    if bucket.len() < TYPE_DEDUP_BUCKET_CAP {
        record_program_counter(|c| c.module_type_dedup_insert_count += 1);
        bucket.push(ty.clone());
    }
    None
}

fn dedup_symbol_table_types(table: &mut SymbolTable, cache: &mut TypeDedupCache) {
    let replacements: Vec<_> = table
        .iter_shared()
        .filter_map(|(name, symbol)| {
            dedup_retained_type(&symbol.ty, cache).map(|canonical| {
                (
                    name.clone(),
                    Arc::new(crate::symbols::SymbolInfo {
                        ty: canonical,
                        kind: symbol.kind,
                        function_signature: symbol.function_signature.clone(),
                    }),
                )
            })
        })
        .collect();
    for (name, symbol) in replacements {
        let _ = table.insert_shared(name, symbol);
    }
}

fn dedup_symbol_handle_type(
    symbol: &mut Option<Arc<crate::symbols::SymbolInfo>>,
    cache: &mut TypeDedupCache,
) {
    if let Some(existing) = symbol.as_ref()
        && let Some(canonical) = dedup_retained_type(&existing.ty, cache)
    {
        *symbol = Some(Arc::new(crate::symbols::SymbolInfo {
            ty: canonical,
            kind: existing.kind,
            function_signature: existing.function_signature.clone(),
        }));
    }
}

/// Collapses value-identical large types retained by a declaration module's
/// analysis. Dependency graphs routinely contain hundreds of modules whose
/// exported annotation resolves to the same shape (icon packs: one
/// `ForwardRefExoticComponent<SVGProps & …>` per icon file); when that shape
/// degrades (`had_error`) it is barred from the instantiation interner, so
/// every module otherwise retains a private multi-megabyte copy — the dominant
/// transient of the module-analysis passes on trpc/unnamed.
fn dedup_module_analysis_types(analysis: &mut ModuleAnalysis, cache: &mut TypeDedupCache) {
    dedup_symbol_table_types(&mut analysis.local_symbols, cache);
    dedup_symbol_table_types(&mut analysis.local_export_table.symbols, cache);
    dedup_symbol_handle_type(&mut analysis.local_export_table.default_symbol, cache);
    dedup_symbol_handle_type(
        &mut analysis.local_export_table.export_assignment_symbol,
        cache,
    );
}

fn retained_module_analysis_type_nodes(analysis: &ModuleAnalysis) -> u64 {
    let mut seen = HashSet::new();
    let mut count = 0;
    for (_, symbol) in analysis.local_symbols.iter_shared() {
        count += retained_type_nodes(&symbol.ty, &mut seen);
    }
    for (_, symbol) in analysis.local_export_table.symbols.iter_shared() {
        count += retained_type_nodes(&symbol.ty, &mut seen);
    }
    if let Some(symbol) = &analysis.local_export_table.default_symbol {
        count += retained_type_nodes(&symbol.ty, &mut seen);
    }
    if let Some(symbol) = &analysis.local_export_table.export_assignment_symbol {
        count += retained_type_nodes(&symbol.ty, &mut seen);
    }
    count
}

fn retained_type_nodes(ty: &Type, seen: &mut HashSet<usize>) -> u64 {
    match ty {
        Type::Object(object) => {
            let identity = Arc::as_ptr(&object.properties) as usize;
            if !seen.insert(identity) {
                return 0;
            }
            1 + object
                .properties
                .values()
                .map(|property| retained_type_nodes(&property.ty, seen))
                .sum::<u64>()
                + object
                    .string_index_type
                    .as_deref()
                    .map_or(0, |index| retained_type_nodes(index, seen))
                + object
                    .call_signature()
                    .map_or(0, |call| retained_function_nodes(call, seen))
                + object
                    .construct_signature()
                    .map_or(0, |construct| retained_function_nodes(construct, seen))
        }
        Type::Function(function) => retained_function_nodes(function, seen),
        Type::Array(element) => 1 + retained_type_nodes(element, seen),
        Type::Tuple(elements) => {
            let identity = elements.as_ptr() as usize;
            if !elements.is_empty() && !seen.insert(identity) {
                return 0;
            }
            1 + elements
                .iter()
                .map(|element| retained_type_nodes(element, seen))
                .sum::<u64>()
        }
        Type::Union(union) => {
            let types = union.types();
            let identity = types.as_ptr() as usize;
            if !types.is_empty() && !seen.insert(identity) {
                return 0;
            }
            1 + types
                .iter()
                .map(|member| retained_type_nodes(member, seen))
                .sum::<u64>()
        }
        Type::Reference(reference) => {
            let identity = reference.arguments.as_ptr() as usize;
            if !reference.arguments.is_empty() && !seen.insert(identity) {
                return 0;
            }
            1 + reference
                .arguments
                .iter()
                .map(|argument| retained_type_nodes(argument, seen))
                .sum::<u64>()
        }
        _ => 1,
    }
}

fn retained_function_nodes(
    function: &surge_ts_types::FunctionType,
    seen: &mut HashSet<usize>,
) -> u64 {
    let identity = function.parameters().as_ptr() as usize;
    if !function.parameters().is_empty() && !seen.insert(identity) {
        return 0;
    }
    1 + function
        .parameters()
        .iter()
        .map(|parameter| retained_type_nodes(parameter, seen))
        .sum::<u64>()
        + retained_type_nodes(function.return_type(), seen)
}

pub(crate) fn build_module_resolution_scopes(
    local_type_declarations_by_module: &[Option<Arc<TypeDeclarationTable>>],
    module_import_bindings: &[Option<ModuleImportBindings>],
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
) -> Vec<Option<Arc<TypeDeclarationScope>>> {
    local_type_declarations_by_module
        .iter()
        .enumerate()
        .map(|(file_index, local_type_declarations)| {
            let Some(local_type_declarations) = local_type_declarations else {
                return None;
            };

            let clone_start = Instant::now();
            let mut layers = vec![local_type_declarations.clone()];
            if let Some(imported) = module_import_bindings
                .get(file_index)
                .and_then(|bindings| bindings.as_ref())
            {
                layers.extend(imported.scope_layers());
            }
            record_program_timing(timings, |timings| {
                timings.clone_copy_heavy_operations += clone_start.elapsed()
            });
            Some(Arc::new(TypeDeclarationScope::new(layers)))
        })
        .collect()
}

pub(crate) fn collect_module_import_bindings(
    parsed_files: &[ParsedProgramFile],
    module_analyses: &[Option<ModuleAnalysis>],
    module_export_tables: &[Option<ModuleExportTable>],
    module_resolution_scopes: &[Option<Arc<TypeDeclarationScope>>],
    ctx: &mut CheckerContext,
) -> Vec<Option<ModuleImportBindings>> {
    let mut module_import_bindings = Vec::with_capacity(parsed_files.len());

    for (file_index, parsed_file) in parsed_files.iter().enumerate() {
        if !parsed_file.is_module && parsed_file.file_kind != FileKind::DependencyDeclaration {
            module_import_bindings.push(None);
            continue;
        }

        let Some(module_analysis) = module_analyses[file_index].as_ref() else {
            module_import_bindings.push(None);
            continue;
        };

        ctx.set_file_name(parsed_file.file_name.clone());
        let imported_bindings = resolve_module_imports(
            parsed_file,
            parsed_files,
            module_export_tables,
            module_resolution_scopes,
            &module_analysis.local_symbols,
            ctx,
        );
        module_import_bindings.push(Some(imported_bindings));
    }

    module_import_bindings
}

pub(crate) fn should_replay_preliminary_diagnostic(
    diagnostic: &Diagnostic,
    ctx: &CheckerContext,
) -> bool {
    if diagnostic.code.to_string() != "TS2305" {
        return false;
    }

    let Some(module_specifier) = diagnostic
        .message
        .strip_prefix("Module '")
        .and_then(|message| {
            message
                .split_once("' has no exported member ")
                .map(|(module_specifier, _)| module_specifier)
        })
    else {
        return true;
    };

    ctx.ambient_modules.contains_key(module_specifier)
}

pub(crate) fn merge_module_import_bindings(
    final_bindings: &[Option<ModuleImportBindings>],
    fallback_bindings: &[Option<ModuleImportBindings>],
) -> Vec<Option<ModuleImportBindings>> {
    final_bindings
        .iter()
        .zip(fallback_bindings.iter())
        .map(|(final_bindings, fallback_bindings)| {
            let Some(final_bindings) = final_bindings else {
                return surge_ts_types::with_type_copy_reason(
                    surge_ts_types::TypeCopyReason::ModuleExport,
                    || fallback_bindings.clone(),
                );
            };

            let Some(fallback_bindings) = fallback_bindings else {
                return Some(
                    final_bindings.clone_with_reason(surge_ts_types::TypeCopyReason::ModuleExport),
                );
            };

            let mut merged_type_declarations = final_bindings.type_declarations.as_ref().clone();
            record_type_declaration_table_merge(
                None,
                fallback_bindings.type_declarations.len(),
                TableMergeKind::General,
            );
            for (name, declaration) in fallback_bindings.type_declarations.iter() {
                if merged_type_declarations.get(name.as_ref()).is_none() {
                    let _ = merged_type_declarations.insert(name.clone(), declaration.clone());
                }
            }
            let mut merged_bindings =
                final_bindings.clone_with_reason(surge_ts_types::TypeCopyReason::ModuleExport);
            merged_bindings.type_declarations = Arc::new(merged_type_declarations);
            for (name, symbol) in fallback_bindings.symbols.iter_shared() {
                if merged_bindings.symbols.get(name).is_none() {
                    let _ = merged_bindings
                        .symbols
                        .insert_shared(name.clone(), symbol.clone());
                }
            }

            Some(merged_bindings)
        })
        .collect()
}
