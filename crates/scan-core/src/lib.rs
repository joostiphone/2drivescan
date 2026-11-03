//! Core of 2DriveScan: a fast parallel disk usage scanner and the tree model
//! the UI navigates.

pub mod category;
mod listing;
pub mod scanner;
pub mod store;
pub mod tree;

pub use category::{categorize, Category, CATEGORY_COUNT};
pub use listing::disable_cloud_downloads;
pub use scanner::{scan, Progress, ProgressSnapshot, ScanError, ScanOptions};
pub use store::SavedScan;
pub use tree::{kind_str, Crumb, ItemHit, Kind, Node, NodeId, Tree, TreeStats, ViewNode};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use std::sync::Arc;

    fn write_file(path: &std::path::Path, bytes: usize) {
        let mut f = fs::File::create(path).unwrap();
        f.write_all(&vec![7u8; bytes]).unwrap();
        f.sync_all().unwrap();
    }

    fn sample() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("photos/2024")).unwrap();
        fs::create_dir_all(root.join("music")).unwrap();
        fs::create_dir_all(root.join("empty")).unwrap();
        write_file(&root.join("photos/2024/a.jpg"), 300_000);
        write_file(&root.join("photos/2024/b.jpg"), 200_000);
        write_file(&root.join("photos/cover.png"), 50_000);
        write_file(&root.join("music/song.mp3"), 120_000);
        write_file(&root.join("notes.txt"), 10_000);
        dir
    }

    fn scan_sample(dir: &tempfile::TempDir) -> Tree {
        let progress = Progress::new();
        scan(dir.path(), &ScanOptions::default(), &progress).unwrap()
    }

    #[test]
    fn scans_files_and_sizes() {
        let dir = sample();
        let tree = scan_sample(&dir);
        let stats = tree.stats();
        assert_eq!(stats.files, 5);
        assert_eq!(stats.dirs, 5); // root, photos, 2024, music, empty
        assert!(stats.size >= 680_000, "size {} too small", stats.size);

        // Children are sorted by size: photos first.
        let root = tree.get(tree.root()).unwrap();
        let first = tree.get(root.children[0]).unwrap();
        assert_eq!(&*first.name, "photos");
        assert_eq!(first.category, Category::Image);
        assert!(tree.path_of(root.children[0]).ends_with("photos"));
    }

    #[test]
    fn view_groups_small_items() {
        let dir = sample();
        let tree = scan_sample(&dir);
        let view = tree.view(tree.root(), 3, 0.2).unwrap();
        // Only photos (>20%) survives on its own, the rest is grouped.
        assert_eq!(view.children[0].name, "photos");
        let last = view.children.last().unwrap();
        assert_eq!(last.kind, "group");
        assert!(last.items >= 2);
    }

    #[test]
    fn largest_files_and_search() {
        let dir = sample();
        let tree = scan_sample(&dir);
        let top = tree.largest_files(tree.root(), 2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].name, "a.jpg");
        assert_eq!(top[1].name, "b.jpg");
        assert!(top[0].rel_path.contains("2024"));

        let hits = tree.search(tree.root(), "SONG", 10);
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].name, "song.mp3");
    }

    #[test]
    fn remove_updates_ancestors() {
        let dir = sample();
        let mut tree = scan_sample(&dir);
        let before = tree.stats();
        let photos = tree.get(0).unwrap().children[0];
        let year = tree.get(photos).unwrap().children[0];
        let year_size = tree.get(year).unwrap().size;
        let freed = tree.remove(year).unwrap();
        assert_eq!(freed, year_size);
        let after = tree.stats();
        assert_eq!(after.size, before.size - year_size);
        assert_eq!(after.files, before.files - 2);
        assert!(tree.get(year).is_none());
        let cats = tree.category_totals(0);
        assert!(cats[Category::Image.index()] < 100_000);
        assert!(tree.remove(0).is_none());
    }

    #[test]
    fn hidden_space_is_attached_to_root() {
        let dir = sample();
        let mut tree = scan_sample(&dir);
        let size = tree.stats().size;
        tree.add_hidden_space(10_000_000);
        assert_eq!(tree.stats().size, size + 10_000_000);
        let root = tree.get(0).unwrap();
        assert_eq!(tree.get(root.children[0]).unwrap().kind, Kind::Hidden);
    }

    #[cfg(unix)]
    #[test]
    fn hard_links_are_counted_once() {
        let dir = sample();
        fs::hard_link(
            dir.path().join("photos/2024/a.jpg"),
            dir.path().join("music/a-link.jpg"),
        )
        .unwrap();
        let tree = scan_sample(&dir);
        let images = tree.category_totals(0)[Category::Image.index()];
        assert!(images < 700_000, "hard link counted twice: {images}");
    }

    #[test]
    fn excluded_paths_are_skipped() {
        let dir = sample();
        let progress = Progress::new();
        let opts = ScanOptions {
            excluded: vec![dir.path().join("photos")],
            ..Default::default()
        };
        let tree = scan(dir.path(), &opts, &progress).unwrap();
        assert_eq!(tree.stats().files, 2);
    }

    /// Regression: unreadable folders (normal on macOS without Full Disk
    /// Access) must not make a finished scan look unfinished.
    #[cfg(unix)]
    #[test]
    fn unreadable_folders_do_not_mark_the_scan_unfinished() {
        use std::os::unix::fs::PermissionsExt;
        let dir = sample();
        let locked = dir.path().join("locked");
        fs::create_dir(&locked).unwrap();
        write_file(&locked.join("secret.bin"), 1000);
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o000)).unwrap();
        let readable = fs::read_dir(&locked).is_ok(); // true when running as root
        let tree = scan_sample(&dir);
        fs::set_permissions(&locked, fs::Permissions::from_mode(0o755)).unwrap();
        assert!(tree.is_complete());
        if !readable {
            assert_eq!(tree.stats().unreadable, 1);
            let file = tempfile::tempdir().unwrap().path().join("s.2ds");
            assert!(store::save(&tree, &file, 0).unwrap().complete);
            assert!(store::load(&file).unwrap().is_complete());
        }
    }

    #[test]
    fn cancelled_scan_returns_an_incomplete_tree() {
        let dir = sample();
        let progress = Progress::new();
        progress.cancel();
        let tree = scan(dir.path(), &ScanOptions::default(), &progress).unwrap();
        assert!(!tree.is_complete());
        assert!(scan_sample(&dir).is_complete());
    }

    /// Compares two trees by path, size and kind.
    fn flat(tree: &Tree) -> Vec<(String, u64, Kind)> {
        let mut out = Vec::new();
        let mut stack = vec![tree.root()];
        while let Some(id) = stack.pop() {
            let n = tree.get(id).unwrap();
            out.push((tree.path_of(id).display().to_string(), n.size, n.kind));
            stack.extend(n.children.iter().copied());
        }
        out.sort();
        out
    }

    #[test]
    fn saved_scans_load_back_identically() {
        let dir = sample();
        let tree = scan_sample(&dir);
        let elsewhere = tempfile::tempdir().unwrap();
        let file = elsewhere.path().join("scan.2ds");
        let summary = store::save(&tree, &file, 1234).unwrap();
        assert!(summary.complete);
        assert_eq!(summary.files, tree.stats().files);
        assert_eq!(store::read_summary(&file).unwrap().elapsed_ms, 1234);

        let loaded = store::load(&file).unwrap();
        assert_eq!(flat(&loaded), flat(&tree));
        assert_eq!(loaded.stats().files, tree.stats().files);
        assert_eq!(loaded.stats().size, tree.stats().size);
        assert_eq!(loaded.category_totals(0), tree.category_totals(0));
        assert_eq!(loaded.scanned_at(), tree.scanned_at());
        assert!(loaded.is_complete());
        assert_eq!(loaded.largest_files(0, 1)[0].name, "a.jpg");
    }

    #[test]
    fn rescans_with_a_previous_tree_match_a_fresh_scan() {
        let dir = sample();
        let previous = Arc::new(scan_sample(&dir));

        // Change things: a new file, a removed file, a grown big file.
        write_file(&dir.path().join("photos/2024/c.jpg"), 70_000);
        fs::remove_file(dir.path().join("music/song.mp3")).unwrap();
        write_file(&dir.path().join("photos/cover.png"), 20 * 1024 * 1024);

        let progress = Progress::new();
        let opts = ScanOptions {
            previous: Some(previous),
            ..Default::default()
        };
        let incremental = scan(dir.path(), &opts, &progress).unwrap();
        assert_eq!(flat(&incremental), flat(&scan_sample(&dir)));
    }

    #[cfg(unix)]
    #[test]
    fn unchanged_folders_are_reused_and_interrupted_scans_continue() {
        use std::time::{Duration, SystemTime};
        let dir = sample();
        // Make every folder look older than the scan, as on a real disk.
        let old = SystemTime::now() - Duration::from_secs(3600);
        for d in ["", "photos", "photos/2024", "music", "empty"] {
            fs::File::open(dir.path().join(d))
                .unwrap()
                .set_modified(old)
                .unwrap();
        }
        let first = Arc::new(scan_sample(&dir));

        let progress = Progress::new();
        let opts = ScanOptions {
            previous: Some(first.clone()),
            ..Default::default()
        };
        let second = scan(dir.path(), &opts, &progress).unwrap();
        assert_eq!(
            progress.reused.load(std::sync::atomic::Ordering::Relaxed),
            5
        );
        assert_eq!(flat(&second), flat(&first));

        // An interrupted scan, saved and loaded, is continued by the next one.
        let cancelled = Progress::new();
        cancelled.cancel();
        let partial = scan(dir.path(), &ScanOptions::default(), &cancelled).unwrap();
        let elsewhere = tempfile::tempdir().unwrap();
        let file = elsewhere.path().join("partial.2ds");
        store::save(&partial, &file, 0).unwrap();
        let partial = Arc::new(store::load(&file).unwrap());
        assert!(!partial.is_complete());
        let opts = ScanOptions {
            previous: Some(partial),
            ..Default::default()
        };
        let resumed = scan(dir.path(), &opts, &Progress::new()).unwrap();
        assert!(resumed.is_complete());
        assert_eq!(flat(&resumed), flat(&first));
    }
}
