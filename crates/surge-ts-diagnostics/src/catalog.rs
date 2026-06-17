use crate::{DIAGNOSTIC_CATALOG, DiagnosticDescriptor, DiagnosticSource, DiagnosticSupport};

#[derive(Debug, Clone, Copy, Default)]
pub struct DiagnosticCoverageStats {
    pub catalog_total: usize,
    pub emitted_total: usize,
    pub catalog_only_total: usize,
    pub emitted_typescript_total: usize,
    pub catalog_only_typescript_total: usize,
}

pub fn catalog_coverage_stats() -> DiagnosticCoverageStats {
    let mut stats = DiagnosticCoverageStats::default();

    for descriptor in DIAGNOSTIC_CATALOG {
        stats.catalog_total += 1;

        let is_ts = descriptor.source == DiagnosticSource::TypeScript;

        match descriptor.support {
            DiagnosticSupport::Emitted => {
                stats.emitted_total += 1;
                if is_ts {
                    stats.emitted_typescript_total += 1;
                }
            }
            DiagnosticSupport::CatalogOnly => {
                stats.catalog_only_total += 1;
                if is_ts {
                    stats.catalog_only_typescript_total += 1;
                }
            }
        }
    }

    stats
}

pub fn cataloged_diagnostic_descriptors() -> &'static [DiagnosticDescriptor] {
    DIAGNOSTIC_CATALOG
}

pub fn cataloged_typescript_diagnostics() -> &'static [DiagnosticDescriptor] {
    cataloged_diagnostic_descriptors()
}

pub fn emitted_diagnostic_descriptors() -> impl Iterator<Item = &'static DiagnosticDescriptor> {
    DIAGNOSTIC_CATALOG
        .iter()
        .filter(|descriptor| descriptor.support == DiagnosticSupport::Emitted)
}

pub fn emitted_typescript_diagnostics() -> impl Iterator<Item = &'static DiagnosticDescriptor> {
    emitted_diagnostic_descriptors()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_contains_ts5112() {
        let descriptor = crate::TS5112;
        assert_eq!(descriptor.code, "TS5112");
        assert_eq!(descriptor.number, Some(5112));
        assert_eq!(descriptor.argument_count, 0);
        assert_eq!(descriptor.support, DiagnosticSupport::Emitted);
    }
}
