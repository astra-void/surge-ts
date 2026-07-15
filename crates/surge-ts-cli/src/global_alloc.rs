//! Global allocator selection for the `surge` binary.
//!
//! The system allocator is the default. Exactly one of the `mimalloc`,
//! `jemalloc`, or `snmalloc` cargo features may be enabled to swap the
//! process-wide allocator; enabling more than one is a compile-time error, as
//! is requesting an allocator on a platform it does not support. There is
//! deliberately no runtime fallback: a build either uses the allocator it was
//! asked for or fails to compile.

#[cfg(all(feature = "mimalloc", feature = "jemalloc"))]
compile_error!(
    "allocator features are mutually exclusive: `mimalloc` and `jemalloc` are both enabled; \
     enable at most one of `mimalloc`, `jemalloc`, `snmalloc`"
);

#[cfg(all(feature = "mimalloc", feature = "snmalloc"))]
compile_error!(
    "allocator features are mutually exclusive: `mimalloc` and `snmalloc` are both enabled; \
     enable at most one of `mimalloc`, `jemalloc`, `snmalloc`"
);

#[cfg(all(feature = "jemalloc", feature = "snmalloc"))]
compile_error!(
    "allocator features are mutually exclusive: `jemalloc` and `snmalloc` are both enabled; \
     enable at most one of `mimalloc`, `jemalloc`, `snmalloc`"
);

#[cfg(all(feature = "jemalloc", target_env = "msvc"))]
compile_error!(
    "the `jemalloc` allocator feature is not supported on MSVC targets; \
     build with `mimalloc` or the default system allocator instead"
);

#[cfg(feature = "mimalloc")]
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

#[cfg(all(feature = "jemalloc", not(target_env = "msvc")))]
#[global_allocator]
static GLOBAL: tikv_jemallocator::Jemalloc = tikv_jemallocator::Jemalloc;

#[cfg(feature = "snmalloc")]
#[global_allocator]
static GLOBAL: snmalloc_rs::SnMalloc = snmalloc_rs::SnMalloc;

/// Which global allocator this binary was compiled with. Reported when the
/// `SURGE_PRINT_ALLOCATOR` environment variable is set so benchmark harnesses
/// can verify they are measuring the binary they think they are.
pub(crate) const ACTIVE_ALLOCATOR: &str = if cfg!(feature = "mimalloc") {
    "mimalloc"
} else if cfg!(feature = "jemalloc") {
    "jemalloc"
} else if cfg!(feature = "snmalloc") {
    "snmalloc"
} else {
    "system"
};
