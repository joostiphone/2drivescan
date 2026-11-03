//! Reads directory entries together with the metadata the scanner needs.
//!
//! On macOS this uses `getattrlistbulk`, which returns names, types, sizes and
//! modification times for many entries per system call (the fastest way to
//! walk APFS). Elsewhere it uses `read_dir` plus `lstat`, which is cheap on
//! Linux and free on Windows (the metadata comes with the directory listing).

use std::fs::{self, Metadata};
use std::io;
use std::path::Path;
use std::time::UNIX_EPOCH;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EntryKind {
    Dir,
    File,
    Link,
    Other,
}

#[derive(Debug)]
pub(crate) struct RawEntry {
    pub name: Box<str>,
    pub kind: EntryKind,
    /// Bytes allocated on disk.
    pub size: u64,
    /// Device id (Unix); 0 when unknown.
    pub dev: u64,
    #[cfg_attr(not(unix), allow(dead_code))]
    pub ino: u64,
    #[cfg_attr(not(unix), allow(dead_code))]
    pub nlink: u32,
    /// Modification time in nanoseconds since the Unix epoch; 0 when unknown.
    pub mtime: i64,
}

#[derive(Debug, Default)]
pub(crate) struct Listing {
    pub entries: Vec<RawEntry>,
    /// Entries whose metadata could not be read.
    pub errors: u64,
}

#[derive(Debug)]
pub(crate) enum ListError {
    /// No permission, or the folder vanished.
    Unreadable,
    /// A cloud folder (iCloud Drive, OneDrive, …) whose contents are not on
    /// this computer. Listing it would download it, so it is skipped.
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    CloudOnly,
}

fn classify(err: io::Error) -> ListError {
    #[cfg(target_os = "macos")]
    {
        // EDEADLK: the folder is "dataless" and downloads are disabled.
        if err.raw_os_error() == Some(11) {
            return ListError::CloudOnly;
        }
    }
    let _ = err;
    ListError::Unreadable
}

/// Lists a directory.
pub(crate) fn list_dir(path: &Path) -> Result<Listing, ListError> {
    #[cfg(target_os = "macos")]
    {
        match macos::list_dir_bulk(path) {
            Ok(listing) => return Ok(listing),
            Err(macos::BulkError::Io(err)) => return Err(classify(err)),
            Err(macos::BulkError::Unsupported) => {}
        }
    }
    list_dir_std(path)
}

/// Portable implementation, also used as fallback on macOS.
pub(crate) fn list_dir_std(path: &Path) -> Result<Listing, ListError> {
    let entries = fs::read_dir(path).map_err(classify)?;
    let mut out = Listing::default();
    for entry in entries {
        let Ok(entry) = entry else {
            out.errors += 1;
            continue;
        };
        let Ok(ft) = entry.file_type() else {
            out.errors += 1;
            continue;
        };
        let kind = if ft.is_dir() {
            EntryKind::Dir
        } else if ft.is_symlink() {
            EntryKind::Link
        } else if ft.is_file() {
            EntryKind::File
        } else {
            EntryKind::Other
        };
        let mut raw = RawEntry {
            name: entry.file_name().to_string_lossy().into(),
            kind,
            size: 0,
            dev: 0,
            ino: 0,
            nlink: 1,
            mtime: 0,
        };
        match entry.metadata() {
            Ok(md) => fill(&mut raw, &md),
            Err(_) => out.errors += 1,
        }
        out.entries.push(raw);
    }
    Ok(out)
}

/// `lstat` of a single path, in the same shape as a listing entry.
pub(crate) fn stat(path: &Path) -> io::Result<RawEntry> {
    let md = fs::symlink_metadata(path)?;
    let ft = md.file_type();
    let kind = if ft.is_dir() {
        EntryKind::Dir
    } else if ft.is_symlink() {
        EntryKind::Link
    } else if ft.is_file() {
        EntryKind::File
    } else {
        EntryKind::Other
    };
    let mut raw = RawEntry {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().into())
            .unwrap_or_default(),
        kind,
        size: 0,
        dev: 0,
        ino: 0,
        nlink: 1,
        mtime: 0,
    };
    fill(&mut raw, &md);
    // The bulk listing on macOS does not report folder sizes (negligible on
    // APFS); keep both code paths consistent.
    #[cfg(target_os = "macos")]
    if kind == EntryKind::Dir {
        raw.size = 0;
    }
    Ok(raw)
}

fn fill(raw: &mut RawEntry, md: &Metadata) {
    raw.size = disk_size(md);
    raw.mtime = mtime_ns(md);
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        raw.dev = md.dev();
        raw.ino = md.ino();
        raw.nlink = md.nlink().min(u32::MAX as u64) as u32;
    }
}

pub(crate) fn mtime_ns(md: &Metadata) -> i64 {
    md.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

/// Bytes a file actually occupies on disk.
#[cfg(unix)]
pub(crate) fn disk_size(md: &Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    md.blocks().saturating_mul(512)
}

#[cfg(windows)]
pub(crate) fn disk_size(md: &Metadata) -> u64 {
    use std::os::windows::fs::MetadataExt;
    const FILE_ATTRIBUTE_OFFLINE: u32 = 0x1000;
    const FILE_ATTRIBUTE_RECALL_ON_OPEN: u32 = 0x40000;
    const FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS: u32 = 0x400000;
    // Cloud placeholders (OneDrive, iCloud for Windows, ...) report their
    // full size but take no space locally.
    let attrs = md.file_attributes();
    if attrs
        & (FILE_ATTRIBUTE_OFFLINE
            | FILE_ATTRIBUTE_RECALL_ON_OPEN
            | FILE_ATTRIBUTE_RECALL_ON_DATA_ACCESS)
        != 0
    {
        return 0;
    }
    md.len()
}

#[cfg(not(any(unix, windows)))]
pub(crate) fn disk_size(md: &Metadata) -> u64 {
    md.len()
}

/// Stops the scanner from downloading cloud files and folders.
///
/// On macOS, merely listing an iCloud Drive or File Provider folder that is
/// only in the cloud makes the system download it first, which turns a scan
/// into a very slow network crawl. With this policy such folders fail fast
/// instead and are skipped; files that are not downloaded take no disk space
/// anyway. It applies to this process only, so opening files from the app in
/// other apps is unaffected.
pub fn disable_cloud_downloads() {
    #[cfg(target_os = "macos")]
    macos::disable_materialization();
}

#[cfg(target_os = "macos")]
mod macos {
    use super::{EntryKind, Listing, RawEntry};
    use std::cell::RefCell;
    use std::ffi::c_void;
    use std::fs::File;
    use std::io;
    use std::os::raw::c_int;
    use std::os::unix::io::AsRawFd;
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

    const ATTR_BIT_MAP_COUNT: u16 = 5;
    const ATTR_CMN_NAME: u32 = 0x0000_0001;
    const ATTR_CMN_DEVID: u32 = 0x0000_0002;
    const ATTR_CMN_OBJTYPE: u32 = 0x0000_0008;
    const ATTR_CMN_MODTIME: u32 = 0x0000_0400;
    const ATTR_CMN_FILEID: u32 = 0x0200_0000;
    const ATTR_CMN_RETURNED_ATTRS: u32 = 0x8000_0000;
    const ATTR_FILE_LINKCOUNT: u32 = 0x0000_0001;
    const ATTR_FILE_ALLOCSIZE: u32 = 0x0000_0004;
    const FSOPT_PACK_INVAL_ATTRS: u64 = 0x0000_0008;

    const VREG: u32 = 1;
    const VDIR: u32 = 2;
    const VLNK: u32 = 5;

    // <sys/resource.h>
    const IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES: c_int = 3;
    const IOPOL_SCOPE_PROCESS: c_int = 0;
    const IOPOL_MATERIALIZE_DATALESS_FILES_OFF: c_int = 1;

    // Entry layout with FSOPT_PACK_INVAL_ATTRS: every requested attribute is
    // present, in bit order, each aligned to 4 bytes.
    const OFF_RETURNED: usize = 4; // attribute_set_t (5 × u32)
    const OFF_NAME: usize = 24; // attrreference_t { i32 offset, u32 length }
    const OFF_DEVID: usize = 32; // dev_t (i32)
    const OFF_OBJTYPE: usize = 36; // fsobj_type_t (u32)
    const OFF_MODTIME: usize = 40; // struct timespec (i64, i64)
    const OFF_FILEID: usize = 56; // u64
    const OFF_LINKCOUNT: usize = 64; // u32
    const OFF_ALLOCSIZE: usize = 68; // off_t (i64)
    const FIXED_LEN: usize = 76;

    extern "C" {
        fn getattrlistbulk(
            dirfd: c_int,
            attr_list: *mut c_void,
            attr_buf: *mut c_void,
            attr_buf_size: usize,
            options: u64,
        ) -> c_int;
        fn setiopolicy_np(iotype: c_int, scope: c_int, policy: c_int) -> c_int;
    }

    pub(super) fn disable_materialization() {
        // SAFETY: plain system call without pointers.
        unsafe {
            setiopolicy_np(
                IOPOL_TYPE_VFS_MATERIALIZE_DATALESS_FILES,
                IOPOL_SCOPE_PROCESS,
                IOPOL_MATERIALIZE_DATALESS_FILES_OFF,
            );
        }
    }

    pub(super) enum BulkError {
        /// The file system does not support bulk listing: use read_dir.
        Unsupported,
        Io(io::Error),
    }

    thread_local! {
        static BUF: RefCell<Vec<u8>> = RefCell::new(vec![0u8; 256 * 1024]);
    }

    fn u32_at(b: &[u8], off: usize) -> u32 {
        u32::from_ne_bytes(b[off..off + 4].try_into().expect("4 bytes"))
    }

    fn i32_at(b: &[u8], off: usize) -> i32 {
        i32::from_ne_bytes(b[off..off + 4].try_into().expect("4 bytes"))
    }

    fn u64_at(b: &[u8], off: usize) -> u64 {
        u64::from_ne_bytes(b[off..off + 8].try_into().expect("8 bytes"))
    }

    fn i64_at(b: &[u8], off: usize) -> i64 {
        i64::from_ne_bytes(b[off..off + 8].try_into().expect("8 bytes"))
    }

    fn invalid(msg: &str) -> BulkError {
        BulkError::Io(io::Error::new(io::ErrorKind::InvalidData, msg.to_string()))
    }

    pub(super) fn list_dir_bulk(path: &Path) -> Result<Listing, BulkError> {
        let dir = File::open(path).map_err(BulkError::Io)?;
        let fd = dir.as_raw_fd();
        let mut attrs = AttrList {
            bitmapcount: ATTR_BIT_MAP_COUNT,
            reserved: 0,
            commonattr: ATTR_CMN_RETURNED_ATTRS
                | ATTR_CMN_NAME
                | ATTR_CMN_DEVID
                | ATTR_CMN_OBJTYPE
                | ATTR_CMN_MODTIME
                | ATTR_CMN_FILEID,
            volattr: 0,
            dirattr: 0,
            fileattr: ATTR_FILE_LINKCOUNT | ATTR_FILE_ALLOCSIZE,
            forkattr: 0,
        };
        let mut out = Listing::default();
        BUF.with(|cell| {
            let mut buf = cell.borrow_mut();
            loop {
                // SAFETY: `attrs` and `buf` outlive the call; the kernel writes
                // at most `buf.len()` bytes.
                let count = unsafe {
                    getattrlistbulk(
                        fd,
                        (&mut attrs as *mut AttrList).cast(),
                        buf.as_mut_ptr().cast(),
                        buf.len(),
                        FSOPT_PACK_INVAL_ATTRS,
                    )
                };
                if count < 0 {
                    let err = io::Error::last_os_error();
                    // ENOTSUP, EINVAL, ENOSYS, EOPNOTSUPP
                    let unsupported = matches!(err.raw_os_error(), Some(45 | 22 | 78 | 102));
                    return Err(if unsupported && out.entries.is_empty() {
                        BulkError::Unsupported
                    } else {
                        BulkError::Io(err)
                    });
                }
                if count == 0 {
                    return Ok(());
                }
                let mut off = 0usize;
                for _ in 0..count {
                    if off + 4 > buf.len() {
                        return Err(invalid("truncated getattrlistbulk buffer"));
                    }
                    let len = u32_at(&buf, off) as usize;
                    if len < FIXED_LEN || off + len > buf.len() {
                        return Err(invalid("malformed getattrlistbulk entry"));
                    }
                    parse_entry(&buf[off..off + len], path, &mut out);
                    off += len;
                }
            }
        })?;
        Ok(out)
    }

    fn parse_entry(e: &[u8], dir: &Path, out: &mut Listing) {
        let returned_common = u32_at(e, OFF_RETURNED);
        let returned_file = u32_at(e, OFF_RETURNED + 12);

        if returned_common & ATTR_CMN_NAME == 0 {
            out.errors += 1;
            return;
        }
        let name_ref = OFF_NAME as i64 + i32_at(e, OFF_NAME) as i64;
        let name_len = u32_at(e, OFF_NAME + 4) as usize;
        if name_ref < 0 || name_len == 0 || name_ref as usize + name_len > e.len() {
            out.errors += 1;
            return;
        }
        let start = name_ref as usize;
        // The length includes the terminating NUL.
        let name_bytes = &e[start..start + name_len - 1];
        let name: Box<str> = String::from_utf8_lossy(name_bytes).into();

        let kind = match u32_at(e, OFF_OBJTYPE) {
            VDIR => EntryKind::Dir,
            VREG => EntryKind::File,
            VLNK => EntryKind::Link,
            _ => EntryKind::Other,
        };
        let mtime = if returned_common & ATTR_CMN_MODTIME != 0 {
            i64_at(e, OFF_MODTIME)
                .saturating_mul(1_000_000_000)
                .saturating_add(i64_at(e, OFF_MODTIME + 8))
        } else {
            0
        };
        // Same conversion as std's MetadataExt::dev() (dev_t is i32).
        let dev = i32_at(e, OFF_DEVID) as u64;
        let ino = u64_at(e, OFF_FILEID);

        let (size, nlink) = if kind == EntryKind::Dir {
            (0, 1)
        } else if returned_file & ATTR_FILE_ALLOCSIZE != 0 {
            let nlink = if returned_file & ATTR_FILE_LINKCOUNT != 0 {
                u32_at(e, OFF_LINKCOUNT)
            } else {
                1
            };
            (i64_at(e, OFF_ALLOCSIZE).max(0) as u64, nlink)
        } else {
            // Rare: the file system did not report a size; ask for it.
            match super::stat(&dir.join(&*name)) {
                Ok(raw) => (raw.size, raw.nlink),
                Err(_) => {
                    out.errors += 1;
                    (0, 1)
                }
            }
        };

        out.entries.push(RawEntry {
            name,
            kind,
            size,
            dev,
            ino,
            nlink,
            mtime,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn listing_reports_kinds_sizes_and_times() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("sub")).unwrap();
        let mut f = fs::File::create(dir.path().join("data.bin")).unwrap();
        f.write_all(&vec![1u8; 100_000]).unwrap();
        f.sync_all().unwrap();
        #[cfg(unix)]
        std::os::unix::fs::symlink("data.bin", dir.path().join("link")).unwrap();

        let listing = list_dir(dir.path()).unwrap();
        let find = |n: &str| listing.entries.iter().find(|e| &*e.name == n).unwrap();
        assert_eq!(find("sub").kind, EntryKind::Dir);
        let data = find("data.bin");
        assert_eq!(data.kind, EntryKind::File);
        assert!(data.size >= 100_000, "size {}", data.size);
        assert!(data.mtime > 0);
        #[cfg(unix)]
        assert_eq!(find("link").kind, EntryKind::Link);
    }

    /// The fast macOS listing must agree with the portable one.
    #[cfg(target_os = "macos")]
    #[test]
    fn bulk_listing_matches_read_dir() {
        let dir = tempfile::tempdir().unwrap();
        fs::create_dir(dir.path().join("folder")).unwrap();
        for (i, len) in [0usize, 1, 4096, 300_000].iter().enumerate() {
            let mut f = fs::File::create(dir.path().join(format!("file-{i} ünïcode.txt"))).unwrap();
            f.write_all(&vec![7u8; *len]).unwrap();
            f.sync_all().unwrap();
        }
        std::os::unix::fs::symlink("folder", dir.path().join("link")).unwrap();
        fs::hard_link(
            dir.path().join("file-3 ünïcode.txt"),
            dir.path().join("hard"),
        )
        .unwrap();

        let mut bulk = match macos::list_dir_bulk(dir.path()) {
            Ok(l) => l.entries,
            Err(_) => panic!("getattrlistbulk failed"),
        };
        let mut std = list_dir_std(dir.path()).unwrap().entries;
        bulk.sort_by(|a, b| a.name.cmp(&b.name));
        std.sort_by(|a, b| a.name.cmp(&b.name));
        assert_eq!(bulk.len(), std.len());
        for (b, s) in bulk.iter().zip(std.iter()) {
            assert_eq!(b.name, s.name);
            assert_eq!(b.kind, s.kind, "{}", b.name);
            assert_eq!(b.dev, s.dev, "{}", b.name);
            assert_eq!(b.ino, s.ino, "{}", b.name);
            assert_eq!(b.mtime, s.mtime, "{}", b.name);
            if b.kind == EntryKind::File {
                assert_eq!(b.size, s.size, "{}", b.name);
                assert_eq!(b.nlink, s.nlink, "{}", b.name);
            }
        }
    }
}
