//! Post-run reporting: `--extendedDiagnostics`, `--memoryReport`, and
//! `--reportJson`.
//!
//! All reporting here runs strictly after checking completes: it reads the
//! already-collected `CliTimings`, classifies the returned source list by file
//! name, and samples process memory once. Nothing in this module touches the
//! checking hot path, and stdout stays reserved for diagnostics — the
//! human-readable blocks go to stderr and the JSON report goes to a file.

use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Map, Value};
use surge_ts::ProjectSource;

use crate::CliTimings;

pub(crate) const REPORT_SCHEMA_VERSION: u64 = 1;

#[derive(Debug, Clone)]
pub(crate) struct ReportRequest {
    pub(crate) extended: bool,
    pub(crate) memory: bool,
    pub(crate) json_path: Option<PathBuf>,
}

impl ReportRequest {
    pub(crate) fn any(&self) -> bool {
        self.extended || self.memory || self.json_path.is_some()
    }

    pub(crate) fn wants_timings(&self) -> bool {
        self.extended || self.json_path.is_some()
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct FileCounts {
    total: usize,
    source: usize,
    dependency_declaration: usize,
    default_lib: usize,
}

#[derive(Debug, Clone, Copy)]
struct MemorySample {
    peak_physical: Option<u64>,
    finish_physical: Option<u64>,
    peak_rss: Option<u64>,
}

pub(crate) fn emit_run_reports(
    request: &ReportRequest,
    sources: &[ProjectSource],
    diagnostics: usize,
    jobs: usize,
    timings: &CliTimings,
) -> Result<(), String> {
    if !request.any() {
        return Ok(());
    }

    let files = count_files(sources);
    let memory = sample_memory();

    if request.extended {
        eprint!(
            "{}",
            render_extended_diagnostics(&files, diagnostics, jobs, timings, &memory)
        );
    }
    if request.memory {
        eprint!("{}", render_memory_report(&memory));
    }
    if let Some(path) = &request.json_path {
        let json = build_report_json(&files, diagnostics, jobs, timings, &memory);
        let mut rendered = serde_json::to_string_pretty(&json)
            .map_err(|error| format!("failed to serialize --reportJson output: {error}"))?;
        rendered.push('\n');
        std::fs::write(path, rendered)
            .map_err(|error| format!("failed to write {}: {error}", path.display()))?;
    }
    Ok(())
}

fn count_files(sources: &[ProjectSource]) -> FileCounts {
    let mut counts = FileCounts {
        total: sources.len(),
        ..FileCounts::default()
    };
    for (_, file_name, _) in sources {
        if is_default_lib_file_name(file_name) {
            counts.default_lib += 1;
        } else if is_declaration_file_name(file_name) && {
            let normalized = normalize(file_name);
            normalized.contains("/node_modules/")
        } {
            counts.dependency_declaration += 1;
        } else {
            counts.source += 1;
        }
    }
    counts
}

fn normalize(file_name: &str) -> String {
    file_name.replace('\\', "/").to_ascii_lowercase()
}

fn is_declaration_file_name(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.ends_with(".d.ts") || lower.ends_with(".d.mts") || lower.ends_with(".d.cts")
}

// Mirrors the checker's name-based default-lib routing: physical TypeScript
// libs live at `.../typescript/lib/lib.*.d.ts`, and the generated fallback
// subset is loaded from the checker crate's `generated-libs/` directory.
fn is_default_lib_file_name(file_name: &str) -> bool {
    let normalized = normalize(file_name);
    if normalized.contains("/generated-libs/") {
        return true;
    }
    let Some(split) = normalized.rfind('/') else {
        return false;
    };
    let (dir, file) = normalized.split_at(split);
    let file = &file[1..];
    dir.ends_with("/typescript/lib") && file.starts_with("lib.") && file.ends_with(".d.ts")
}

fn sample_memory() -> MemorySample {
    MemorySample {
        peak_physical: probe::peak_footprint_bytes(),
        finish_physical: probe::current_footprint_bytes(),
        peak_rss: probe::peak_rss_bytes(),
    }
}

fn jobs_label(jobs: usize) -> String {
    if jobs == 0 {
        "auto".to_string()
    } else {
        jobs.to_string()
    }
}

fn jobs_json(jobs: usize) -> Value {
    if jobs == 0 {
        Value::String("auto".to_string())
    } else {
        Value::from(jobs as u64)
    }
}

fn duration_ms(duration: Duration) -> f64 {
    (duration.as_secs_f64() * 1_000_000.0).round() / 1000.0
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const MIB: f64 = KIB * 1024.0;
    const GIB: f64 = MIB * 1024.0;
    let value = bytes as f64;
    if value >= GIB {
        format!("{:.2} GiB", value / GIB)
    } else if value >= MIB {
        format!("{:.1} MiB", value / MIB)
    } else if value >= KIB {
        format!("{:.1} KiB", value / KIB)
    } else {
        format!("{bytes} B")
    }
}

fn format_bytes_opt(bytes: Option<u64>) -> String {
    bytes.map_or_else(|| "unavailable".to_string(), format_bytes)
}

fn render_aligned(title: &str, rows: &[(String, String)]) -> String {
    let label_width = rows.iter().map(|(label, _)| label.len()).max().unwrap_or(0);
    let value_width = rows.iter().map(|(_, value)| value.len()).max().unwrap_or(0);
    let mut out = String::with_capacity(64 + rows.len() * (label_width + value_width + 4));
    out.push_str(title);
    out.push('\n');
    for (label, value) in rows {
        out.push_str(&format!("  {label:<label_width$}  {value:>value_width$}\n"));
    }
    out
}

fn memory_rows(memory: &MemorySample) -> Vec<(String, String)> {
    vec![
        (
            "peak physical footprint:".to_string(),
            format_bytes_opt(memory.peak_physical),
        ),
        (
            "finish physical footprint:".to_string(),
            format_bytes_opt(memory.finish_physical),
        ),
        ("peak rss:".to_string(), format_bytes_opt(memory.peak_rss)),
    ]
}

fn render_extended_diagnostics(
    files: &FileCounts,
    diagnostics: usize,
    jobs: usize,
    timings: &CliTimings,
    memory: &MemorySample,
) -> String {
    let duration = |value: Duration| crate::format_duration(value);
    let mut rows = vec![
        ("files:".to_string(), files.total.to_string()),
        ("  source files:".to_string(), files.source.to_string()),
        (
            "  dependency declaration files:".to_string(),
            files.dependency_declaration.to_string(),
        ),
        (
            "  default lib files:".to_string(),
            files.default_lib.to_string(),
        ),
        ("diagnostics:".to_string(), diagnostics.to_string()),
        ("jobs:".to_string(), jobs_label(jobs)),
        (
            "allocator:".to_string(),
            crate::global_alloc::ACTIVE_ALLOCATOR.to_string(),
        ),
        (
            "config/project loading:".to_string(),
            duration(timings.config_project_loading),
        ),
        (
            "file discovery:".to_string(),
            duration(timings.file_discovery),
        ),
        (
            "default lib loading:".to_string(),
            duration(timings.default_lib_loading),
        ),
        (
            "package declaration discovery:".to_string(),
            duration(timings.package_declaration_discovery),
        ),
        (
            "import graph expansion:".to_string(),
            duration(timings.import_graph_expansion),
        ),
        (
            "path mapping resolution:".to_string(),
            duration(timings.path_mapping_resolution),
        ),
        ("checking:".to_string(), duration(timings.checking)),
        (
            "diagnostic rendering:".to_string(),
            duration(timings.diagnostic_rendering),
        ),
        ("total:".to_string(), duration(timings.total)),
    ];
    rows.extend(memory_rows(memory));
    render_aligned("Extended diagnostics:", &rows)
}

fn render_memory_report(memory: &MemorySample) -> String {
    render_aligned("Memory report:", &memory_rows(memory))
}

fn json_bytes(bytes: Option<u64>) -> Value {
    bytes.map_or(Value::Null, Value::from)
}

// Field order is fixed by insertion order (`serde_json` is built with
// `preserve_order`), so the emitted key sequence is deterministic run-to-run.
fn build_report_json(
    files: &FileCounts,
    diagnostics: usize,
    jobs: usize,
    timings: &CliTimings,
    memory: &MemorySample,
) -> Value {
    let mut summary = Map::new();
    summary.insert("files".to_string(), Value::from(files.total as u64));
    summary.insert("sourceFiles".to_string(), Value::from(files.source as u64));
    summary.insert(
        "dependencyDeclarationFiles".to_string(),
        Value::from(files.dependency_declaration as u64),
    );
    summary.insert(
        "defaultLibFiles".to_string(),
        Value::from(files.default_lib as u64),
    );
    summary.insert("diagnostics".to_string(), Value::from(diagnostics as u64));
    summary.insert(
        "wallTimeMs".to_string(),
        Value::from(duration_ms(timings.total)),
    );
    summary.insert("jobs".to_string(), jobs_json(jobs));
    summary.insert(
        "allocator".to_string(),
        Value::String(crate::global_alloc::ACTIVE_ALLOCATOR.to_string()),
    );

    let mut phases = Map::new();
    for (key, value) in [
        ("configProjectLoadingMs", timings.config_project_loading),
        ("fileDiscoveryMs", timings.file_discovery),
        ("defaultLibLoadingMs", timings.default_lib_loading),
        (
            "packageDeclarationDiscoveryMs",
            timings.package_declaration_discovery,
        ),
        ("importGraphExpansionMs", timings.import_graph_expansion),
        ("pathMappingResolutionMs", timings.path_mapping_resolution),
        ("checkingMs", timings.checking),
        ("diagnosticRenderingMs", timings.diagnostic_rendering),
        ("totalMs", timings.total),
    ] {
        phases.insert(key.to_string(), Value::from(duration_ms(value)));
    }

    let mut memory_json = Map::new();
    memory_json.insert(
        "peakPhysicalBytes".to_string(),
        json_bytes(memory.peak_physical),
    );
    memory_json.insert(
        "finishPhysicalBytes".to_string(),
        json_bytes(memory.finish_physical),
    );
    memory_json.insert("peakRssBytes".to_string(), json_bytes(memory.peak_rss));

    let mut root = Map::new();
    root.insert(
        "schemaVersion".to_string(),
        Value::from(REPORT_SCHEMA_VERSION),
    );
    root.insert("summary".to_string(), Value::Object(summary));
    root.insert("phases".to_string(), Value::Object(phases));
    root.insert("memory".to_string(), Value::Object(memory_json));
    Value::Object(root)
}

// Process-memory probes, sampled once after checking completes. All probes
// return `None` on unsupported platforms or syscall failure — the report must
// state `unavailable`/`null` rather than fabricate a number. macOS reports
// `phys_footprint` (the Activity Monitor / jetsam figure, which keeps counting
// compressed pages that leave the resident set) plus the getrusage peak RSS;
// Linux exposes only the RSS high-water mark, so the footprint fields are
// `None` there.
#[cfg(target_os = "macos")]
mod probe {
    use std::ffi::c_void;
    use std::mem::MaybeUninit;

    // Mirrors <mach/task_info.h> `struct task_vm_info` through the rev3
    // fields. The kernel fills fields up to the boundary implied by the
    // caller's count, so requesting through rev3 yields `phys_footprint`
    // (rev1) and `ledger_phys_footprint_peak` (rev3); older kernels report a
    // smaller filled count, which the per-field offset checks turn into `None`.
    #[repr(C)]
    struct TaskVmInfo {
        virtual_size: u64,
        region_count: i32,
        page_size: i32,
        resident_size: u64,
        resident_size_peak: u64,
        device: u64,
        device_peak: u64,
        internal: u64,
        internal_peak: u64,
        external: u64,
        external_peak: u64,
        reusable: u64,
        reusable_peak: u64,
        purgeable_volatile_pmap: u64,
        purgeable_volatile_resident: u64,
        purgeable_volatile_virtual: u64,
        compressed: u64,
        compressed_peak: u64,
        compressed_lifetime: u64,
        phys_footprint: u64,
        min_address: u64,
        max_address: u64,
        ledger_phys_footprint_peak: i64,
        ledger_tail: [i64; 20],
    }

    #[repr(C)]
    struct Timeval {
        tv_sec: i64,
        tv_usec: i32,
    }

    #[repr(C)]
    struct Rusage {
        ru_utime: Timeval,
        ru_stime: Timeval,
        ru_maxrss: i64,
        ru_tail: [i64; 13],
    }

    const RUSAGE_SELF: i32 = 0;
    const TASK_VM_INFO: u32 = 22;
    const KERN_SUCCESS: i32 = 0;

    unsafe extern "C" {
        fn getrusage(who: i32, usage: *mut Rusage) -> i32;
        static mach_task_self_: u32;
        fn task_info(task: u32, flavor: u32, info: *mut c_void, count: *mut u32) -> i32;
    }

    fn task_vm_info() -> Option<(TaskVmInfo, usize)> {
        let mut info = MaybeUninit::<TaskVmInfo>::zeroed();
        let mut count = (std::mem::size_of::<TaskVmInfo>() / 4) as u32;
        let kr = unsafe {
            task_info(
                mach_task_self_,
                TASK_VM_INFO,
                info.as_mut_ptr().cast(),
                &mut count,
            )
        };
        if kr != KERN_SUCCESS {
            return None;
        }
        Some((unsafe { info.assume_init() }, count as usize * 4))
    }

    pub(super) fn current_footprint_bytes() -> Option<u64> {
        let (info, filled) = task_vm_info()?;
        let end = std::mem::offset_of!(TaskVmInfo, phys_footprint) + 8;
        (filled >= end).then_some(info.phys_footprint)
    }

    pub(super) fn peak_footprint_bytes() -> Option<u64> {
        let (info, filled) = task_vm_info()?;
        let end = std::mem::offset_of!(TaskVmInfo, ledger_phys_footprint_peak) + 8;
        if filled < end {
            return None;
        }
        u64::try_from(info.ledger_phys_footprint_peak).ok()
    }

    pub(super) fn peak_rss_bytes() -> Option<u64> {
        let mut usage = MaybeUninit::<Rusage>::zeroed();
        if unsafe { getrusage(RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
            return None;
        }
        // BSD getrusage reports ru_maxrss in bytes (Linux reports kilobytes).
        u64::try_from(unsafe { usage.assume_init() }.ru_maxrss).ok()
    }
}

#[cfg(target_os = "linux")]
mod probe {
    pub(super) fn current_footprint_bytes() -> Option<u64> {
        None
    }

    pub(super) fn peak_footprint_bytes() -> Option<u64> {
        None
    }

    pub(super) fn peak_rss_bytes() -> Option<u64> {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let line = status
            .lines()
            .find_map(|line| line.strip_prefix("VmHWM:"))?;
        let kb: u64 = line.trim().trim_end_matches("kB").trim().parse().ok()?;
        Some(kb * 1024)
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod probe {
    pub(super) fn current_footprint_bytes() -> Option<u64> {
        None
    }

    pub(super) fn peak_footprint_bytes() -> Option<u64> {
        None
    }

    pub(super) fn peak_rss_bytes() -> Option<u64> {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_lib_and_dependency_classification() {
        let sources: Vec<ProjectSource> = [
            "/repo/src/index.ts",
            "/repo/src/legacy.d.ts",
            "/repo/node_modules/pkg/index.d.ts",
            "/repo/node_modules/typescript/lib/lib.es2024.d.ts",
            "/checker/generated-libs/lib.es2024.full.d.ts",
        ]
        .into_iter()
        .map(|name| (PathBuf::from(name), name.to_string(), String::new()))
        .collect();

        let counts = count_files(&sources);
        assert_eq!(counts.total, 5);
        assert_eq!(counts.source, 2);
        assert_eq!(counts.dependency_declaration, 1);
        assert_eq!(counts.default_lib, 2);
    }

    #[test]
    fn report_json_key_order_is_fixed() {
        let timings = CliTimings::default();
        let memory = MemorySample {
            peak_physical: Some(1),
            finish_physical: None,
            peak_rss: Some(2),
        };
        let json = build_report_json(&FileCounts::default(), 0, 0, &timings, &memory);
        let root: Vec<&String> = json.as_object().unwrap().keys().collect();
        assert_eq!(root, ["schemaVersion", "summary", "phases", "memory"]);
        assert_eq!(json["summary"]["jobs"], Value::String("auto".to_string()));
        assert_eq!(json["memory"]["finishPhysicalBytes"], Value::Null);
    }

    #[test]
    fn bytes_render_human_units() {
        assert_eq!(format_bytes(512), "512 B");
        assert_eq!(format_bytes(2048), "2.0 KiB");
        assert_eq!(format_bytes(3 * 1024 * 1024), "3.0 MiB");
        assert_eq!(format_bytes(5 * 1024 * 1024 * 1024), "5.00 GiB");
        assert_eq!(format_bytes_opt(None), "unavailable");
    }
}
