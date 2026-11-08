//! Finds files with identical contents below a folder of a scan.
//!
//! Work is done in rounds, each only on files that are still candidates:
//! files of equal length, then equal hashes of their first and last 16 KiB,
//! then equal hashes of the whole contents (XXH3-128). Hard links to the same
//! file are not duplicates (they take space once) and are left out. Files
//! that cannot be read are skipped.

use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicU8, Ordering};

use rayon::prelude::*;
use serde::Serialize;
use xxhash_rust::xxh3::Xxh3;

use crate::tree::{Kind, NodeId, Tree};

const EDGE: u64 = 16 * 1024;

/// Live counters while duplicates are searched.
#[derive(Debug, Default)]
pub struct DupProgress {
    /// 1 = comparing sizes, 2 = comparing the start and end, 3 = comparing contents.
    pub phase: AtomicU8,
    pub files_done: AtomicU64,
    pub files_total: AtomicU64,
    pub bytes_done: AtomicU64,
    pub bytes_total: AtomicU64,
    cancelled: AtomicBool,
}

#[derive(Clone, Debug, Serialize)]
pub struct DupSnapshot {
    pub phase: u8,
    pub files_done: u64,
    pub files_total: u64,
    pub bytes_done: u64,
    pub bytes_total: u64,
}

impl DupProgress {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Relaxed);
    }

    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }

    pub fn snapshot(&self) -> DupSnapshot {
        DupSnapshot {
            phase: self.phase.load(Ordering::Relaxed),
            files_done: self.files_done.load(Ordering::Relaxed),
            files_total: self.files_total.load(Ordering::Relaxed),
            bytes_done: self.bytes_done.load(Ordering::Relaxed),
            bytes_total: self.bytes_total.load(Ordering::Relaxed),
        }
    }

    fn start(&self, phase: u8, files: u64, bytes: u64) {
        self.phase.store(phase, Ordering::Relaxed);
        self.files_done.store(0, Ordering::Relaxed);
        self.files_total.store(files, Ordering::Relaxed);
        self.bytes_done.store(0, Ordering::Relaxed);
        self.bytes_total.store(bytes, Ordering::Relaxed);
    }
}

/// Files with identical contents.
#[derive(Clone, Debug, Serialize)]
pub struct DupGroup {
    /// Length of each file in bytes.
    pub len: u64,
    /// Space one copy takes on disk.
    pub size: u64,
    /// The copies, shortest path first.
    pub ids: Vec<NodeId>,
}

impl DupGroup {
    /// Space that removing all copies but one would free.
    pub fn wasted(&self) -> u64 {
        self.size * (self.ids.len() as u64).saturating_sub(1)
    }
}

struct Candidate {
    id: NodeId,
    path: PathBuf,
    size: u64,
    len: u64,
    key: u128,
}

/// Groups of identical files below `root` that take at least `min_size`
/// bytes each, the most space-wasting group first. Returns what was found so
/// far when cancelled.
pub fn find_duplicates(
    tree: &Tree,
    root: NodeId,
    min_size: u64,
    progress: &DupProgress,
) -> Vec<DupGroup> {
    compare_files(duplicate_candidates(tree, root, min_size), progress)
}

/// Files below `root` worth comparing: (id, path, size on disk). Only this
/// step needs the tree, so callers can release it during the slow part.
pub fn duplicate_candidates(
    tree: &Tree,
    root: NodeId,
    min_size: u64,
) -> Vec<(NodeId, PathBuf, u64)> {
    let mut files: Vec<(NodeId, u64)> = Vec::new();
    if tree.get(root).is_some() {
        tree.for_each_descendant(root, |id, n| {
            if n.kind == Kind::File && n.size >= min_size.max(1) {
                files.push((id, n.size));
            }
        });
    }
    files
        .into_iter()
        .map(|(id, size)| (id, tree.path_of(id), size))
        .collect()
}

/// Compares the candidate files on disk (see [`duplicate_candidates`]).
pub fn compare_files(files: Vec<(NodeId, PathBuf, u64)>, progress: &DupProgress) -> Vec<DupGroup> {
    progress.start(1, files.len() as u64, 0);

    // Round 1: the exact length (and, on Unix, the identity to drop hard links).
    let mut cands: Vec<Candidate> = files
        .into_par_iter()
        .filter_map(|(id, path, size)| {
            if progress.is_cancelled() {
                return None;
            }
            let md = std::fs::symlink_metadata(&path).ok();
            progress.files_done.fetch_add(1, Ordering::Relaxed);
            let md = md.filter(|m| m.is_file())?;
            Some(Candidate {
                id,
                path,
                size,
                len: md.len(),
                key: identity(&md),
            })
        })
        .collect();
    let mut seen = std::collections::HashSet::new();
    cands.retain(|c| c.key == 0 || seen.insert(c.key));
    let cands = regroup(cands, |c| c.len as u128);

    // Round 2: start and end of each file.
    let bytes: u64 = cands.iter().map(|c| c.len.min(2 * EDGE)).sum();
    progress.start(2, cands.len() as u64, bytes);
    let cands = hash_round(cands, progress, |c| hash_edges(&c.path, c.len));

    // Round 3: whole contents of files longer than the parts already compared.
    let (small, big): (Vec<Candidate>, Vec<Candidate>) =
        cands.into_iter().partition(|c| c.len <= 2 * EDGE);
    let bytes: u64 = big.iter().map(|c| c.len).sum();
    progress.start(3, big.len() as u64, bytes);
    let mut done = small;
    done.extend(hash_round(big, progress, |c| hash_all(&c.path, progress)));

    let mut groups: HashMap<(u64, u128), Vec<Candidate>> = HashMap::new();
    for c in done {
        groups.entry((c.len, c.key)).or_default().push(c);
    }
    let mut out: Vec<DupGroup> = groups
        .into_values()
        .filter(|g| g.len() > 1)
        .map(|mut g| {
            // The copy that stays comes first: not named like a copy, then
            // the shortest path.
            g.sort_by(|a, b| {
                let key = |c: &Candidate| (looks_like_copy(&c.path), c.path.components().count());
                key(a).cmp(&key(b)).then_with(|| a.path.cmp(&b.path))
            });
            DupGroup {
                len: g[0].len,
                size: g.iter().map(|c| c.size).max().unwrap_or(0),
                ids: g.into_iter().map(|c| c.id).collect(),
            }
        })
        .collect();
    out.sort_by(|a, b| {
        b.wasted()
            .cmp(&a.wasted())
            .then_with(|| a.ids[0].cmp(&b.ids[0]))
    });
    out
}

/// Keeps candidates that share `key(c)` with at least one other candidate of
/// the same length; the key becomes the candidate's new `key`.
fn regroup(cands: Vec<Candidate>, key: impl Fn(&Candidate) -> u128) -> Vec<Candidate> {
    let mut map: HashMap<(u64, u128), Vec<Candidate>> = HashMap::new();
    for mut c in cands {
        let k = key(&c);
        c.key = k;
        map.entry((c.len, k)).or_default().push(c);
    }
    map.into_values()
        .filter(|g| g.len() > 1)
        .flatten()
        .collect()
}

fn hash_round(
    cands: Vec<Candidate>,
    progress: &DupProgress,
    hash: impl Fn(&Candidate) -> io::Result<u128> + Sync,
) -> Vec<Candidate> {
    let hashed: Vec<Candidate> = cands
        .into_par_iter()
        .filter_map(|mut c| {
            if progress.is_cancelled() {
                return None;
            }
            let h = hash(&c).ok();
            progress.files_done.fetch_add(1, Ordering::Relaxed);
            c.key = h?;
            Some(c)
        })
        .collect();
    regroup(hashed, |c| c.key)
}

/// "photo copy.jpg", "photo copy 2.jpg", "photo (1).jpg", "photo - Copy.jpg",
/// "photo kopie.jpg": names the system or a user gives a copy.
pub fn looks_like_copy(path: &std::path::Path) -> bool {
    let Some(stem) = path.file_stem().map(|s| s.to_string_lossy().to_lowercase()) else {
        return false;
    };
    let stem = stem.trim_end();
    // Drop a trailing number: "copy 2", "(3)".
    let without_number = stem.trim_end_matches(|c: char| c.is_ascii_digit());
    let bracketed = stem.ends_with(')') && {
        let inner = stem.trim_end_matches(')');
        let digits = inner.trim_end_matches(|c: char| c.is_ascii_digit());
        digits.len() < inner.len() && digits.ends_with(" (")
    };
    bracketed
        || [
            " copy",
            "-copy",
            "_copy",
            " - copy",
            " kopie",
            " - kopie",
            " copie",
            " kopie van",
        ]
        .iter()
        .any(|w| without_number.trim_end().ends_with(w))
}

#[cfg(unix)]
fn identity(md: &std::fs::Metadata) -> u128 {
    use std::os::unix::fs::MetadataExt;
    if md.nlink() > 1 {
        ((md.dev() as u128) << 64) | md.ino() as u128
    } else {
        0
    }
}

#[cfg(not(unix))]
fn identity(_md: &std::fs::Metadata) -> u128 {
    0
}

fn hash_edges(path: &std::path::Path, len: u64) -> io::Result<u128> {
    let mut f = File::open(path)?;
    let mut h = Xxh3::new();
    let mut buf = vec![0u8; EDGE as usize];
    let n = read_up_to(&mut f, &mut buf)?;
    h.update(&buf[..n]);
    if len > 2 * EDGE {
        f.seek(SeekFrom::End(-(EDGE as i64)))?;
        let n = read_up_to(&mut f, &mut buf)?;
        h.update(&buf[..n]);
    } else if len > EDGE {
        let n = read_up_to(&mut f, &mut buf)?;
        h.update(&buf[..n]);
    }
    Ok(h.digest128())
}

fn hash_all(path: &std::path::Path, progress: &DupProgress) -> io::Result<u128> {
    let mut f = File::open(path)?;
    let mut h = Xxh3::new();
    let mut buf = vec![0u8; 1024 * 1024];
    loop {
        if progress.is_cancelled() {
            return Err(io::Error::other("cancelled"));
        }
        let n = f.read(&mut buf)?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
        progress.bytes_done.fetch_add(n as u64, Ordering::Relaxed);
    }
    Ok(h.digest128())
}

fn read_up_to(f: &mut File, buf: &mut [u8]) -> io::Result<usize> {
    let mut filled = 0;
    while filled < buf.len() {
        let n = f.read(&mut buf[filled..])?;
        if n == 0 {
            break;
        }
        filled += n;
    }
    Ok(filled)
}
