use crate::SourceFileInput;

use super::registry::selected_default_lib_sources;
use super::source::default_lib_selection_from_tsconfig;

pub fn load_default_lib_inputs(
    no_lib: bool,
    lib_entries: Option<&[String]>,
) -> Vec<SourceFileInput> {
    let selection = default_lib_selection_from_tsconfig(no_lib, lib_entries);
    selected_default_lib_sources(selection)
        .into_iter()
        .map(|source| SourceFileInput {
            file_name: source.file_name.to_string_lossy().into_owned(),
            source_text: source.source_text,
        })
        .collect()
}
