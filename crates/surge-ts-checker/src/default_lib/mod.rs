mod loader;
mod physical;
mod registry;
mod source;

pub use loader::{
    DefaultLibLoad, DefaultLibRequest, load_default_lib_inputs, load_generated_default_lib_inputs,
};
pub(crate) use physical::is_physical_default_lib_file_name;
pub use physical::{
    DefaultLibIoStats, PhysicalLibResolution, default_full_lib_seed_for_target,
    resolve_physical_default_libs,
};
pub(crate) use source::is_generated_default_lib_file_name;
