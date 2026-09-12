use std::sync::Arc;

use surge_ts_syntax::{ParsedExportDeclaration, ParsedSource, ParsedStatement, ParserWorker};

/// One scanned source: the specifiers both fixpoint scanners ask for, and the
/// parse they came from. The parse is kept because the checker would otherwise
/// re-parse the identical `(text, file_name)` pair from scratch —
/// [`ModuleSpecifierScanner::take_parsed_sources`] hands it over instead.
struct ScannedSource {
    specifiers: Arc<[String]>,
    parsed: Option<ParsedSource>,
}

/// Shared module-specifier extraction for the loader's fixpoint scanners.
///
/// The package-declaration and import-graph scanners both need the module
/// specifiers of every source, and the loader loop used to make each of them
/// re-parse every file on every fixpoint iteration. This cache parses each
/// source exactly once (keyed by its append-only index in `sources`) and hands
/// both scanners the same extracted specifier list.
pub(crate) struct ModuleSpecifierScanner {
    parser: ParserWorker,
    scanned: Vec<Option<ScannedSource>>,
    retain_parses: bool,
}

/// `SURGE_PRESCANNED_PARSE_REUSE=0` drops each parse as soon as its specifiers
/// are extracted, which puts the checker back on its own parse of every file.
/// Retention and reuse move together so the A/B arms differ in nothing else.
fn parse_reuse_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| std::env::var("SURGE_PRESCANNED_PARSE_REUSE").as_deref() != Ok("0"))
}

impl ModuleSpecifierScanner {
    pub(crate) fn new() -> Self {
        Self {
            parser: ParserWorker::new(),
            scanned: Vec::new(),
            retain_parses: parse_reuse_enabled(),
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
            self.scanned.resize_with(sources.len(), || None);
        }
        let workers = std::thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1)
            .min(pending.len());
        if workers <= 1 {
            return;
        }
        let next = std::sync::atomic::AtomicUsize::new(0);
        let retain_parses = self.retain_parses;
        let results: Vec<(usize, ScannedSource)> = std::thread::scope(|scope| {
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
                        out.push((
                            index,
                            ScannedSource {
                                specifiers: source_specifiers(&parsed),
                                parsed: retain_parses.then_some(parsed),
                            },
                        ));
                    }
                    out
                }));
            }
            handles
                .into_iter()
                .flat_map(|handle| handle.join().expect("specifier scan worker panicked"))
                .collect()
        });
        for (index, scanned) in results {
            self.scanned[index] = Some(scanned);
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
            self.scanned.resize_with(index + 1, || None);
        }
        if let Some(cached) = &self.scanned[index] {
            return cached.specifiers.clone();
        }
        let parsed = self.parser.parse(source_text, file_name);
        let specifiers = source_specifiers(&parsed);
        self.scanned[index] = Some(ScannedSource {
            specifiers: specifiers.clone(),
            parsed: self.retain_parses.then_some(parsed),
        });
        specifiers
    }

    /// Hand the scanned parses to the checker, which would otherwise parse the
    /// identical `(source text, file name)` pairs a second time. Every file the
    /// loader scanned is in the program, so nothing is dropped on the floor;
    /// default libs never enter the scan and stay the checker's to parse.
    pub(crate) fn take_parsed_sources(&mut self) -> Vec<ParsedSource> {
        self.scanned
            .iter_mut()
            .filter_map(|slot| slot.as_mut().and_then(|scanned| scanned.parsed.take()))
            .collect()
    }
}

/// Declaration specifiers first, then the `import("...")` forms the lossy
/// `Parsed*` tree cannot carry. Both belong to the module graph: a package
/// reached only through an import type still supplies its
/// `/// <reference types>` directives and ambient `declare module` blocks.
fn source_specifiers(parsed: &ParsedSource) -> Arc<[String]> {
    parsed
        .statements
        .iter()
        .filter_map(statement_module_specifier)
        .chain(parsed.import_call_specifiers.iter().map(String::as_str))
        .map(str::to_owned)
        .collect::<Vec<_>>()
        .into()
}

fn statement_module_specifier(statement: &ParsedStatement) -> Option<&str> {
    match statement {
        ParsedStatement::ImportDeclaration(import) => Some(&import.module_specifier),
        ParsedStatement::ExportDeclaration(export) => match &**export {
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
