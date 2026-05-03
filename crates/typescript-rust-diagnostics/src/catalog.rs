use crate::{DIAGNOSTIC_CATALOG, DiagnosticDescriptor, DiagnosticSupport};

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
