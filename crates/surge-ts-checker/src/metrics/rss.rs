//! Process RSS sampling for per-stage memory profiling.
//!
//! `current_rss_bytes` reads the resident set size right now;
//! `peak_rss_bytes` reads the OS-tracked high-water mark, which catches spikes
//! that fall between two stage samples. All probes return `None` on
//! unsupported platforms or syscall failure — sampling must never fail the
//! check.
//!
//! On macOS RSS undercounts under memory pressure: compressed/swapped pages
//! leave the resident set but the process still owns them. `phys_footprint`
//! (what Activity Monitor's "Memory" column and jetsam use) keeps counting
//! them, so `current_footprint_bytes`/`peak_footprint_bytes` report the real
//! memory demand; elsewhere they return `None`.

pub(crate) use imp::{
    current_footprint_bytes, current_rss_bytes, peak_footprint_bytes, peak_rss_bytes,
};

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

    // Mirrors <mach/task_info.h> `struct task_vm_info` through the rev3
    // fields. The kernel fills fields up to the revision boundary implied by
    // the caller's count, so requesting exactly through rev3 yields
    // `phys_footprint` (rev1) and `ledger_phys_footprint_peak` (rev3) on any
    // supported macOS; older kernels report a smaller filled count, which the
    // per-field offset checks below turn into `None`.
    #[repr(C)]
    struct TaskVmInfo {
        virtual_size: u64,
        region_count: i32,
        page_size: i32,
        resident_size: u64,
        resident_size_peak: u64,
        device: u64,
        device_peak: u64,
        internal: u64,
        internal_peak: u64,
        external: u64,
        external_peak: u64,
        reusable: u64,
        reusable_peak: u64,
        purgeable_volatile_pmap: u64,
        purgeable_volatile_resident: u64,
        purgeable_volatile_virtual: u64,
        compressed: u64,
        compressed_peak: u64,
        compressed_lifetime: u64,
        phys_footprint: u64,
        min_address: u64,
        max_address: u64,
        ledger_phys_footprint_peak: i64,
        ledger_tail: [i64; 20],
    }

    const PROC_PIDTASKINFO: i32 = 4;
    const RUSAGE_SELF: i32 = 0;
    const TASK_VM_INFO: u32 = 22;
    const KERN_SUCCESS: i32 = 0;

    unsafe extern "C" {
        fn proc_pidinfo(
            pid: i32,
            flavor: i32,
            arg: u64,
            buffer: *mut c_void,
            buffersize: i32,
        ) -> i32;
        fn getrusage(who: i32, usage: *mut Rusage) -> i32;
        static mach_task_self_: u32;
        fn task_info(task: u32, flavor: u32, info: *mut c_void, count: *mut u32) -> i32;
    }

    /// Returns the task_vm_info struct plus the count of 4-byte words the
    /// kernel actually filled.
    fn task_vm_info() -> Option<(TaskVmInfo, usize)> {
        let mut info = MaybeUninit::<TaskVmInfo>::zeroed();
        let mut count = (std::mem::size_of::<TaskVmInfo>() / 4) as u32;
        let kr = unsafe {
            task_info(
                mach_task_self_,
                TASK_VM_INFO,
                info.as_mut_ptr().cast(),
                &mut count,
            )
        };
        if kr != KERN_SUCCESS {
            return None;
        }
        Some((unsafe { info.assume_init() }, count as usize * 4))
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

    pub(crate) fn current_footprint_bytes() -> Option<u64> {
        let (info, filled) = task_vm_info()?;
        let end = std::mem::offset_of!(TaskVmInfo, phys_footprint) + 8;
        (filled >= end).then_some(info.phys_footprint)
    }

    pub(crate) fn peak_footprint_bytes() -> Option<u64> {
        let (info, filled) = task_vm_info()?;
        let end = std::mem::offset_of!(TaskVmInfo, ledger_phys_footprint_peak) + 8;
        if filled < end {
            return None;
        }
        u64::try_from(info.ledger_phys_footprint_peak).ok()
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

    pub(crate) fn current_footprint_bytes() -> Option<u64> {
        None
    }

    pub(crate) fn peak_footprint_bytes() -> Option<u64> {
        None
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

    pub(crate) fn current_footprint_bytes() -> Option<u64> {
        None
    }

    pub(crate) fn peak_footprint_bytes() -> Option<u64> {
        None
    }
}
