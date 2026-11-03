//! Parallel directory walker.
//!
//! Directories are read concurrently on a dedicated rayon pool. The walk
//! builds a temporary owned tree that is flattened into a [`Tree`] arena once
//! the scan completes.
//!
//! When a previous scan of the same folder is supplied, folders whose
//! modification time did not change are taken over from it instead of being
//! read again: a folder's modification time changes whenever an entry inside
//! it is added, removed or renamed. Only their subfolders are checked (one
//! `lstat` each) and large files are re-measured, because a file can grow
//! without its folder changing. An interrupted scan returns what it has read
//! so far, with unfinished folders marked incomplete, so the next scan can
//! continue from there.

use std::collections::HashMap;
#[cfg(unix)]
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use rayon::prelude::*;
use serde::Serialize;

use crate::category::{categorize, Category, CATEGORY_COUNT};
use crate::listing::{self, EntryKind, ListError, RawEntry};
use crate::tree::{dominant, Kind, Node, NodeId, Tree, NO_PARENT};

/// Files at least this big are re-measured even inside unchanged folders:
/// databases, disk images and logs grow without touching their folder.
const REMEASURE_FROM: u64 = 16 * 1024 * 1024;

/// Folders modified this close to the previous scan are read again, because
/// some file systems store modification times with a 1–2 second precision.
const MTIME_SLACK_NS: i64 = 2_000_000_000;

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
    /// An earlier (possibly interrupted) scan of the same root. Unchanged
    /// folders are taken over from it.
    pub previous: Option<Arc<Tree>>,
}

/// Live counters shared between the scanner and whoever reports progress.
#[derive(Debug, Default)]
pub struct Progress {
    pub files: AtomicU64,
    pub dirs: AtomicU64,
    pub bytes: AtomicU64,
    pub errors: AtomicU64,
    /// Folders taken over from the previous scan without reading them.
    pub reused: AtomicU64,
    cancelled: AtomicBool,
    current: Mutex<String>,
}

#[derive(Clone, Debug, Serialize)]
pub struct ProgressSnapshot {
    pub files: u64,
    pub dirs: u64,
    pub bytes: u64,
    pub errors: u64,
    pub reused: u64,
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
            reused: self.reused.load(Ordering::Relaxed),
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
    ThreadPool(String),
}

impl std::fmt::Display for ScanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ScanError::NotADirectory(p) => write!(f, "{} is not a folder", p.display()),
            ScanError::Io(p, e) => write!(f, "cannot read {}: {e}", p.display()),
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
    mtime: i64,
    unreadable: bool,
    complete: bool,
    cats: [u64; CATEGORY_COUNT],
    files: Vec<FileTmp>,
    dirs: Vec<DirTmp>,
}

impl DirTmp {
    fn new(name: Box<str>, size: u64, mtime: i64) -> Self {
        DirTmp {
            name,
            size,
            items: 0,
            mtime,
            unreadable: false,
            complete: false,
            cats: [0; CATEGORY_COUNT],
            files: Vec::new(),
            dirs: Vec::new(),
        }
    }

    fn add_file(&mut self, file: FileTmp) {
        self.cats[file.category.index()] += file.size;
        self.size += file.size;
        self.items += 1;
        self.files.push(file);
    }
}

/// A subfolder waiting to be scanned.
struct Pending {
    path: PathBuf,
    name: Box<str>,
    size: u64,
    mtime: i64,
    previous: Option<NodeId>,
}

struct Ctx<'a> {
    opts: &'a ScanOptions,
    progress: &'a Progress,
    previous: Option<&'a Tree>,
    unreadable: AtomicU64,
    #[cfg(unix)]
    inodes: [Mutex<HashSet<(u64, u64)>>; 16],
}

impl Ctx<'_> {
    fn excluded(&self, path: &Path) -> bool {
        self.opts.excluded.iter().any(|e| e == path)
    }

    fn device_allowed(&self, dev: u64) -> bool {
        if cfg!(unix) {
            match &self.opts.allowed_devices {
                Some(devs) => devs.contains(&dev),
                None => true,
            }
        } else {
            true
        }
    }

    /// Returns false when this hard-linked file was already counted.
    #[cfg(unix)]
    fn first_link(&self, e: &RawEntry) -> bool {
        if e.nlink <= 1 {
            return true;
        }
        let shard = (e.ino as usize) % self.inodes.len();
        self.inodes[shard]
            .lock()
            .map(|mut set| set.insert((e.dev, e.ino)))
            .unwrap_or(true)
    }

    #[cfg(not(unix))]
    fn first_link(&self, _e: &RawEntry) -> bool {
        true
    }

    /// The previous scan's version of this folder, when it can be reused.
    fn reusable(&self, previous: Option<NodeId>, mtime: i64) -> Option<(&Tree, NodeId)> {
        let tree = self.previous?;
        let id = previous?;
        let node = tree.raw(id);
        let usable = node.kind == Kind::Dir
            && !node.removed
            && node.complete
            && !node.unreadable
            && mtime != 0
            && node.mtime == mtime
            && mtime < tree.scanned_at().saturating_sub(MTIME_SLACK_NS);
        usable.then_some((tree, id))
    }
}

fn file_kind(kind: EntryKind) -> Kind {
    match kind {
        EntryKind::File => Kind::File,
        EntryKind::Link => Kind::Link,
        EntryKind::Dir | EntryKind::Other => Kind::Other,
    }
}

fn scan_dir(job: Pending, ctx: &Ctx<'_>) -> DirTmp {
    if ctx.progress.is_cancelled() {
        return DirTmp::new(job.name, job.size, job.mtime);
    }
    let n = ctx.progress.dirs.fetch_add(1, Ordering::Relaxed);
    if n.is_multiple_of(8) {
        ctx.progress.set_current(&job.path);
    }
    if let Some((tree, id)) = ctx.reusable(job.previous, job.mtime) {
        if let Some(dir) = reuse_dir(&job, tree, id, ctx) {
            return dir;
        }
        // Something looked off: read the folder normally.
    }
    read_dir(job, ctx)
}

/// Reads a folder from disk.
fn read_dir(job: Pending, ctx: &Ctx<'_>) -> DirTmp {
    let mut dir = DirTmp::new(job.name, job.size, job.mtime);
    let listing = match listing::list_dir(&job.path) {
        Ok(l) => l,
        Err(ListError::Unreadable) => {
            dir.unreadable = true;
            ctx.unreadable.fetch_add(1, Ordering::Relaxed);
            ctx.progress.errors.fetch_add(1, Ordering::Relaxed);
            return dir;
        }
        // Not on this computer; nothing to count, check again next time.
        Err(ListError::CloudOnly) => return dir,
    };
    if listing.errors > 0 {
        ctx.progress
            .errors
            .fetch_add(listing.errors, Ordering::Relaxed);
    }

    // Subfolders of the previous scan, by name, so they can be reused.
    let previous_dirs: HashMap<&str, NodeId> = match (ctx.previous, job.previous) {
        (Some(tree), Some(id)) if !tree.raw(id).removed => tree
            .raw(id)
            .children
            .iter()
            .filter(|&&c| tree.raw(c).kind == Kind::Dir)
            .map(|&c| (&*tree.raw(c).name, c))
            .collect(),
        _ => HashMap::new(),
    };

    let mut subdirs = Vec::new();
    let (mut bytes, mut count) = (0u64, 0u64);
    for e in listing.entries {
        if e.kind == EntryKind::Dir {
            let path = job.path.join(&*e.name);
            if ctx.excluded(&path) || !ctx.device_allowed(e.dev) {
                continue;
            }
            let previous = previous_dirs.get(&*e.name).copied();
            subdirs.push(Pending {
                path,
                name: e.name,
                size: e.size,
                mtime: e.mtime,
                previous,
            });
            continue;
        }
        let kind = file_kind(e.kind);
        let size = if kind == Kind::File && !ctx.first_link(&e) {
            0
        } else {
            e.size
        };
        let category = if kind == Kind::File {
            categorize(&e.name)
        } else {
            Category::Other
        };
        bytes += size;
        count += 1;
        dir.add_file(FileTmp {
            name: e.name,
            size,
            kind,
            category,
        });
    }
    ctx.progress.files.fetch_add(count, Ordering::Relaxed);
    ctx.progress.bytes.fetch_add(bytes, Ordering::Relaxed);
    finish(dir, subdirs, ctx)
}

/// Takes a folder over from the previous scan. Returns None when the folder
/// should be read normally after all.
fn reuse_dir(job: &Pending, tree: &Tree, id: NodeId, ctx: &Ctx<'_>) -> Option<DirTmp> {
    let node = tree.raw(id);
    let mut files = Vec::new();
    let mut subdirs = Vec::new();
    for &c in &node.children {
        let child = tree.raw(c);
        if child.removed || child.kind == Kind::Hidden {
            continue;
        }
        let path = job.path.join(&*child.name);
        if child.kind == Kind::Dir {
            // One lstat tells whether the subfolder itself changed.
            let now = listing::stat(&path).ok()?;
            if now.kind != EntryKind::Dir {
                return None;
            }
            if ctx.excluded(&path) || !ctx.device_allowed(now.dev) {
                continue;
            }
            subdirs.push(Pending {
                path,
                name: child.name.clone(),
                size: now.size,
                mtime: now.mtime,
                previous: Some(c),
            });
        } else {
            let mut size = child.size;
            if size >= REMEASURE_FROM {
                size = listing::stat(&path).ok()?.size;
            }
            files.push(FileTmp {
                name: child.name.clone(),
                size,
                kind: child.kind,
                category: child.category,
            });
        }
    }

    ctx.progress.reused.fetch_add(1, Ordering::Relaxed);
    let mut dir = DirTmp::new(job.name.clone(), job.size, job.mtime);
    let (mut bytes, mut count) = (0u64, 0u64);
    for f in files {
        bytes += f.size;
        count += 1;
        dir.add_file(f);
    }
    ctx.progress.files.fetch_add(count, Ordering::Relaxed);
    ctx.progress.bytes.fetch_add(bytes, Ordering::Relaxed);
    Some(finish(dir, subdirs, ctx))
}

/// Scans the subfolders in parallel and adds them up.
fn finish(mut dir: DirTmp, subdirs: Vec<Pending>, ctx: &Ctx<'_>) -> DirTmp {
    dir.dirs = subdirs
        .into_par_iter()
        .map(|job| scan_dir(job, ctx))
        .collect();
    let mut complete = !ctx.progress.is_cancelled();
    for d in &dir.dirs {
        dir.size += d.size;
        dir.items += d.items;
        for (dst, src) in dir.cats.iter_mut().zip(d.cats.iter()) {
            *dst += *src;
        }
        complete &= d.complete;
    }
    dir.complete = complete;
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
        complete: d.complete,
        mtime: d.mtime,
        children: Vec::new(),
        cats: Some(Box::new(d.cats)),
    }
}

fn flatten(root_path: PathBuf, root: DirTmp, unreadable: u64, scanned_at: i64) -> Tree {
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
                complete: true,
                mtime: 0,
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
    Tree::from_nodes(root_path, nodes, unreadable, scanned_at)
}

fn default_threads() -> usize {
    let cpus = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4);
    // File system calls block a lot; oversubscribing helps on SSDs.
    (cpus * 2).clamp(4, 32)
}

fn now_ns() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos().min(i64::MAX as u128) as i64)
        .unwrap_or(0)
}

/// Scans `root` and returns the resulting tree.
///
/// When the scan is cancelled through `progress`, the tree read so far is
/// returned with [`Tree::is_complete`] false. Pass it as
/// [`ScanOptions::previous`] to continue later.
pub fn scan(root: &Path, opts: &ScanOptions, progress: &Progress) -> Result<Tree, ScanError> {
    let md = fs::metadata(root).map_err(|e| ScanError::Io(root.to_path_buf(), e))?;
    if !md.is_dir() {
        return Err(ScanError::NotADirectory(root.to_path_buf()));
    }
    listing::disable_cloud_downloads();
    let scanned_at = now_ns();
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

    // Only a scan of the same folder can be reused.
    let previous = opts
        .previous
        .as_deref()
        .filter(|p| p.root_path() == root && !p.is_empty());
    let ctx = Ctx {
        opts,
        progress,
        previous,
        unreadable: AtomicU64::new(0),
        #[cfg(unix)]
        inodes: Default::default(),
    };
    let job = Pending {
        path: root.to_path_buf(),
        name: root.display().to_string().into(),
        size: listing::disk_size(&md),
        mtime: listing::mtime_ns(&md),
        previous: previous.map(|_| 0),
    };
    let tmp = pool.install(|| scan_dir(job, &ctx));
    let unreadable = ctx.unreadable.load(Ordering::Relaxed);
    Ok(pool.install(|| flatten(root.to_path_buf(), tmp, unreadable, scanned_at)))
}
