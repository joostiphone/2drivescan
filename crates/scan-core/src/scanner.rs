//! Parallel directory walker.
//!
//! Directories are read concurrently on a dedicated rayon pool. The walk
//! builds a temporary owned tree that is flattened into a [`Tree`] arena once
//! the scan completes.

#[cfg(unix)]
use std::collections::HashSet;
use std::fs::{self, Metadata};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Mutex;

use rayon::prelude::*;
use serde::Serialize;

use crate::category::{categorize, Category, CATEGORY_COUNT};
use crate::tree::{dominant, Kind, Node, NodeId, Tree, NO_PARENT};

/// Options that control what the scanner descends into.
#[derive(Clone, Debug, Default)]
pub struct ScanOptions {
    /// When set (Unix only), directories whose device id is not in this list
    /// are skipped. Used to stay on one volume.
    pub allowed_devices: Option<Vec<u64>>,
    /// Absolute paths that are never entered.
    pub excluded: Vec<PathBuf>,
    /// Number of worker threads. 0 picks a sensible default.
    pub threads: usize,
}

/// Live counters shared between the scanner and whoever reports progress.
#[derive(Debug, Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub dirs: AtomicU64,
    pub bytes: AtomicU64,
    pub errors: AtomicU64,
    cancelled: AtomicBool,
    current: Mutex<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProgressSnapshot {
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
    pub errors: u64,
    pub current: String,
}

impl Progress {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub fn snapshot(&self) -> ProgressSnapshot {
        ProgressSnapshot {
            files: self.files.load(Ordering::Relaxed),
            dirs: self.dirs.load(Ordering::Relaxed),
            bytes: self.bytes.load(Ordering::Relaxed),
            errors: self.errors.load(Ordering::Relaxed),
            current: self.current.lock().map(|c| c.clone()).unwrap_or_default(),
        }
    }

    fn set_current(&self, path: &Path) {
        if let Ok(mut cur) = self.current.try_lock() {
            cur.clear();
            cur.push_str(&path.to_string_lossy());
        }
    }
}

#[derive(Debug)]
pub enum ScanError {
    NotADirectory(PathBuf),
    Io(PathBuf, std::io::Error),
    Cancelled,
    ThreadPool(String),
}

impl std::fmt::Display for ScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScanError::NotADirectory(p) => write!(f, "{} is not a folder", p.display()),
            ScanError::Io(p, e) => write!(f, "cannot read {}: {e}", p.display()),
            ScanError::Cancelled => write!(f, "scan cancelled"),
            ScanError::ThreadPool(e) => write!(f, "could not start scanner threads: {e}"),
        }
    }
}

impl std::error::Error for ScanError {}

struct FileTmp {
    name: Box<str>,
    size: u64,
    kind: Kind,
    category: Category,
}

struct DirTmp {
    name: Box<str>,
    size: u64,
    items: u64,
    unreadable: bool,
    cats: [u64; CATEGORY_COUNT],
    files: Vec<FileTmp>,
    dirs: Vec<DirTmp>,
}

struct Ctx<'a> {
    opts: &'a ScanOptions,
    progress: &'a Progress,
    unreadable: AtomicU64,
    #[cfg(unix)]
    inodes: [Mutex<HashSet<(u64, u64)>>; 16],
}

impl Ctx<'_> {
    fn excluded(&self, path: &Path) -> bool {
        self.opts.excluded.iter().any(|e| e == path)
    }

    #[cfg(unix)]
    fn device_allowed(&self, md: &Metadata) -> bool {
        use std::os::unix::fs::MetadataExt;
        match &self.opts.allowed_devices {
            Some(devs) => devs.contains(&md.dev()),
            None => true,
        }
    }

    #[cfg(not(unix))]
    fn device_allowed(&self, _md: &Metadata) -> bool {
        true
    }

    /// Returns false when this hard-linked file was already counted.
    #[cfg(unix)]
    fn first_link(&self, md: &Metadata) -> bool {
        use std::os::unix::fs::MetadataExt;
        if md.nlink() <= 1 {
            return true;
        }
        let key = (md.dev(), md.ino());
        let shard = (md.ino() as usize) % self.inodes.len();
        self.inodes[shard]
            .lock()
            .map(|mut set| set.insert(key))
            .unwrap_or(true)
    }

    #[cfg(not(unix))]
    fn first_link(&self, _md: &Metadata) -> bool {
        true
    }
}

/// Bytes a file actually occupies on disk.
#[cfg(unix)]
fn disk_size(md: &Metadata) -> u64 {
    use std::os::unix::fs::MetadataExt;
    md.blocks().saturating_mul(512)
}

#[cfg(windows)]
fn disk_size(md: &Metadata) -> u64 {
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
fn disk_size(md: &Metadata) -> u64 {
    md.len()
}

fn scan_dir(path: PathBuf, name: Box<str>, own_size: u64, ctx: &Ctx<'_>) -> DirTmp {
    let mut dir = DirTmp {
        name,
        size: own_size,
        items: 0,
        unreadable: false,
        cats: [0; CATEGORY_COUNT],
        files: Vec::new(),
        dirs: Vec::new(),
    };
    if ctx.progress.is_cancelled() {
        return dir;
    }
    let n = ctx.progress.dirs.fetch_add(1, Ordering::Relaxed);
    if n.is_multiple_of(8) {
        ctx.progress.set_current(&path);
    }

    let mut subdirs: Vec<(PathBuf, Box<str>, u64)> = Vec::new();
    {
        let entries = match fs::read_dir(&path) {
            Ok(rd) => rd,
            Err(_) => {
                dir.unreadable = true;
                ctx.unreadable.fetch_add(1, Ordering::Relaxed);
                ctx.progress.errors.fetch_add(1, Ordering::Relaxed);
                return dir;
            }
        };
        let mut file_bytes = 0u64;
        let mut file_count = 0u64;
        for entry in entries {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => {
                    dir.unreadable = true;
                    ctx.progress.errors.fetch_add(1, Ordering::Relaxed);
                    continue;
                }
            };
            let name: Box<str> = entry.file_name().to_string_lossy().into();
            let Ok(ft) = entry.file_type() else {
                ctx.progress.errors.fetch_add(1, Ordering::Relaxed);
                continue;
            };
            if ft.is_dir() {
                let child = entry.path();
                if ctx.excluded(&child) {
                    continue;
                }
                match entry.metadata() {
                    Ok(md) => {
                        if !ctx.device_allowed(&md) {
                            continue;
                        }
                        subdirs.push((child, name, disk_size(&md)));
                    }
                    Err(_) => {
                        ctx.progress.errors.fetch_add(1, Ordering::Relaxed);
                        subdirs.push((child, name, 0));
                    }
                }
                continue;
            }
            let md = entry.metadata().ok();
            let (kind, size) = if ft.is_symlink() {
                (Kind::Link, md.as_ref().map(disk_size).unwrap_or(0))
            } else if ft.is_file() {
                let size = match &md {
                    Some(md) if ctx.first_link(md) => disk_size(md),
                    Some(_) => 0,
                    None => 0,
                };
                (Kind::File, size)
            } else {
                (Kind::Other, md.as_ref().map(disk_size).unwrap_or(0))
            };
            let category = if kind == Kind::File {
                categorize(&name)
            } else {
                Category::Other
            };
            dir.cats[category.index()] += size;
            file_bytes += size;
            file_count += 1;
            dir.files.push(FileTmp {
                name,
                size,
                kind,
                category,
            });
        }
        ctx.progress.files.fetch_add(file_count, Ordering::Relaxed);
        ctx.progress.bytes.fetch_add(file_bytes, Ordering::Relaxed);
        dir.size += file_bytes;
        dir.items += file_count;
    }

    dir.dirs = subdirs
        .into_par_iter()
        .map(|(p, n, s)| scan_dir(p, n, s, ctx))
        .collect();

    for d in &dir.dirs {
        dir.size += d.size;
        dir.items += d.items;
        for (dst, src) in dir.cats.iter_mut().zip(d.cats.iter()) {
            *dst += *src;
        }
    }
    dir
}

fn dir_node(d: &DirTmp, parent: NodeId) -> Node {
    Node {
        name: d.name.clone(),
        size: d.size,
        items: d.items,
        parent,
        kind: Kind::Dir,
        category: dominant(&d.cats),
        unreadable: d.unreadable,
        removed: false,
        children: Vec::new(),
        cats: Some(Box::new(d.cats)),
    }
}

fn flatten(root_path: PathBuf, root: DirTmp, unreadable: u64) -> Tree {
    let mut nodes: Vec<Node> = Vec::with_capacity((root.items as usize).saturating_add(1024));
    nodes.push(dir_node(&root, NO_PARENT));
    let mut stack: Vec<(DirTmp, NodeId)> = vec![(root, 0)];
    while let Some((dir, id)) = stack.pop() {
        let DirTmp { files, dirs, .. } = dir;
        let mut children: Vec<NodeId> = Vec::with_capacity(files.len() + dirs.len());
        for f in files {
            children.push(nodes.len() as NodeId);
            nodes.push(Node {
                name: f.name,
                size: f.size,
                items: 1,
                parent: id,
                kind: f.kind,
                category: f.category,
                unreadable: false,
                removed: false,
                children: Vec::new(),
                cats: None,
            });
        }
        for d in dirs {
            let cid = nodes.len() as NodeId;
            children.push(cid);
            nodes.push(dir_node(&d, id));
            stack.push((d, cid));
        }
        children.sort_by(|&a, &b| nodes[b as usize].size.cmp(&nodes[a as usize].size));
        nodes[id as usize].children = children;
    }
    Tree::from_nodes(root_path, nodes, unreadable)
}

fn default_threads() -> usize {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    // File system calls block a lot; oversubscribing helps on SSDs.
    (cpus * 2).clamp(4, 32)
}

/// Scans `root` and returns the resulting tree.
pub fn scan(root: &Path, opts: &ScanOptions, progress: &Progress) -> Result<Tree, ScanError> {
    let md = fs::metadata(root).map_err(|e| ScanError::Io(root.to_path_buf(), e))?;
    if !md.is_dir() {
        return Err(ScanError::NotADirectory(root.to_path_buf()));
    }
    let threads = if opts.threads == 0 {
        default_threads()
    } else {
        opts.threads
    };
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .stack_size(16 * 1024 * 1024)
        .thread_name(|i| format!("scan-{i}"))
        .build()
        .map_err(|e| ScanError::ThreadPool(e.to_string()))?;

    let ctx = Ctx {
        opts,
        progress,
        unreadable: AtomicU64::new(0),
        #[cfg(unix)]
        inodes: Default::default(),
    };
    let root_name: Box<str> = root.display().to_string().into();
    let tmp = pool.install(|| scan_dir(root.to_path_buf(), root_name, disk_size(&md), &ctx));
    if progress.is_cancelled() {
        return Err(ScanError::Cancelled);
    }
    let unreadable = ctx.unreadable.load(Ordering::Relaxed);
    let tree = pool.install(|| flatten(root.to_path_buf(), tmp, unreadable));
    Ok(tree)
}
