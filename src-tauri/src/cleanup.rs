//! Cleanup suggestions: well-known places where space piles up (caches,
//! build output, old phone backups, the Trash, old downloads).
//!
//! Suggestions come from the scan that is open; nothing extra is read from
//! disk. They are only suggestions: the user reviews them in the Collector,
//! and anything removed goes to the Trash.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use scan_core::{Kind, NodeId, Tree};
use serde::Serialize;

/// Suggestions smaller than this are not worth showing.
const MIN_SIZE: u64 = 20 * 1024 * 1024;
/// Most items handed to the Collector for one suggestion.
const MAX_ITEMS: usize = 5000;
/// Downloads not modified for this long count as old.
const OLD_DOWNLOAD_DAYS: u64 = 90;

#[derive(Clone, Copy)]
enum Rule {
    /// Everything inside the folder can go; the folder itself stays.
    Contents,
    /// Shown for information only (the Trash: empty it in Finder/Explorer).
    Info,
    /// Every outermost folder with this name below the scan root.
    Named(&'static str),
    /// Files in the folder (and below) not modified for a while.
    OldFiles(u64),
}

struct Place {
    key: &'static str,
    /// Relative to the home folder, or absolute when starting with `/` or a drive.
    path: &'static str,
    rule: Rule,
}

#[cfg(target_os = "macos")]
const PLACES: &[Place] = &[
    Place {
        key: "caches",
        path: "Library/Caches",
        rule: Rule::Contents,
    },
    Place {
        key: "logs",
        path: "Library/Logs",
        rule: Rule::Contents,
    },
    Place {
        key: "xcode_derived",
        path: "Library/Developer/Xcode/DerivedData",
        rule: Rule::Contents,
    },
    Place {
        key: "xcode_archives",
        path: "Library/Developer/Xcode/Archives",
        rule: Rule::Contents,
    },
    Place {
        key: "simulator_caches",
        path: "Library/Developer/CoreSimulator/Caches",
        rule: Rule::Contents,
    },
    Place {
        key: "ios_backups",
        path: "Library/Application Support/MobileSync/Backup",
        rule: Rule::Contents,
    },
    Place {
        key: "npm_cache",
        path: ".npm/_cacache",
        rule: Rule::Contents,
    },
    Place {
        key: "gradle_cache",
        path: ".gradle/caches",
        rule: Rule::Contents,
    },
    Place {
        key: "cargo_cache",
        path: ".cargo/registry/cache",
        rule: Rule::Contents,
    },
    Place {
        key: "node_modules",
        path: "",
        rule: Rule::Named("node_modules"),
    },
    Place {
        key: "old_downloads",
        path: "Downloads",
        rule: Rule::OldFiles(OLD_DOWNLOAD_DAYS),
    },
    Place {
        key: "trash",
        path: ".Trash",
        rule: Rule::Info,
    },
];

#[cfg(target_os = "windows")]
const PLACES: &[Place] = &[
    Place {
        key: "temp",
        path: "AppData/Local/Temp",
        rule: Rule::Contents,
    },
    Place {
        key: "crash_dumps",
        path: "AppData/Local/CrashDumps",
        rule: Rule::Contents,
    },
    Place {
        key: "npm_cache",
        path: "AppData/Local/npm-cache",
        rule: Rule::Contents,
    },
    Place {
        key: "gradle_cache",
        path: ".gradle/caches",
        rule: Rule::Contents,
    },
    Place {
        key: "cargo_cache",
        path: ".cargo/registry/cache",
        rule: Rule::Contents,
    },
    Place {
        key: "node_modules",
        path: "",
        rule: Rule::Named("node_modules"),
    },
    Place {
        key: "old_downloads",
        path: "Downloads",
        rule: Rule::OldFiles(OLD_DOWNLOAD_DAYS),
    },
    Place {
        key: "trash",
        path: "C:/$Recycle.Bin",
        rule: Rule::Info,
    },
];

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const PLACES: &[Place] = &[
    Place {
        key: "caches",
        path: ".cache",
        rule: Rule::Contents,
    },
    Place {
        key: "npm_cache",
        path: ".npm/_cacache",
        rule: Rule::Contents,
    },
    Place {
        key: "gradle_cache",
        path: ".gradle/caches",
        rule: Rule::Contents,
    },
    Place {
        key: "cargo_cache",
        path: ".cargo/registry/cache",
        rule: Rule::Contents,
    },
    Place {
        key: "node_modules",
        path: "",
        rule: Rule::Named("node_modules"),
    },
    Place {
        key: "old_downloads",
        path: "Downloads",
        rule: Rule::OldFiles(OLD_DOWNLOAD_DAYS),
    },
    Place {
        key: "trash",
        path: ".local/share/Trash",
        rule: Rule::Info,
    },
];

#[derive(Serialize)]
pub struct Suggestion {
    /// Identifies the text shown for it (translated in the UI).
    pub key: &'static str,
    /// Folder to open when the suggestion is clicked, if there is one.
    pub focus: Option<NodeId>,
    pub path: Option<String>,
    pub size: u64,
    /// Number of items the suggestion covers.
    pub count: usize,
    /// Items to put in the Collector (empty for information-only suggestions).
    pub ids: Vec<NodeId>,
    /// More items exist than were put in `ids`.
    pub truncated: bool,
    /// Only information: nothing to collect (the Trash).
    pub info: bool,
}

fn resolve(home: Option<&Path>, path: &str) -> Option<PathBuf> {
    let p = Path::new(path);
    if p.is_absolute() || path.get(1..2) == Some(":") {
        Some(p.to_path_buf())
    } else {
        Some(home?.join(p))
    }
}

/// Suggestions for the scan that is open, largest first.
pub fn suggestions(tree: &Tree, home: Option<&Path>) -> Vec<Suggestion> {
    let mut out = Vec::new();
    for place in PLACES {
        let suggestion = match place.rule {
            Rule::Named(name) => {
                let mut dirs = tree.dirs_named(tree.root(), name);
                dirs.sort_by_key(|&d| std::cmp::Reverse(tree.get(d).map_or(0, |n| n.size)));
                let size = dirs
                    .iter()
                    .filter_map(|&d| tree.get(d))
                    .map(|n| n.size)
                    .sum();
                Some(Suggestion {
                    key: place.key,
                    focus: None,
                    path: None,
                    size,
                    count: dirs.len(),
                    truncated: dirs.len() > MAX_ITEMS,
                    ids: dirs.into_iter().take(MAX_ITEMS).collect(),
                    info: false,
                })
            }
            rule => {
                let Some(path) = resolve(home, place.path) else {
                    continue;
                };
                let Some(id) = tree.find_path(&path) else {
                    continue;
                };
                let Some(node) = tree.get(id) else { continue };
                if node.kind != Kind::Dir {
                    continue;
                }
                match rule {
                    Rule::Contents | Rule::Info => {
                        let info = matches!(rule, Rule::Info);
                        let kids: Vec<NodeId> = node.children.clone();
                        Some(Suggestion {
                            key: place.key,
                            focus: Some(id),
                            path: Some(path.display().to_string()),
                            size: node.size,
                            count: kids.len(),
                            truncated: !info && kids.len() > MAX_ITEMS,
                            ids: if info {
                                Vec::new()
                            } else {
                                kids.into_iter().take(MAX_ITEMS).collect()
                            },
                            info,
                        })
                    }
                    Rule::OldFiles(days) => {
                        if !tree.has_file_dates() {
                            continue;
                        }
                        let cutoff = SystemTime::now()
                            .checked_sub(Duration::from_secs(days * 86_400))
                            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
                            .map_or(0, |d| d.as_nanos() as i64);
                        let mut files: Vec<(u64, NodeId)> = Vec::new();
                        tree.for_each_descendant(id, |cid, n| {
                            if n.kind != Kind::Dir
                                && n.kind != Kind::Hidden
                                && n.mtime != 0
                                && n.mtime < cutoff
                            {
                                files.push((n.size, cid));
                            }
                        });
                        files.sort_by_key(|&(size, _)| std::cmp::Reverse(size));
                        Some(Suggestion {
                            key: place.key,
                            focus: Some(id),
                            path: Some(path.display().to_string()),
                            size: files.iter().map(|&(s, _)| s).sum(),
                            count: files.len(),
                            truncated: files.len() > MAX_ITEMS,
                            ids: files
                                .into_iter()
                                .take(MAX_ITEMS)
                                .map(|(_, id)| id)
                                .collect(),
                            info: false,
                        })
                    }
                    Rule::Named(_) => unreachable!(),
                }
            }
        };
        if let Some(s) = suggestion.filter(|s| s.size >= MIN_SIZE && s.count > 0) {
            out.push(s);
        }
    }
    out.sort_by_key(|s| std::cmp::Reverse(s.size));
    out
}

// ------------------------------------------------------------------ CSV export

/// Files at least this big get their own row; folders always do.
pub const CSV_MIN_FILE: u64 = 1_000_000;
const CSV_MAX_ROWS: usize = 1_000_000;

/// Writes the folders (and files of at least [`CSV_MIN_FILE`] bytes) below
/// `id` as CSV, parents before their children, largest first. Returns the
/// number of rows.
pub fn write_csv(tree: &Tree, id: NodeId, out: &mut impl std::io::Write) -> std::io::Result<usize> {
    // UTF-8 byte order mark: spreadsheet apps then read names correctly.
    out.write_all("\u{feff}Path,Type,Size (bytes),Size,Items,Modified\n".as_bytes())?;
    let mut rows = 0;
    let mut stack: Vec<(NodeId, String)> = vec![(id, tree.path_of(id).display().to_string())];
    let sep = std::path::MAIN_SEPARATOR;
    while let Some((cur, path)) = stack.pop() {
        let Some(n) = tree.get(cur) else { continue };
        let kind = scan_core::kind_str(n.kind);
        let kind = if kind == "dir" { "folder" } else { kind };
        let modified = if n.kind == Kind::Dir {
            String::new()
        } else {
            date(n.mtime)
        };
        writeln!(
            out,
            "{},{},{},{},{},{}",
            quote(&path),
            kind,
            n.size,
            human(n.size),
            n.items,
            modified
        )?;
        rows += 1;
        if rows >= CSV_MAX_ROWS {
            break;
        }
        // Reverse, so the largest child comes off the stack first.
        for &c in n.children.iter().rev() {
            let Some(child) = tree.get(c) else { continue };
            if child.kind == Kind::Dir || child.size >= CSV_MIN_FILE {
                let name = if child.kind == Kind::Hidden {
                    "(hidden space)"
                } else {
                    &child.name
                };
                let p = if path.ends_with(sep) {
                    format!("{path}{name}")
                } else {
                    format!("{path}{sep}{name}")
                };
                stack.push((c, p));
            }
        }
    }
    Ok(rows)
}

fn quote(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

fn human(bytes: u64) -> String {
    const UNITS: [&str; 6] = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut v = bytes as f64;
    let mut i = 0;
    while v >= 1000.0 && i < UNITS.len() - 1 {
        v /= 1000.0;
        i += 1;
    }
    if i == 0 {
        format!("{bytes} B")
    } else {
        format!("{v:.1} {}", UNITS[i])
    }
}

/// YYYY-MM-DD (UTC) for a time in nanoseconds since the Unix epoch.
fn date(ns: i64) -> String {
    if ns <= 0 {
        return String::new();
    }
    // Days to civil date (Howard Hinnant's algorithm).
    let z = ns.div_euclid(86_400 * 1_000_000_000) + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = yoe + era * 400 + i64::from(m <= 2);
    format!("{y:04}-{m:02}-{d:02}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_and_sizes_format_like_a_spreadsheet_expects() {
        assert_eq!(date(0), "");
        assert_eq!(date(1_700_000_000 * 1_000_000_000), "2023-11-14");
        assert_eq!(date(951_782_400 * 1_000_000_000), "2000-02-29");
        assert_eq!(human(999), "999 B");
        assert_eq!(human(1_500_000), "1.5 MB");
        assert_eq!(quote("a,b"), "\"a,b\"");
        assert_eq!(quote("say \"hi\""), "\"say \"\"hi\"\"\"");
        assert_eq!(quote("plain"), "plain");
    }

    #[test]
    fn csv_lists_folders_and_big_files_and_suggestions_find_places() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        std::fs::create_dir_all(root.join("proj/node_modules/pkg")).unwrap();
        std::fs::create_dir_all(root.join(".cache/thing")).unwrap();
        std::fs::create_dir_all(root.join("Library/Caches/thing")).unwrap();
        std::fs::write(
            root.join("proj/node_modules/pkg/big.js"),
            vec![1u8; 30_000_000],
        )
        .unwrap();
        std::fs::write(root.join(".cache/thing/blob"), vec![2u8; 25_000_000]).unwrap();
        std::fs::write(
            root.join("Library/Caches/thing/blob"),
            vec![2u8; 25_000_000],
        )
        .unwrap();
        std::fs::write(root.join("small, \"odd\".txt"), b"x").unwrap();
        let tree = scan_core::scan(root, &Default::default(), &scan_core::Progress::new()).unwrap();

        let mut buf = Vec::new();
        let rows = write_csv(&tree, 0, &mut buf).unwrap();
        let text = String::from_utf8(buf).unwrap();
        assert!(text.starts_with("\u{feff}Path,Type,"));
        assert!(text.contains("big.js"));
        assert!(!text.contains("odd")); // Files under 1 MB are left out.
        assert_eq!(rows, text.lines().count() - 1);

        let found = suggestions(&tree, Some(root));
        let keys: Vec<&str> = found.iter().map(|s| s.key).collect();
        assert!(keys.contains(&"node_modules"), "{keys:?}");
        #[cfg(not(windows))] // Windows has no "caches" suggestion.
        assert!(keys.contains(&"caches"), "{keys:?}");
        let nm = found.iter().find(|s| s.key == "node_modules").unwrap();
        assert_eq!(nm.count, 1);
        assert!(nm.size >= 30_000_000);
    }
}
