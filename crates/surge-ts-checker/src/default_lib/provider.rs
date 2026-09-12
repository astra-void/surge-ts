use std::fs;
use std::path::PathBuf;
use std::time::Instant;

use super::embedded::{embedded_lib_file_name, embedded_lib_source};
use super::physical::DefaultLibIoStats;

/// Where default-lib declaration text comes from.
///
/// The reference-graph loader is written against this trait so the embedded
/// snapshot and an on-disk `lib/` directory share one graph walk, one name
/// normalization, and one parse path. Callers pick a provider; nothing below
/// this abstraction knows which one is in use.
pub trait LibSource {
    /// Stable identity for a normalized lib name, used as the checker's file
    /// name for the loaded source.
    fn file_name(&self, normalized_name: &str) -> String;

    /// Whether this source can supply the named lib.
    fn contains(&self, normalized_name: &str, stats: &mut DefaultLibIoStats) -> bool;

    /// The declaration text, plus the identity to dedupe the graph by.
    fn load(
        &self,
        normalized_name: &str,
        stats: &mut DefaultLibIoStats,
    ) -> Option<(String, String)>;

    /// Human-readable description for diagnostics and debug output.
    fn describe(&self) -> String;
}

/// Serves the version-pinned snapshot compiled into the binary.
pub struct EmbeddedLibSource;

impl LibSource for EmbeddedLibSource {
    fn file_name(&self, normalized_name: &str) -> String {
        embedded_lib_file_name(normalized_name)
    }

    fn contains(&self, normalized_name: &str, _stats: &mut DefaultLibIoStats) -> bool {
        embedded_lib_source(normalized_name).is_some()
    }

    fn load(
        &self,
        normalized_name: &str,
        stats: &mut DefaultLibIoStats,
    ) -> Option<(String, String)> {
        let source = embedded_lib_source(normalized_name)?;
        // No I/O happens, but the byte count still describes the loaded graph,
        // so the timings block stays comparable against the filesystem path.
        stats.files_read += 1;
        stats.bytes_read += source.len() as u64;
        Some((self.file_name(normalized_name), source.to_string()))
    }

    fn describe(&self) -> String {
        format!(
            "bundled TypeScript {} libs",
            super::embedded::bundled_typescript_version()
        )
    }
}

/// Serves `lib.*.d.ts` from a real directory, for an explicit user override.
pub struct DirectoryLibSource {
    pub lib_dir: PathBuf,
}

impl DirectoryLibSource {
    pub fn new(lib_dir: PathBuf) -> Self {
        Self { lib_dir }
    }

    fn path(&self, normalized_name: &str) -> PathBuf {
        self.lib_dir.join(format!("lib.{normalized_name}.d.ts"))
    }
}

impl LibSource for DirectoryLibSource {
    fn file_name(&self, normalized_name: &str) -> String {
        self.path(normalized_name).to_string_lossy().into_owned()
    }

    fn contains(&self, normalized_name: &str, stats: &mut DefaultLibIoStats) -> bool {
        stats.existence_probes += 1;
        self.path(normalized_name).is_file()
    }

    fn load(
        &self,
        normalized_name: &str,
        stats: &mut DefaultLibIoStats,
    ) -> Option<(String, String)> {
        let path = self.path(normalized_name);
        stats.canonicalize_syscalls += 1;
        let canonical = fs::canonicalize(&path).unwrap_or_else(|_| path.clone());

        let read_start = Instant::now();
        let source_text = fs::read_to_string(&path).ok()?;
        stats.read_io += read_start.elapsed();
        stats.files_read += 1;
        stats.bytes_read += source_text.len() as u64;

        Some((canonical.to_string_lossy().into_owned(), source_text))
    }

    fn describe(&self) -> String {
        format!("TypeScript libs at {}", self.lib_dir.display())
    }
}
