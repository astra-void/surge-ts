use std::sync::Arc;

use surge_ts_syntax::{ParsedExportDeclaration, ParsedStatement, ParserWorker};

/// Shared module-specifier extraction for the loader's fixpoint scanners.
///
/// The package-declaration and import-graph scanners both need the module
/// specifiers of every source, and the loader loop used to make each of them
/// re-parse every file on every fixpoint iteration. This cache parses each
/// source exactly once (keyed by its append-only index in `sources`) and hands
/// both scanners the same extracted specifier list.
pub(crate) struct ModuleSpecifierScanner {
    parser: ParserWorker,
    scanned: Vec<Option<Arc<[String]>>>,
}

impl ModuleSpecifierScanner {
    pub(crate) fn new() -> Self {
        Self {
            parser: ParserWorker::new(),
            scanned: Vec::new(),
        }
    }

    /// Parse `sources[start..]` on a small worker pool and fill the cache, so
    /// the scanners' serial BFS resolution only pays lookup cost. Extraction is
    /// pure per file (one arena per worker thread, results index-ordered), so
    /// this changes no observable ordering. The loader's source reads are
    /// already unconditionally parallel, so this follows the same contract.
    pub(crate) fn prefetch(
        &mut self,
        sources: &[(std::path::PathBuf, String, String)],
        start: usize,
    ) {
        let pending: Vec<usize> = (start..sources.len())
            .filter(|&index| self.scanned.get(index).is_none_or(|slot| slot.is_none()))
            .collect();
        if pending.len() < 32 {
            return;
        }
        if self.scanned.len() < sources.len() {
            self.scanned.resize(sources.len(), None);
        }
        let workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .min(pending.len());
        if workers <= 1 {
            return;
        }
        let next = std::sync::atomic::AtomicUsize::new(0);
        let results: Vec<(usize, Arc<[String]>)> = std::thread::scope(|scope| {
            let pending = &pending;
            let next = &next;
            let mut handles = Vec::with_capacity(workers);
            for _ in 0..workers {
                handles.push(scope.spawn(move || {
                    let mut parser = ParserWorker::new();
                    let mut out = Vec::new();
                    loop {
                        let slot = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        if slot >= pending.len() {
                            break;
                        }
                        let index = pending[slot];
                        let (_, file_name, source_text) = &sources[index];
                        let parsed = parser.parse(source_text, file_name);
                        out.push((index, source_specifiers(parsed)));
                    }
                    out
                }));
            }
            handles
                .into_iter()
                .flat_map(|handle| handle.join().expect("specifier scan worker panicked"))
                .collect()
        });
        for (index, specifiers) in results {
            self.scanned[index] = Some(specifiers);
        }
    }

    /// Module specifiers of `sources[index]`, parsing on first request only.
    /// `index` must be the file's position in the loader's append-only
    /// `sources` vector so repeated requests hit the cache.
    pub(crate) fn specifiers(
        &mut self,
        index: usize,
        file_name: &str,
        source_text: &str,
    ) -> Arc<[String]> {
        if self.scanned.len() <= index {
            self.scanned.resize(index + 1, None);
        }
        if let Some(cached) = &self.scanned[index] {
            return cached.clone();
        }
        let parsed = self.parser.parse(source_text, file_name);
        let specifiers = source_specifiers(parsed);
        self.scanned[index] = Some(specifiers.clone());
        specifiers
    }
}

/// Declaration specifiers first, then the `import("...")` forms the lossy
/// `Parsed*` tree cannot carry. Both belong to the module graph: a package
/// reached only through an import type still supplies its
/// `/// <reference types>` directives and ambient `declare module` blocks.
fn source_specifiers(parsed: surge_ts_syntax::ParsedSource) -> Arc<[String]> {
    let surge_ts_syntax::ParsedSource {
        statements,
        import_call_specifiers,
        ..
    } = parsed;
    statements
        .into_iter()
        .filter_map(statement_module_specifier)
        .chain(import_call_specifiers)
        .collect::<Vec<_>>()
        .into()
}

fn statement_module_specifier(statement: ParsedStatement) -> Option<String> {
    match statement {
        ParsedStatement::ImportDeclaration(import) => Some(import.module_specifier),
        ParsedStatement::ExportDeclaration(export) => match *export {
            ParsedExportDeclaration::Named {
                module_specifier: Some(module_specifier),
                ..
            }
            | ParsedExportDeclaration::All {
                module_specifier, ..
            }
            | ParsedExportDeclaration::Namespace {
                module_specifier, ..
            } => Some(module_specifier),
            _ => None,
        },
        _ => None,
    }
}
