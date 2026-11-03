//! In-memory representation of a scanned directory hierarchy.
//!
//! Nodes live in a flat arena and refer to each other by [`NodeId`]. IDs stay
//! stable for the lifetime of a [`Tree`], even when nodes are removed, which
//! lets the UI hold on to them across interactions.

use std::cmp::Reverse;
use std::collections::BinaryHeap;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::category::{Category, CATEGORY_COUNT};

pub type NodeId = u32;
pub const NO_PARENT: NodeId = u32::MAX;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Dir,
    File,
    Link,
    Other,
    /// Synthetic node: used space on a volume that could not be attributed to
    /// any scanned file (snapshots, system files, swap, no permission, ...).
    Hidden,
}

#[derive(Debug)]
pub struct Node {
    pub name: Box<str>,
    /// Allocated size in bytes, including all descendants.
    pub size: u64,
    /// Number of files (non-directories) in this subtree. 1 for a file.
    pub items: u64,
    pub parent: NodeId,
    pub kind: Kind,
    /// For files: their category. For directories: the dominant category.
    pub category: Category,
    /// The directory (or part of it) could not be read.
    pub unreadable: bool,
    pub removed: bool,
    /// Directories: every entry below was read (false for unreadable or
    /// cloud-only folders and for folders an interrupted scan did not finish).
    /// Only complete folders are reused by a later scan.
    pub complete: bool,
    /// Directories: modification time in nanoseconds since the Unix epoch, as
    /// seen during the scan. It changes whenever an entry is added, removed or
    /// renamed, which is what lets a rescan skip unchanged folders.
    pub mtime: i64,
    /// Children sorted by size, largest first.
    pub children: Vec<NodeId>,
    /// Per-category byte totals for directories.
    pub cats: Option<Box<[u64; CATEGORY_COUNT]>>,
}

impl Node {
    pub fn is_dir(&self) -> bool {
        self.kind == Kind::Dir
    }
}

/// Summary information returned to the UI after a scan.
#[derive(Clone, Debug, Serialize)]
pub struct TreeStats {
    pub root_id: NodeId,
    pub root_path: String,
    pub size: u64,
    pub files: u64,
    pub dirs: u64,
    pub unreadable: u64,
    pub hidden: u64,
    /// False when the scan was interrupted before it finished.
    pub complete: bool,
    /// When the scan started, in milliseconds since the Unix epoch.
    pub scanned_at_ms: i64,
}

/// A node as sent to the UI, optionally with (pruned) children.
#[derive(Clone, Debug, Serialize)]
pub struct ViewNode {
    /// Node id, or -1 for an aggregated group of small items.
    pub id: i64,
    pub name: String,
    pub size: u64,
    pub items: u64,
    pub kind: &'static str,
    pub cat: u8,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub unreadable: bool,
    /// Whether the node has children at all (even if they were pruned).
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    pub has_children: bool,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<ViewNode>,
}

/// A flat search / largest-files hit.
#[derive(Clone, Debug, Serialize)]
pub struct ItemHit {
    pub id: NodeId,
    pub name: String,
    pub size: u64,
    pub kind: &'static str,
    pub cat: u8,
    /// Path relative to the node that was searched.
    pub rel_path: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct Crumb {
    pub id: NodeId,
    pub name: String,
}

pub fn kind_str(kind: Kind) -> &'static str {
    match kind {
        Kind::Dir => "dir",
        Kind::File => "file",
        Kind::Link => "link",
        Kind::Other => "other",
        Kind::Hidden => "hidden",
    }
}

#[derive(Debug)]
pub struct Tree {
    root_path: PathBuf,
    pub(crate) nodes: Vec<Node>,
    pub(crate) unreadable_dirs: u64,
    hidden: u64,
    /// When the scan started, in nanoseconds since the Unix epoch.
    pub(crate) scanned_at: i64,
}

impl Tree {
    pub(crate) fn from_nodes(
        root_path: PathBuf,
        nodes: Vec<Node>,
        unreadable_dirs: u64,
        scanned_at: i64,
    ) -> Self {
        Tree {
            root_path,
            nodes,
            unreadable_dirs,
            hidden: 0,
            scanned_at,
        }
    }

    /// False when the scan was interrupted; the tree then holds what was read
    /// so far and can be used to continue later.
    pub fn is_complete(&self) -> bool {
        self.nodes.first().map(|n| n.complete).unwrap_or(false)
    }

    /// When the scan started, in nanoseconds since the Unix epoch.
    pub fn scanned_at(&self) -> i64 {
        self.scanned_at
    }

    /// Node by id, including removed ones (for crate-internal walks).
    pub(crate) fn raw(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    pub fn root(&self) -> NodeId {
        0
    }

    pub fn root_path(&self) -> &Path {
        &self.root_path
    }

    /// Total number of nodes ever allocated (including removed ones).
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }

    pub fn get(&self, id: NodeId) -> Option<&Node> {
        self.nodes.get(id as usize).filter(|n| !n.removed)
    }

    pub fn stats(&self) -> TreeStats {
        let root = &self.nodes[0];
        let dirs = self
            .nodes
            .iter()
            .filter(|n| !n.removed && n.kind == Kind::Dir)
            .count() as u64;
        TreeStats {
            root_id: 0,
            root_path: self.root_path.display().to_string(),
            size: root.size,
            files: root.items,
            dirs,
            unreadable: self.unreadable_dirs,
            hidden: self.hidden,
            complete: self.is_complete(),
            scanned_at_ms: self.scanned_at / 1_000_000,
        }
    }

    /// Adds a synthetic "hidden space" child to the root, representing used
    /// volume space not accounted for by scanned files.
    pub fn add_hidden_space(&mut self, bytes: u64) {
        if bytes == 0 || self.hidden > 0 {
            return;
        }
        let id = self.nodes.len() as NodeId;
        self.nodes.push(Node {
            name: "Hidden space".into(),
            size: bytes,
            items: 0,
            parent: 0,
            kind: Kind::Hidden,
            category: Category::Other,
            unreadable: false,
            removed: false,
            complete: true,
            mtime: 0,
            children: Vec::new(),
            cats: None,
        });
        self.hidden = bytes;
        self.nodes[0].size += bytes;
        self.nodes[0].children.push(id);
        self.sort_children(0);
    }

    fn sort_children(&mut self, id: NodeId) {
        let mut children = std::mem::take(&mut self.nodes[id as usize].children);
        children.sort_by_key(|&c| Reverse(self.nodes[c as usize].size));
        self.nodes[id as usize].children = children;
    }

    /// Chain of ancestors from the root down to (and including) `id`.
    pub fn ancestors(&self, id: NodeId) -> Vec<NodeId> {
        let mut chain = Vec::new();
        let mut cur = id;
        while cur != NO_PARENT {
            chain.push(cur);
            cur = self.nodes[cur as usize].parent;
        }
        chain.reverse();
        chain
    }

    pub fn crumbs(&self, id: NodeId) -> Vec<Crumb> {
        self.ancestors(id)
            .into_iter()
            .map(|a| Crumb {
                id: a,
                name: self.display_name(a),
            })
            .collect()
    }

    fn display_name(&self, id: NodeId) -> String {
        let node = &self.nodes[id as usize];
        if id == 0 {
            let p = self.root_path.display().to_string();
            return p;
        }
        node.name.to_string()
    }

    /// Absolute file system path of a node.
    pub fn path_of(&self, id: NodeId) -> PathBuf {
        let mut path = self.root_path.clone();
        for a in self.ancestors(id).into_iter().skip(1) {
            path.push(&*self.nodes[a as usize].name);
        }
        path
    }

    fn rel_path(&self, base: NodeId, id: NodeId) -> String {
        let chain = self.ancestors(id);
        let start = chain
            .iter()
            .position(|&a| a == base)
            .map(|p| p + 1)
            .unwrap_or(1);
        let parts: Vec<&str> = chain[start..]
            .iter()
            .map(|&a| &*self.nodes[a as usize].name)
            .collect();
        parts.join(std::path::MAIN_SEPARATOR_STR)
    }

    fn to_view_leaf(&self, id: NodeId) -> ViewNode {
        let n = &self.nodes[id as usize];
        ViewNode {
            id: id as i64,
            name: if id == 0 {
                self.display_name(0)
            } else {
                n.name.to_string()
            },
            size: n.size,
            items: n.items,
            kind: kind_str(n.kind),
            cat: n.category as u8,
            unreadable: n.unreadable,
            has_children: !n.children.is_empty(),
            children: Vec::new(),
        }
    }

    /// Builds a pruned subtree for rendering. Children smaller than
    /// `min_fraction` of the view root are merged into one group per parent.
    pub fn view(&self, id: NodeId, depth: u32, min_fraction: f64) -> Option<ViewNode> {
        self.get(id)?;
        let min_size = (self.nodes[id as usize].size as f64 * min_fraction).max(1.0) as u64;
        Some(self.view_rec(id, depth, min_size))
    }

    fn view_rec(&self, id: NodeId, depth: u32, min_size: u64) -> ViewNode {
        let mut v = self.to_view_leaf(id);
        if depth == 0 {
            return v;
        }
        let node = &self.nodes[id as usize];
        let mut group_size = 0u64;
        let mut group_items = 0u64;
        for (i, &c) in node.children.iter().enumerate() {
            let child = &self.nodes[c as usize];
            if child.size < min_size {
                // Children are sorted, so everything after this is smaller.
                for &rest in &node.children[i..] {
                    group_size += self.nodes[rest as usize].size;
                    group_items += 1;
                }
                break;
            }
            v.children.push(self.view_rec(c, depth - 1, min_size));
        }
        if group_items > 0 && group_size > 0 {
            v.children.push(ViewNode {
                id: -1,
                name: String::new(),
                size: group_size,
                items: group_items,
                kind: "group",
                cat: Category::Other as u8,
                unreadable: false,
                has_children: false,
                children: Vec::new(),
            });
        }
        v
    }

    /// Direct children of `id`, up to `limit` entries; the remainder is
    /// returned as a single group entry.
    pub fn list_children(&self, id: NodeId, limit: usize) -> Vec<ViewNode> {
        let Some(node) = self.get(id) else {
            return Vec::new();
        };
        let mut out: Vec<ViewNode> = node
            .children
            .iter()
            .take(limit)
            .map(|&c| self.to_view_leaf(c))
            .collect();
        if node.children.len() > limit {
            let rest = &node.children[limit..];
            out.push(ViewNode {
                id: -1,
                name: String::new(),
                size: rest.iter().map(|&c| self.nodes[c as usize].size).sum(),
                items: rest.len() as u64,
                kind: "group",
                cat: Category::Other as u8,
                unreadable: false,
                has_children: false,
                children: Vec::new(),
            });
        }
        out
    }

    /// Iterates all live descendants of `id` (excluding `id` itself).
    fn for_each_descendant(&self, id: NodeId, mut f: impl FnMut(NodeId, &Node)) {
        let mut stack: Vec<NodeId> = self.nodes[id as usize].children.clone();
        while let Some(c) = stack.pop() {
            let n = &self.nodes[c as usize];
            if n.removed {
                continue;
            }
            f(c, n);
            stack.extend_from_slice(&n.children);
        }
    }

    /// The `limit` largest files below `id`.
    pub fn largest_files(&self, id: NodeId, limit: usize) -> Vec<ItemHit> {
        if self.get(id).is_none() || limit == 0 {
            return Vec::new();
        }
        let mut heap: BinaryHeap<Reverse<(u64, NodeId)>> = BinaryHeap::with_capacity(limit + 1);
        self.for_each_descendant(id, |cid, n| {
            if matches!(n.kind, Kind::File | Kind::Link | Kind::Other) {
                if heap.len() < limit {
                    heap.push(Reverse((n.size, cid)));
                } else if let Some(Reverse((min, _))) = heap.peek() {
                    if n.size > *min {
                        heap.pop();
                        heap.push(Reverse((n.size, cid)));
                    }
                }
            }
        });
        let mut hits: Vec<(u64, NodeId)> = heap.into_iter().map(|Reverse(x)| x).collect();
        hits.sort_by_key(|&(size, _)| Reverse(size));
        hits.into_iter().map(|(_, cid)| self.hit(id, cid)).collect()
    }

    fn hit(&self, base: NodeId, id: NodeId) -> ItemHit {
        let n = &self.nodes[id as usize];
        ItemHit {
            id,
            name: n.name.to_string(),
            size: n.size,
            kind: kind_str(n.kind),
            cat: n.category as u8,
            rel_path: self.rel_path(base, id),
        }
    }

    /// Case-insensitive substring search on names below `id`, largest first.
    pub fn search(&self, id: NodeId, query: &str, limit: usize) -> Vec<ItemHit> {
        let q = query.trim().to_lowercase();
        if q.is_empty() || self.get(id).is_none() {
            return Vec::new();
        }
        let mut found: Vec<(u64, NodeId)> = Vec::new();
        self.for_each_descendant(id, |cid, n| {
            if n.kind != Kind::Hidden && n.name.to_lowercase().contains(&q) {
                found.push((n.size, cid));
            }
        });
        found.sort_by_key(|&(size, _)| Reverse(size));
        found.truncate(limit);
        found
            .into_iter()
            .map(|(_, cid)| self.hit(id, cid))
            .collect()
    }

    /// Bytes per category below `id`.
    pub fn category_totals(&self, id: NodeId) -> [u64; CATEGORY_COUNT] {
        let mut out = [0u64; CATEGORY_COUNT];
        let Some(n) = self.get(id) else {
            return out;
        };
        match &n.cats {
            Some(c) => out = **c,
            None if n.kind != Kind::Hidden => out[n.category.index()] = n.size,
            None => {}
        }
        out
    }

    /// Hidden-space bytes directly below `id` (only the root can have any).
    pub fn hidden_below(&self, id: NodeId) -> u64 {
        if id == 0 {
            self.hidden
        } else {
            0
        }
    }

    /// Removes a node (after it was deleted on disk) and updates all
    /// ancestors. Returns the number of bytes freed.
    pub fn remove(&mut self, id: NodeId) -> Option<u64> {
        if id == 0 {
            return None;
        }
        let node = self.get(id)?;
        let size = node.size;
        let items = node.items;
        let parent = node.parent;
        let cats = self.category_totals(id);
        let was_hidden = node.kind == Kind::Hidden;

        // Mark the whole subtree as removed.
        let mut stack = vec![id];
        while let Some(c) = stack.pop() {
            let n = &mut self.nodes[c as usize];
            n.removed = true;
            stack.extend_from_slice(&n.children);
        }

        if was_hidden {
            self.hidden = 0;
        }

        let p = &mut self.nodes[parent as usize];
        p.children.retain(|&c| c != id);

        // Walk up and subtract.
        let mut cur = parent;
        while cur != NO_PARENT {
            let n = &mut self.nodes[cur as usize];
            n.size = n.size.saturating_sub(size);
            n.items = n.items.saturating_sub(items);
            if let Some(c) = n.cats.as_mut() {
                for (dst, src) in c.iter_mut().zip(cats.iter()) {
                    *dst = dst.saturating_sub(*src);
                }
                n.category = dominant(c);
            }
            cur = n.parent;
        }
        // Keep sibling order correct for every ancestor.
        for a in self.ancestors(parent) {
            let ap = self.nodes[a as usize].parent;
            if ap != NO_PARENT {
                self.sort_children(ap);
            }
        }
        Some(size)
    }
}

pub(crate) fn dominant(cats: &[u64; CATEGORY_COUNT]) -> Category {
    let mut best = Category::Other;
    let mut best_size = 0;
    for (i, &s) in cats.iter().enumerate() {
        if s > best_size {
            best_size = s;
            best = Category::from_index(i);
        }
    }
    best
}
