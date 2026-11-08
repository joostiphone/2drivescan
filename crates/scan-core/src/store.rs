//! Saves scans to disk and loads them again.
//!
//! File layout:
//!
//! ```text
//! "2DSCAN" u16-version  u32 header-length  header   (little endian, uncompressed)
//! LZ4 frame: one record per node in pre-order (parents before children)
//! ```
//!
//! The header holds the summary shown in lists (root, time, totals), so saved
//! scans can be listed without reading the tree. Records store a folder's own
//! size only; totals, file counts and per-type sums are recomputed on load.
//! Hidden space is not stored: it depends on the volume at the time it is
//! shown.
//!
//! Version 2 (2DriveScan 0.5) adds the modification time of every file and,
//! at the end of the header, flags and the folders the scan excluded.
//! Version 1 files still load; their files have no dates.

use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::category::{Category, CATEGORY_COUNT};
use crate::tree::{dominant, Kind, Node, NodeId, Tree, NO_PARENT};

const MAGIC: &[u8; 6] = b"2DSCAN";
const VERSION: u16 = 2;
const OLDEST_VERSION: u16 = 1;

const HEADER_FILE_DATES: u8 = 1;

const FLAG_UNREADABLE: u8 = 1;
const FLAG_COMPLETE: u8 = 2;

/// Summary of a saved scan.
#[derive(Clone, Debug, Serialize)]
pub struct SavedScan {
    pub root_path: String,
    /// When the scan started, in milliseconds since the Unix epoch.
    pub scanned_at_ms: i64,
    pub elapsed_ms: u64,
    pub complete: bool,
    pub size: u64,
    pub files: u64,
    pub dirs: u64,
    pub unreadable: u64,
    /// Number of stored nodes.
    pub nodes: u64,
    /// Files carry modification times (version 2).
    #[serde(skip)]
    pub file_dates: bool,
    /// Folders the scan excluded (version 2).
    #[serde(skip)]
    pub excluded: Vec<String>,
}

fn kind_code(kind: Kind) -> u8 {
    match kind {
        Kind::Dir => 0,
        Kind::File => 1,
        Kind::Link => 2,
        Kind::Other => 3,
        Kind::Hidden => 4,
    }
}

fn kind_from(code: u8) -> io::Result<Kind> {
    Ok(match code {
        0 => Kind::Dir,
        1 => Kind::File,
        2 => Kind::Link,
        3 => Kind::Other,
        _ => return Err(bad("unknown node kind")),
    })
}

fn bad(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

fn write_varint(w: &mut impl Write, mut v: u64) -> io::Result<()> {
    let mut buf = [0u8; 10];
    let mut i = 0;
    loop {
        let byte = (v & 0x7f) as u8;
        v >>= 7;
        if v == 0 {
            buf[i] = byte;
            i += 1;
            break;
        }
        buf[i] = byte | 0x80;
        i += 1;
    }
    w.write_all(&buf[..i])
}

fn read_varint(r: &mut impl Read) -> io::Result<u64> {
    let mut v = 0u64;
    let mut shift = 0;
    loop {
        let mut b = [0u8; 1];
        r.read_exact(&mut b)?;
        if shift >= 64 {
            return Err(bad("varint too long"));
        }
        v |= ((b[0] & 0x7f) as u64) << shift;
        if b[0] & 0x80 == 0 {
            return Ok(v);
        }
        shift += 7;
    }
}

fn zigzag(v: i64) -> u64 {
    ((v << 1) ^ (v >> 63)) as u64
}

fn unzigzag(v: u64) -> i64 {
    ((v >> 1) as i64) ^ -((v & 1) as i64)
}

fn write_str(w: &mut impl Write, s: &str) -> io::Result<()> {
    write_varint(w, s.len() as u64)?;
    w.write_all(s.as_bytes())
}

fn read_str(r: &mut impl Read, max: u64) -> io::Result<Box<str>> {
    let len = read_varint(r)?;
    if len > max {
        return Err(bad("string too long"));
    }
    let mut buf = vec![0u8; len as usize];
    r.read_exact(&mut buf)?;
    Ok(String::from_utf8_lossy(&buf).into())
}

fn encode_header(s: &SavedScan, version: u16) -> io::Result<Vec<u8>> {
    let mut h = Vec::new();
    write_str(&mut h, &s.root_path)?;
    h.write_all(&s.scanned_at_ms.to_le_bytes())?;
    h.write_all(&s.elapsed_ms.to_le_bytes())?;
    h.push(s.complete as u8);
    for v in [s.size, s.files, s.dirs, s.unreadable, s.nodes] {
        h.write_all(&v.to_le_bytes())?;
    }
    if version < 2 {
        return Ok(h);
    }
    // Version 2 additions; older readers ignore trailing header bytes.
    h.push(if s.file_dates { HEADER_FILE_DATES } else { 0 });
    write_varint(&mut h, s.excluded.len() as u64)?;
    for e in &s.excluded {
        write_str(&mut h, e)?;
    }
    Ok(h)
}

fn decode_header(h: &[u8]) -> io::Result<SavedScan> {
    let mut r = h;
    let root_path = read_str(&mut r, 64 * 1024)?.into_string();
    let mut b8 = [0u8; 8];
    let mut next_u64 = |r: &mut &[u8]| -> io::Result<u64> {
        r.read_exact(&mut b8)?;
        Ok(u64::from_le_bytes(b8))
    };
    let scanned_at_ms = next_u64(&mut r)? as i64;
    let elapsed_ms = next_u64(&mut r)?;
    let mut flag = [0u8; 1];
    r.read_exact(&mut flag)?;
    let mut summary = SavedScan {
        root_path,
        scanned_at_ms,
        elapsed_ms,
        complete: flag[0] != 0,
        size: next_u64(&mut r)?,
        files: next_u64(&mut r)?,
        dirs: next_u64(&mut r)?,
        unreadable: next_u64(&mut r)?,
        nodes: next_u64(&mut r)?,
        file_dates: false,
        excluded: Vec::new(),
    };
    // Version 2 additions (absent in version 1 headers).
    if !r.is_empty() {
        r.read_exact(&mut flag)?;
        summary.file_dates = flag[0] & HEADER_FILE_DATES != 0;
        let count = read_varint(&mut r)?;
        if count > 10_000 {
            return Err(bad("too many excluded folders"));
        }
        for _ in 0..count {
            summary
                .excluded
                .push(read_str(&mut r, 64 * 1024)?.into_string());
        }
    }
    Ok(summary)
}

/// Live (not removed, not hidden-space) children of a node.
fn stored_children<'a>(tree: &'a Tree, id: NodeId) -> impl Iterator<Item = NodeId> + 'a {
    tree.raw(id).children.iter().copied().filter(move |&c| {
        let n = tree.raw(c);
        !n.removed && n.kind != Kind::Hidden
    })
}

/// Writes `tree` to `path` (atomically, through a temporary file).
pub fn save(tree: &Tree, path: &Path, elapsed_ms: u64) -> io::Result<SavedScan> {
    write(tree, path, elapsed_ms, VERSION)
}

fn write(tree: &Tree, path: &Path, elapsed_ms: u64, version: u16) -> io::Result<SavedScan> {
    let stats = tree.stats();
    let mut count = 0u64;
    let mut stack = vec![tree.root()];
    while let Some(id) = stack.pop() {
        count += 1;
        stack.extend(stored_children(tree, id));
    }
    let summary = SavedScan {
        root_path: stats.root_path.clone(),
        scanned_at_ms: stats.scanned_at_ms,
        elapsed_ms,
        complete: stats.complete,
        size: stats.size.saturating_sub(stats.hidden),
        files: stats.files,
        dirs: stats.dirs,
        unreadable: stats.unreadable,
        nodes: count,
        file_dates: tree.has_file_dates() && version >= 2,
        excluded: tree
            .excluded()
            .iter()
            .map(|p| p.display().to_string())
            .collect(),
    };

    if let Some(dir) = path.parent() {
        fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension("tmp");
    {
        let mut out = BufWriter::new(File::create(&tmp)?);
        let header = encode_header(&summary, version)?;
        out.write_all(MAGIC)?;
        out.write_all(&version.to_le_bytes())?;
        out.write_all(&(header.len() as u32).to_le_bytes())?;
        out.write_all(&header)?;
        out.write_all(&tree.scanned_at().to_le_bytes())?;

        let mut body = lz4_flex::frame::FrameEncoder::new(out);
        let mut stack = vec![tree.root()];
        while let Some(id) = stack.pop() {
            let n = tree.raw(id);
            let children: Vec<NodeId> = stored_children(tree, id).collect();
            let mut flags = 0u8;
            if n.unreadable {
                flags |= FLAG_UNREADABLE;
            }
            if n.complete {
                flags |= FLAG_COMPLETE;
            }
            body.write_all(&[kind_code(n.kind), flags, n.category as u8])?;
            write_str(&mut body, if id == tree.root() { "" } else { &n.name })?;
            if n.kind == Kind::Dir {
                let below: u64 = n.children.iter().map(|&c| tree.raw(c).size).sum();
                write_varint(&mut body, n.size.saturating_sub(below))?;
                write_varint(&mut body, zigzag(n.mtime))?;
                write_varint(&mut body, children.len() as u64)?;
            } else {
                write_varint(&mut body, n.size)?;
                if version >= 2 {
                    write_varint(&mut body, zigzag(n.mtime))?;
                }
            }
            // Pre-order: push in reverse so the largest child is written first.
            stack.extend(children.into_iter().rev());
        }
        let mut out = body.finish().map_err(io::Error::other)?;
        out.flush()?;
        out.get_ref().sync_all()?;
    }
    fs::rename(&tmp, path)?;
    Ok(summary)
}

fn open_checked(path: &Path) -> io::Result<(BufReader<File>, SavedScan, u16)> {
    let mut r = BufReader::new(File::open(path)?);
    let mut magic = [0u8; 6];
    r.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(bad("not a 2DriveScan scan file"));
    }
    let mut v = [0u8; 2];
    r.read_exact(&mut v)?;
    let version = u16::from_le_bytes(v);
    if !(OLDEST_VERSION..=VERSION).contains(&version) {
        return Err(bad("unsupported scan file version"));
    }
    let mut len = [0u8; 4];
    r.read_exact(&mut len)?;
    let len = u32::from_le_bytes(len) as usize;
    if len > 1024 * 1024 {
        return Err(bad("header too large"));
    }
    let mut header = vec![0u8; len];
    r.read_exact(&mut header)?;
    Ok((r, decode_header(&header)?, version))
}

/// Reads only the summary of a saved scan.
pub fn read_summary(path: &Path) -> io::Result<SavedScan> {
    open_checked(path).map(|(_, s, _)| s)
}

/// Loads a saved scan.
pub fn load(path: &Path) -> io::Result<Tree> {
    let (mut r, summary, version) = open_checked(path)?;
    let mut ts = [0u8; 8];
    r.read_exact(&mut ts)?;
    let scanned_at = i64::from_le_bytes(ts);
    let mut body = BufReader::new(lz4_flex::frame::FrameDecoder::new(r));

    let capacity = summary.nodes.min(50_000_000) as usize;
    let mut nodes: Vec<Node> = Vec::with_capacity(capacity);
    // (node, children still to read)
    let mut open: Vec<(NodeId, u64)> = Vec::new();
    loop {
        let parent = match open.last_mut() {
            Some((id, left)) if *left > 0 => {
                *left -= 1;
                *id
            }
            Some(_) => {
                open.pop();
                continue;
            }
            None if nodes.is_empty() => NO_PARENT,
            None => break,
        };
        let mut head = [0u8; 3];
        body.read_exact(&mut head)?;
        let kind = kind_from(head[0])?;
        let name = read_str(&mut body, 64 * 1024)?;
        let size = read_varint(&mut body)?;
        let (mtime, child_count) = if kind == Kind::Dir {
            (unzigzag(read_varint(&mut body)?), read_varint(&mut body)?)
        } else if version >= 2 {
            (unzigzag(read_varint(&mut body)?), 0)
        } else {
            (0, 0)
        };
        let id = nodes.len() as NodeId;
        if id == NO_PARENT {
            return Err(bad("too many nodes"));
        }
        nodes.push(Node {
            name,
            size,
            items: if kind == Kind::Dir { 0 } else { 1 },
            parent,
            kind,
            category: Category::from_index(head[2] as usize),
            unreadable: head[1] & FLAG_UNREADABLE != 0,
            removed: false,
            complete: head[1] & FLAG_COMPLETE != 0,
            mtime,
            children: Vec::new(),
            cats: (kind == Kind::Dir).then(|| Box::new([0u64; CATEGORY_COUNT])),
        });
        if parent != NO_PARENT {
            nodes[parent as usize].children.push(id);
        }
        if kind == Kind::Dir && child_count > 0 {
            open.push((id, child_count));
        }
    }
    if nodes.is_empty() || nodes[0].kind != Kind::Dir {
        return Err(bad("empty scan file"));
    }

    // Totals bottom-up: in pre-order every node comes after its parent.
    for i in (1..nodes.len()).rev() {
        let (size, items, kind, category) = {
            let n = &nodes[i];
            (n.size, n.items, n.kind, n.category)
        };
        let child_cats = (kind == Kind::Dir).then(|| *nodes[i].cats.as_deref().unwrap());
        if kind == Kind::Dir {
            let n = &mut nodes[i];
            n.category = dominant(n.cats.as_deref().unwrap());
        }
        let p = nodes[i].parent as usize;
        let parent = &mut nodes[p];
        parent.size += size;
        parent.items += items;
        let pc = parent.cats.as_deref_mut().expect("parents are folders");
        match child_cats {
            Some(c) => pc.iter_mut().zip(c.iter()).for_each(|(d, s)| *d += *s),
            None => pc[category.index()] += size,
        }
    }
    let root_cats = *nodes[0].cats.as_deref().unwrap();
    nodes[0].category = dominant(&root_cats);

    let unreadable = summary.unreadable;
    let excluded = summary.excluded.iter().map(PathBuf::from).collect();
    Ok(Tree::from_nodes(
        PathBuf::from(summary.root_path),
        nodes,
        unreadable,
        scanned_at,
        summary.complete,
    )
    .with_meta(summary.file_dates, excluded))
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Writes a scan in an older format, for compatibility tests.
    pub(crate) fn save_version(tree: &Tree, path: &Path, version: u16) -> io::Result<SavedScan> {
        write(tree, path, 0, version)
    }

    #[test]
    fn varints_and_zigzag_roundtrip() {
        for v in [0u64, 1, 127, 128, 300, u32::MAX as u64, u64::MAX] {
            let mut buf = Vec::new();
            write_varint(&mut buf, v).unwrap();
            assert_eq!(read_varint(&mut buf.as_slice()).unwrap(), v);
        }
        for v in [0i64, 1, -1, 1_700_000_000_000_000_000, i64::MIN, i64::MAX] {
            assert_eq!(unzigzag(zigzag(v)), v);
        }
    }
}
