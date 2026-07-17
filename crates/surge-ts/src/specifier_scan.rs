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
        let specifiers: Arc<[String]> = parsed
            .statements
            .into_iter()
            .filter_map(statement_module_specifier)
            .collect::<Vec<_>>()
            .into();
        self.scanned[index] = Some(specifiers.clone());
        specifiers
    }
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
