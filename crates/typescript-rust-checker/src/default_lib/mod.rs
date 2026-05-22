mod loader;
mod registry;
mod source;

pub use loader::load_default_lib_inputs;
pub(crate) use source::is_generated_default_lib_file_name;
