mod generated_tables;
mod loader;
mod physical;
mod registry;
mod snapshot;
mod source;

pub use loader::load_default_lib_inputs;
pub(crate) use physical::is_physical_default_lib_file_name;
pub use physical::{
    PhysicalLibResolution, default_full_lib_seed_for_target, resolve_physical_default_libs,
};
pub(crate) use snapshot::inject_generated_default_lib_snapshot_for_file_name;
pub(crate) use source::is_generated_default_lib_file_name;
