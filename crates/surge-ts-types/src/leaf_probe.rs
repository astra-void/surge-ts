//! Single-syscall directory-entry probe backing the canonicalization caches.
//!
//! `std::fs::canonicalize` walks every path component (one `getattrlist` per
//! component inside `realpath(3)`), which the loader and checker pay once per
//! unique path spelling. When a path's parent directory is already canonical,
//! one `getattrlist` on the joined path answers everything `realpath` would:
//! whether the entry exists, whether it is a symlink (which needs the full
//! walk after all), and the entry's on-disk name — the case-normalized form
//! `realpath` reports on APFS's default case-insensitive volumes. This module
//! lives in `surge-ts-types` only because it is the one crate both the config
//! loader and the checker depend on.

#[cfg(target_os = "macos")]
pub use macos::probe_leaf;

use std::ffi::OsString;

/// Outcome of probing one directory entry without following symlinks.
#[derive(Debug)]
pub enum LeafProbe {
    /// The entry exists. `name` is its on-disk (case-corrected) name; a
    /// symlink still needs a full `realpath` walk to resolve its target.
    Entry { name: OsString, is_symlink: bool },
    /// The entry definitely does not exist (`ENOENT`/`ENOTDIR`).
    Missing,
    /// The probe could not answer (unsupported platform or filesystem,
    /// permission errors, oversized names). Callers must fall back to
    /// `std::fs::canonicalize` so behavior stays identical to the unprobed
    /// path.
    Unsupported,
}

#[cfg(not(target_os = "macos"))]
pub fn probe_leaf(_path: &std::path::Path) -> LeafProbe {
    LeafProbe::Unsupported
}

#[cfg(target_os = "macos")]
mod macos {
    use super::LeafProbe;
    use std::ffi::{CString, OsString};
    use std::os::unix::ffi::{OsStrExt, OsStringExt};
    use std::path::Path;

    #[repr(C)]
    struct AttrList {
        bitmapcount: u16,
        reserved: u16,
        commonattr: u32,
        volattr: u32,
        dirattr: u32,
        fileattr: u32,
        forkattr: u32,
    }

    unsafe extern "C" {
        fn getattrlist(
            path: *const std::ffi::c_char,
            attr_list: *mut AttrList,
            attr_buf: *mut u8,
            attr_buf_size: usize,
            options: usize,
        ) -> i32;
    }

    const ATTR_BIT_MAP_COUNT: u16 = 5;
    const ATTR_CMN_NAME: u32 = 0x0000_0001;
    const ATTR_CMN_OBJTYPE: u32 = 0x0000_0008;
    const ATTR_CMN_RETURNED_ATTRS: u32 = 0x8000_0000;
    const FSOPT_NOFOLLOW: usize = 0x1;
    const VLNK: u32 = 5;
    const ENOENT: i32 = 2;
    const ENOTDIR: i32 = 20;

    // u32 buffer length + attribute_set_t (5 u32s) + attrreference_t
    // (i32 offset, u32 length) + fsobj_type_t (u32), then the name bytes
    // (up to NAME_MAX = 255 UTF-8 bytes plus NUL), all 4-byte aligned.
    const ATTR_BUF_LEN: usize = 4 + 20 + 8 + 4 + 1024;

    pub fn probe_leaf(path: &Path) -> LeafProbe {
        // A path with an interior NUL cannot exist on disk.
        let Ok(c_path) = CString::new(path.as_os_str().as_bytes()) else {
            return LeafProbe::Missing;
        };
        let mut attr_list = AttrList {
            bitmapcount: ATTR_BIT_MAP_COUNT,
            reserved: 0,
            commonattr: ATTR_CMN_RETURNED_ATTRS | ATTR_CMN_NAME | ATTR_CMN_OBJTYPE,
            volattr: 0,
            dirattr: 0,
            fileattr: 0,
            forkattr: 0,
        };
        let mut buf = [0u8; ATTR_BUF_LEN];
        let rc = unsafe {
            getattrlist(
                c_path.as_ptr(),
                &mut attr_list,
                buf.as_mut_ptr(),
                buf.len(),
                FSOPT_NOFOLLOW,
            )
        };
        if rc != 0 {
            return match std::io::Error::last_os_error().raw_os_error() {
                Some(ENOENT) | Some(ENOTDIR) => LeafProbe::Missing,
                _ => LeafProbe::Unsupported,
            };
        }

        let read_u32 =
            |offset: usize| u32::from_ne_bytes(buf[offset..offset + 4].try_into().unwrap());
        let total_len = read_u32(0) as usize;
        if total_len > buf.len() {
            return LeafProbe::Unsupported;
        }
        // attribute_set_t.commonattr echoes which attributes were actually
        // packed; a filesystem may silently omit some.
        let returned_common = read_u32(4);
        if returned_common & (ATTR_CMN_NAME | ATTR_CMN_OBJTYPE)
            != (ATTR_CMN_NAME | ATTR_CMN_OBJTYPE)
        {
            return LeafProbe::Unsupported;
        }

        let name_ref_offset = 4 + 20;
        let name_data_offset = read_u32(name_ref_offset) as i32 as isize;
        let name_len = read_u32(name_ref_offset + 4) as usize;
        let obj_type = read_u32(name_ref_offset + 8);

        let name_start = name_ref_offset as isize + name_data_offset;
        if name_start < 0 || name_len == 0 {
            return LeafProbe::Unsupported;
        }
        let name_start = name_start as usize;
        let name_end = name_start + name_len;
        if name_end > total_len {
            return LeafProbe::Unsupported;
        }
        // attrreference length includes the terminating NUL.
        let name_bytes = &buf[name_start..name_end - 1];
        if name_bytes.is_empty() || name_bytes.contains(&0) {
            return LeafProbe::Unsupported;
        }

        LeafProbe::Entry {
            name: OsString::from_vec(name_bytes.to_vec()),
            is_symlink: obj_type == VLNK,
        }
    }
}
