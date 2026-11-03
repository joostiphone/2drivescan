//! Discovers mounted volumes and decides which parts of the file system a
//! scan should (not) descend into.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use scan_core::ScanOptions;
use serde::Serialize;
use sysinfo::{DiskKind, Disks};

#[derive(Clone, Debug, Serialize)]
pub struct Volume {
    pub name: String,
    pub mount_point: String,
    pub total: u64,
    pub available: u64,
    pub file_system: String,
    pub removable: bool,
    pub kind: &'static str,
    pub is_root: bool,
}

/// What to scan and how.
pub struct Plan {
    pub opts: ScanOptions,
    /// Set when the scan root is the mount point of a volume; used to show
    /// "hidden space" (used space not attributable to any file).
    pub volume: Option<Volume>,
}

#[cfg(target_os = "linux")]
const PSEUDO_FS: &[&str] = &[
    "tmpfs",
    "devtmpfs",
    "squashfs",
    "overlay",
    "proc",
    "sysfs",
    "ramfs",
    "efivarfs",
    "devpts",
    "cgroup",
    "cgroup2",
    "autofs",
    "nsfs",
    "tracefs",
    "debugfs",
    "securityfs",
    "fusectl",
    "configfs",
    "bpf",
    "pstore",
    "hugetlbfs",
    "mqueue",
    "binfmt_misc",
    "rpc_pipefs",
    "fuse.portal",
    "fuse.gvfsd-fuse",
    "fuse.snapfuse",
    "fuse.lxcfs",
    "nfsd",
    "selinuxfs",
];

fn ignored_mount(mount: &str, fs: &str) -> bool {
    let _ = fs;
    #[cfg(target_os = "macos")]
    {
        if mount.starts_with("/System/Volumes/")
            || mount.starts_with("/private/var/vm")
            || mount.starts_with("/Library/Developer/CoreSimulator")
            || mount.contains("com.apple.TimeMachine.localsnapshots")
            || fs == "devfs"
            || fs == "autofs"
        {
            return true;
        }
    }
    #[cfg(target_os = "linux")]
    {
        if PSEUDO_FS.contains(&fs)
            || mount.starts_with("/snap/")
            || mount.starts_with("/proc")
            || mount.starts_with("/sys")
            || mount.starts_with("/dev")
            || mount == "/boot/efi"
            || mount == "/efi"
            || mount.starts_with("/var/lib/docker")
            || mount.starts_with("/var/snap")
            || (mount.starts_with("/run/") && !mount.starts_with("/run/media/"))
        {
            return true;
        }
    }
    let _ = mount;
    false
}

fn display_name(raw_name: &str, mount: &str, is_root: bool) -> String {
    #[cfg(windows)]
    {
        let _ = is_root;
        let letter = mount.trim_end_matches('\\');
        if raw_name.trim().is_empty() {
            return letter.to_string();
        }
        return format!("{} ({})", raw_name.trim(), letter);
    }
    #[cfg(target_os = "macos")]
    {
        if !raw_name.trim().is_empty() {
            return raw_name.trim().to_string();
        }
    }
    #[allow(unreachable_code)]
    {
        let _ = raw_name;
        if is_root {
            return String::new(); // The UI shows a localized "System" label.
        }
        Path::new(mount)
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| mount.to_string())
    }
}

/// Mounted volumes worth showing on the start screen.
pub fn list() -> Vec<Volume> {
    let Ok(disks) = Disks::new_with_refreshed_list() else {
        return Vec::new();
    };
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for d in disks.iter() {
        let mount = d.mount_point().to_string_lossy().to_string();
        let fs = d.file_system().to_string_lossy().to_string();
        if d.total_space() == 0 || ignored_mount(&mount, &fs) || !seen.insert(mount.clone()) {
            continue;
        }
        let is_root = if cfg!(windows) {
            std::env::var("SystemDrive")
                .map(|s| mount.to_uppercase().starts_with(&s.to_uppercase()))
                .unwrap_or(false)
        } else {
            mount == "/"
        };
        out.push(Volume {
            name: display_name(&d.name().to_string_lossy(), &mount, is_root),
            mount_point: mount,
            total: d.total_space(),
            available: d.available_space(),
            file_system: fs,
            removable: d.is_removable(),
            kind: match d.kind() {
                DiskKind::SSD => "ssd",
                DiskKind::HDD => "hdd",
                _ => "unknown",
            },
            is_root,
        });
    }
    // On Linux, several mounts can point to the same file system (btrfs
    // subvolumes, bind mounts). Keep the one closest to the root.
    #[cfg(target_os = "linux")]
    {
        out.sort_by_key(|v| v.mount_point.len());
        let mut fingerprints = HashSet::new();
        out.retain(|v| {
            v.removable
                || fingerprints.insert((
                    v.total,
                    v.available / (64 * 1024 * 1024),
                    v.file_system.clone(),
                ))
        });
    }
    out.sort_by(|a, b| {
        b.is_root
            .cmp(&a.is_root)
            .then(a.removable.cmp(&b.removable))
            .then(a.mount_point.cmp(&b.mount_point))
    });
    out
}

/// Turns user input into an absolute path suitable as scan root.
pub fn normalize(path: &str) -> Result<PathBuf, String> {
    let p = PathBuf::from(path);
    if !p.exists() {
        return Err(format!("{path} does not exist"));
    }
    #[cfg(windows)]
    {
        // canonicalize() would produce \\?\ verbatim paths.
        std::path::absolute(&p).map_err(|e| e.to_string())
    }
    #[cfg(not(windows))]
    {
        p.canonicalize().map_err(|e| e.to_string())
    }
}

fn same_path(a: &str, b: &Path) -> bool {
    let a = a.trim_end_matches(['/', '\\']);
    let b = b.to_string_lossy();
    let b = b.trim_end_matches(['/', '\\']);
    if cfg!(windows) {
        a.eq_ignore_ascii_case(b)
    } else {
        a == b || (a.is_empty() && b.is_empty())
    }
}

pub fn plan_for(root: &Path) -> Plan {
    let volume = list().into_iter().find(|v| same_path(&v.mount_point, root));
    Plan {
        opts: options_for(root),
        volume,
    }
}

#[cfg(target_os = "linux")]
fn unescape_mount(s: &str) -> String {
    // /proc/self/mounts encodes spaces etc. as octal escapes (\040).
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'\\' && i + 3 < bytes.len() {
            let oct = &bytes[i + 1..i + 4];
            if oct.iter().all(|b| (b'0'..=b'7').contains(b)) {
                let v = (oct[0] - b'0') as u32 * 64
                    + (oct[1] - b'0') as u32 * 8
                    + (oct[2] - b'0') as u32;
                if let Ok(v) = u8::try_from(v) {
                    out.push(v);
                    i += 4;
                    continue;
                }
            }
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

#[cfg(target_os = "linux")]
fn options_for(root: &Path) -> ScanOptions {
    struct Mount {
        source: String,
        point: PathBuf,
        fs: String,
    }
    let mounts: Vec<Mount> = std::fs::read_to_string("/proc/self/mounts")
        .unwrap_or_default()
        .lines()
        .filter_map(|line| {
            let mut parts = line.split_whitespace();
            let source = parts.next()?.to_string();
            let point = PathBuf::from(unescape_mount(parts.next()?));
            let fs = parts.next()?.to_string();
            Some(Mount { source, point, fs })
        })
        .collect();

    if mounts.is_empty() {
        // Fall back to staying on the device of the root.
        use std::os::unix::fs::MetadataExt;
        return ScanOptions {
            allowed_devices: std::fs::metadata(root).ok().map(|m| vec![m.dev()]),
            ..Default::default()
        };
    }

    let host = mounts
        .iter()
        .filter(|m| root.starts_with(&m.point))
        .max_by_key(|m| m.point.as_os_str().len());
    let host_source = host.map(|m| m.source.clone()).unwrap_or_default();

    let mut excluded: Vec<PathBuf> = mounts
        .iter()
        .filter(|m| m.point != root && m.point.starts_with(root))
        .filter(|m| {
            let same_device = m.source == host_source && m.source.starts_with('/');
            !same_device || PSEUDO_FS.contains(&m.fs.as_str())
        })
        .map(|m| m.point.clone())
        .collect();
    if root == Path::new("/") {
        for p in ["/proc", "/sys", "/dev", "/run"] {
            excluded.push(PathBuf::from(p));
        }
    }
    excluded.sort();
    excluded.dedup();
    ScanOptions {
        excluded,
        ..Default::default()
    }
}

#[cfg(target_os = "macos")]
fn options_for(root: &Path) -> ScanOptions {
    use std::os::unix::fs::MetadataExt;
    let dev = |p: &str| std::fs::metadata(p).ok().map(|m| m.dev());
    let root_dev = std::fs::metadata(root).ok().map(|m| m.dev());
    let system = dev("/");
    let data = dev("/System/Volumes/Data");
    let mut allowed: Vec<u64> = root_dev.into_iter().collect();
    // The boot volume is split into a read-only system volume and a data
    // volume that are stitched together with firmlinks. Treat them as one.
    if root_dev.is_some() && (root_dev == system || root_dev == data) {
        allowed.extend(system);
        allowed.extend(data);
    }
    allowed.sort_unstable();
    allowed.dedup();
    let mut excluded = Vec::new();
    if root == Path::new("/") {
        for p in [
            "/System/Volumes",
            "/Volumes",
            "/dev",
            "/net",
            "/home",
            "/Network",
        ] {
            excluded.push(PathBuf::from(p));
        }
    }
    ScanOptions {
        allowed_devices: if allowed.is_empty() {
            None
        } else {
            Some(allowed)
        },
        excluded,
        ..Default::default()
    }
}

#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
fn options_for(root: &Path) -> ScanOptions {
    use std::os::unix::fs::MetadataExt;
    ScanOptions {
        allowed_devices: std::fs::metadata(root).ok().map(|m| vec![m.dev()]),
        ..Default::default()
    }
}

#[cfg(not(unix))]
fn options_for(_root: &Path) -> ScanOptions {
    // Junctions and mounted folders are reparse points, which the scanner
    // never follows, so nothing special is needed on Windows.
    ScanOptions::default()
}
