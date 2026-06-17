#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct DefaultLibSelection {
    pub(crate) include_core: bool,
    pub(crate) include_dom: bool,
}

impl DefaultLibSelection {
    pub(crate) fn none() -> Self {
        Self {
            include_core: false,
            include_dom: false,
        }
    }

    pub(crate) fn default_with_dom() -> Self {
        Self {
            include_core: true,
            include_dom: true,
        }
    }

    pub(crate) fn includes_anything(self) -> bool {
        self.include_core || self.include_dom
    }
}

pub(crate) fn default_lib_selection_from_tsconfig(
    no_lib: bool,
    lib_entries: Option<&[String]>,
) -> DefaultLibSelection {
    if no_lib {
        return DefaultLibSelection::none();
    }

    let Some(lib_entries) = lib_entries else {
        return DefaultLibSelection::default_with_dom();
    };

    if lib_entries.is_empty() {
        return DefaultLibSelection::default_with_dom();
    }

    let mut selection = DefaultLibSelection::none();

    for entry in lib_entries {
        let normalized = entry.trim().to_ascii_lowercase();
        if normalized.starts_with("es") || normalized == "scripthost" {
            selection.include_core = true;
        }

        if normalized.contains("dom") || normalized.contains("webworker") {
            selection.include_dom = true;
            selection.include_core = true;
        }
    }

    if !selection.includes_anything() {
        selection.include_core = true;
    }

    selection
}

pub(crate) fn is_generated_default_lib_file_name(file_name: &str) -> bool {
    let lower = file_name.to_ascii_lowercase();
    lower.contains("/generated-libs/")
        || lower.ends_with(".generated.d.ts")
        || lower.ends_with(".generated.d.mts")
        || lower.ends_with(".generated.d.cts")
}
