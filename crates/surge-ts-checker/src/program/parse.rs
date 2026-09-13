use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Instant;

use surge_ts_syntax::{ParsedSource, ParserWorker};

// Instrumentation lives in `metrics`; re-export it so existing callers that
// reference `crate::program::record_*` / `ProgramTimings` keep resolving and so
// the bare `record_*` calls throughout this file stay in scope.
use crate::metrics::*;

use crate::context::FileKind;
use super::{AUTO_JOBS, ParsedProgramFile, SourceFileInput, classify_file_kind};

/// Minimum source bytes assigned to a worker before `AUTO_JOBS` parallelizes the
/// parse phase. Parse cost tracks byte length, and files are pulled through a
/// shared cursor so large declaration/default-lib files don't stall one worker.
/// This is a calibration threshold — tune with `bench:test`.
pub(super) const MIN_BYTES_PER_PARSE_WORKER: usize = 256 * 1024;

pub(super) fn resolve_parse_worker_count(jobs: usize, files: &[SourceFileInput]) -> usize {
    let file_count = files.len();
    if file_count <= 1 {
        return 1;
    }

    let requested = if jobs == AUTO_JOBS {
        let cores = thread::available_parallelism()
            .map(|n| n.get())
            .unwrap_or(1);
        let bytes: usize = files.iter().map(|file| file.source_text.len()).sum();
        let by_work = (bytes / MIN_BYTES_PER_PARSE_WORKER).max(1);
        cores.min(by_work)
    } else {
        jobs
    };

    requested.max(1).min(file_count)
}

pub(super) fn parse_program_files(
    files: Vec<SourceFileInput>,
    prescanned: Vec<ParsedSource>,
    jobs: usize,
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
) -> Vec<ParsedProgramFile> {
    let prescanned = prescanned_by_file(prescanned, &files);
    let worker_count = resolve_parse_worker_count(jobs, &files);
    if worker_count <= 1 {
        let mut parser = ParserWorker::new();
        return files
            .iter()
            .zip(prescanned)
            .map(|(input, reused)| {
                parse_program_file(
                    &mut parser,
                    input,
                    reused.into_inner().ok().flatten(),
                    timings,
                )
            })
            .collect();
    }

    let next_index = AtomicUsize::new(0);
    let timings_owned = timings.cloned();

    let mut indexed = thread::scope(|scope| {
        let next_index = &next_index;
        let files = &files;
        let prescanned = &prescanned;
        let mut handles = Vec::with_capacity(worker_count);

        for _ in 0..worker_count {
            let timings = timings_owned.clone();
            handles.push(scope.spawn(move || {
                // One arena per parse thread; never shared across threads.
                let mut parser = ParserWorker::new();
                let mut worker_results = Vec::new();
                loop {
                    let file_index = next_index.fetch_add(1, Ordering::Relaxed);
                    if file_index >= files.len() {
                        break;
                    }
                    // Each index is claimed by exactly one worker, so the lock
                    // never contends; it is what lets a worker take ownership
                    // of its own slot out of the shared slice.
                    let reused = prescanned[file_index]
                        .lock()
                        .ok()
                        .and_then(|mut slot| slot.take());
                    worker_results.push((
                        file_index,
                        parse_program_file(
                            &mut parser,
                            &files[file_index],
                            reused,
                            timings.as_ref(),
                        ),
                    ));
                }
                worker_results
            }));
        }

        handles
            .into_iter()
            .flat_map(|handle| handle.join().expect("parse worker panicked"))
            .collect::<Vec<_>>()
    });

    indexed.sort_by_key(|(file_index, _)| *file_index);
    indexed.into_iter().map(|(_, parsed)| parsed).collect()
}

/// Both flags are deliberately *textual*, not AST facts: `contains_typeof`
/// gates releasing a declaration module's local symbols, so a `typeof` in a
/// comment or a string must keep the symbols alive. Every file's full text is
/// swept for them, which on a project whose dependency declarations dwarf its
/// sources is megabytes per run, and `str::contains` builds a fresh two-way
/// searcher on each call. The needle-only finders are built once instead.
pub(super) fn source_text_has_export_default(source_text: &str) -> bool {
    static FINDER: std::sync::OnceLock<memchr::memmem::Finder<'static>> =
        std::sync::OnceLock::new();
    FINDER
        .get_or_init(|| memchr::memmem::Finder::new("export default"))
        .find(source_text.as_bytes())
        .is_some()
}

pub(super) fn source_text_has_typeof(source_text: &str) -> bool {
    static FINDER: std::sync::OnceLock<memchr::memmem::Finder<'static>> =
        std::sync::OnceLock::new();
    FINDER
        .get_or_init(|| memchr::memmem::Finder::new("typeof"))
        .find(source_text.as_bytes())
        .is_some()
}

/// Line up the loader's parses with the program's file order. Matching is by
/// file name because the program splices in generated default libs of its own,
/// and a loader is free to hand over fewer files than the program checks.
pub(super) fn prescanned_by_file(
    prescanned: Vec<ParsedSource>,
    files: &[SourceFileInput],
) -> Vec<Mutex<Option<ParsedSource>>> {
    if prescanned.is_empty() {
        return files.iter().map(|_| Mutex::new(None)).collect();
    }

    let mut by_name = prescanned
        .into_iter()
        .map(|parsed| (parsed.file_name.clone(), parsed))
        .collect::<HashMap<_, _>>();
    files
        .iter()
        .map(|file| Mutex::new(by_name.remove(&file.file_name)))
        .collect()
}

pub(super) fn parse_program_file(
    parser: &mut ParserWorker,
    input: &SourceFileInput,
    prescanned: Option<ParsedSource>,
    timings: Option<&Arc<Mutex<ProgramTimings>>>,
) -> ParsedProgramFile {
    record_program_counter(|c| c.files_total += 1);
    let file_kind = classify_file_kind(&input.file_name);
    if file_kind == FileKind::GeneratedDeclaration {
        record_program_counter(|c| c.generated_default_lib_files += 1);
    }

    let parse_start = Instant::now();
    let parsed = match prescanned {
        Some(parsed) => parsed,
        None => parser.parse(&input.source_text, &input.file_name),
    };
    let parse_duration = parse_start.elapsed();
    let file_name = parsed.file_name;
    record_program_timing(timings, |timings| match file_kind {
        FileKind::DependencyDeclaration => {
            timings.dependency_declaration_parse_time += parse_duration
        }
        FileKind::GeneratedDeclaration | FileKind::PhysicalDefaultLib => {
            timings.generated_default_lib_parse_time += parse_duration
        }
        FileKind::RootSource | FileKind::RootDeclaration => {}
    });
    record_program_counter(|c| match file_kind {
        FileKind::RootSource => c.root_source_files += 1,
        FileKind::RootDeclaration | FileKind::DependencyDeclaration => {
            if matches!(file_kind, FileKind::DependencyDeclaration) {
                c.dependency_declaration_files += 1;
            }
            if matches!(file_kind, FileKind::RootDeclaration) {
                c.root_source_files += 1;
            }
            if matches!(file_kind, FileKind::DependencyDeclaration) {
                c.parsed_dependency_declaration_files += 1;
            } else {
                c.parsed_root_source_files += 1;
            }
        }
        FileKind::GeneratedDeclaration => {}
        FileKind::PhysicalDefaultLib => {
            c.generated_default_lib_files += 1;
        }
    });
    ParsedProgramFile {
        file_name: file_name.clone(),
        has_export_default: source_text_has_export_default(&input.source_text),
        contains_typeof: source_text_has_typeof(&input.source_text),
        statements: parsed.statements,
        parser_errors: parsed.parser_errors,
        is_module: parsed.is_module,
        import_call_specifiers: parsed.import_call_specifiers,
        file_kind,
        module_reads: parsed.module_reads,
        suppressed_ranges: parsed.suppressed_ranges,
        grammar_diagnostics: parsed.grammar_diagnostics,
        json_module_type: parsed.json_module_type,
    }
}
