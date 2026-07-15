//! Process RSS sampling for per-stage memory profiling.
//!
//! `current_rss_bytes` reads the resident set size right now;
//! `peak_rss_bytes` reads the OS-tracked high-water mark, which catches spikes
//! that fall between two stage samples. Both return `None` on unsupported
//! platforms or syscall failure — sampling must never fail the check.

pub(crate) use imp::{current_rss_bytes, peak_rss_bytes};

#[cfg(target_os = "macos")]
mod imp {
    use std::ffi::c_void;
    use std::mem::MaybeUninit;

    // Layouts mirror <sys/proc_info.h> / <sys/resource.h>; only the leading
    // fields are consumed, but the full structs are declared so the kernel
    // writes into correctly sized buffers.
    #[repr(C)]
    struct ProcTaskInfo {
        pti_virtual_size: u64,
        pti_resident_size: u64,
        pti_total_user: u64,
        pti_total_system: u64,
        pti_threads_user: u64,
        pti_threads_system: u64,
        pti_policy: i32,
        pti_faults: i32,
        pti_pageins: i32,
        pti_cow_faults: i32,
        pti_messages_sent: i32,
        pti_messages_received: i32,
        pti_syscalls_mach: i32,
        pti_syscalls_unix: i32,
        pti_csw: i32,
        pti_threadnum: i32,
        pti_numrunning: i32,
        pti_priority: i32,
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
        ru_ixrss: i64,
        ru_idrss: i64,
        ru_isrss: i64,
        ru_minflt: i64,
        ru_majflt: i64,
        ru_nswap: i64,
        ru_inblock: i64,
        ru_oublock: i64,
        ru_msgsnd: i64,
        ru_msgrcv: i64,
        ru_nsignals: i64,
        ru_nvcsw: i64,
        ru_nivcsw: i64,
    }

    const PROC_PIDTASKINFO: i32 = 4;
    const RUSAGE_SELF: i32 = 0;

    extern "C" {
        fn proc_pidinfo(
            pid: i32,
            flavor: i32,
            arg: u64,
            buffer: *mut c_void,
            buffersize: i32,
        ) -> i32;
        fn getrusage(who: i32, usage: *mut Rusage) -> i32;
    }

    pub(crate) fn current_rss_bytes() -> Option<u64> {
        let mut info = MaybeUninit::<ProcTaskInfo>::uninit();
        let size = std::mem::size_of::<ProcTaskInfo>() as i32;
        let written = unsafe {
            proc_pidinfo(
                std::process::id() as i32,
                PROC_PIDTASKINFO,
                0,
                info.as_mut_ptr().cast(),
                size,
            )
        };
        if written != size {
            return None;
        }
        Some(unsafe { info.assume_init() }.pti_resident_size)
    }

    pub(crate) fn peak_rss_bytes() -> Option<u64> {
        let mut usage = MaybeUninit::<Rusage>::zeroed();
        if unsafe { getrusage(RUSAGE_SELF, usage.as_mut_ptr()) } != 0 {
            return None;
        }
        // BSD getrusage reports ru_maxrss in bytes (Linux reports kilobytes).
        u64::try_from(unsafe { usage.assume_init() }.ru_maxrss).ok()
    }
}

#[cfg(target_os = "linux")]
mod imp {
    fn status_field_bytes(field: &str) -> Option<u64> {
        let status = std::fs::read_to_string("/proc/self/status").ok()?;
        let line = status.lines().find_map(|line| line.strip_prefix(field))?;
        let kb: u64 = line.trim().trim_end_matches("kB").trim().parse().ok()?;
        Some(kb * 1024)
    }

    pub(crate) fn current_rss_bytes() -> Option<u64> {
        status_field_bytes("VmRSS:")
    }

    pub(crate) fn peak_rss_bytes() -> Option<u64> {
        status_field_bytes("VmHWM:")
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
mod imp {
    pub(crate) fn current_rss_bytes() -> Option<u64> {
        None
    }

    pub(crate) fn peak_rss_bytes() -> Option<u64> {
        None
    }
}
