//! Core of 2DriveScan: a fast parallel disk usage scanner and the tree model
//! the UI navigates.

pub mod category;
pub mod duplicates;
mod listing;
pub mod scanner;
pub mod store;
pub mod tree;

pub use category::{categorize, Category, CATEGORY_COUNT};
pub use duplicates::{
    compare_files, duplicate_candidates, find_duplicates, DupGroup, DupProgress, DupSnapshot,
};
pub use listing::disable_cloud_downloads;
pub use scanner::{scan, Progress, ProgressSnapshot, ScanError, ScanOptions};
pub use store::SavedScan;
pub use tree::{
    kind_str, ChangeItem, Comparison, Crumb, ItemHit, Kind, Node, NodeId, Tree, TreeStats, ViewNode,
};

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

    fn set_age(path: &std::path::Path, days: u64) {
        use std::time::{Duration, SystemTime};
        let t = SystemTime::now() - Duration::from_secs(days * 86_400);
        fs::File::options()
            .write(true)
            .open(path)
            .unwrap()
            .set_modified(t)
            .unwrap();
    }

    #[test]
    fn files_have_dates_and_the_age_filter_uses_them() {
        let dir = sample();
        set_age(&dir.path().join("photos/2024/b.jpg"), 400);
        let tree = scan_sample(&dir);
        assert!(tree.has_file_dates());
        let all = tree.largest_files(0, 10);
        assert_eq!(all.len(), 5);
        assert!(all.iter().all(|h| h.modified_ms > 0));

        let year_ago = (std::time::SystemTime::now() - std::time::Duration::from_secs(365 * 86_400))
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos() as i64;
        let old = tree.largest_files_before(0, 10, Some(year_ago));
        assert_eq!(old.len(), 1);
        assert_eq!(old[0].name, "b.jpg");

        // Dates survive saving and loading.
        let elsewhere = tempfile::tempdir().unwrap();
        let file = elsewhere.path().join("scan.2ds");
        store::save(&tree, &file, 0).unwrap();
        let loaded = store::load(&file).unwrap();
        assert!(loaded.has_file_dates());
        assert_eq!(
            loaded.largest_files_before(0, 10, Some(year_ago))[0].name,
            "b.jpg"
        );
    }

    #[test]
    fn scans_saved_by_older_versions_load_but_are_not_reused() {
        let dir = sample();
        let tree = scan_sample(&dir);
        let elsewhere = tempfile::tempdir().unwrap();
        let file = elsewhere.path().join("v1.2ds");
        store::tests::save_version(&tree, &file, 1).unwrap();
        let old = store::load(&file).unwrap();
        assert_eq!(flat(&old), flat(&tree));
        assert!(!old.has_file_dates());
        assert!(old.largest_files(0, 1)[0].modified_ms == 0);

        // Without file dates the next scan reads everything again.
        let progress = Progress::new();
        let opts = ScanOptions {
            previous: Some(Arc::new(old)),
            ..Default::default()
        };
        let again = scan(dir.path(), &opts, &progress).unwrap();
        assert_eq!(
            progress.reused.load(std::sync::atomic::Ordering::Relaxed),
            0
        );
        assert!(again.has_file_dates());
    }

    #[cfg(unix)]
    #[test]
    fn changed_exclusions_make_the_next_scan_read_everything() {
        use std::time::{Duration, SystemTime};
        let dir = sample();
        let old = SystemTime::now() - Duration::from_secs(3600);
        for d in ["", "photos", "photos/2024", "music", "empty"] {
            fs::File::open(dir.path().join(d))
                .unwrap()
                .set_modified(old)
                .unwrap();
        }
        let opts = ScanOptions {
            excluded: vec![dir.path().join("photos")],
            ..Default::default()
        };
        let first = Arc::new(scan(dir.path(), &opts, &Progress::new()).unwrap());
        assert_eq!(first.excluded(), &[dir.path().join("photos")]);

        // Same exclusions: reused. No exclusions any more: photos must come back.
        let progress = Progress::new();
        let same = ScanOptions {
            previous: Some(first.clone()),
            ..opts.clone()
        };
        scan(dir.path(), &same, &progress).unwrap();
        assert!(progress.reused.load(std::sync::atomic::Ordering::Relaxed) > 0);

        let progress = Progress::new();
        let none = ScanOptions {
            previous: Some(first),
            ..Default::default()
        };
        let tree = scan(dir.path(), &none, &progress).unwrap();
        assert_eq!(
            progress.reused.load(std::sync::atomic::Ordering::Relaxed),
            0
        );
        assert_eq!(tree.stats().files, 5);
    }

    #[test]
    fn comparing_with_the_previous_scan() {
        let dir = sample();
        let before = scan_sample(&dir);
        write_file(&dir.path().join("photos/2024/c.jpg"), 70_000);
        fs::remove_file(dir.path().join("music/song.mp3")).unwrap();
        fs::create_dir(dir.path().join("new-folder")).unwrap();
        write_file(&dir.path().join("new-folder/x.bin"), 30_000);
        let after = scan_sample(&dir);
        let cmp = after.compare_with(&before);

        let root_delta = cmp.delta(&after, 0).unwrap();
        assert_eq!(
            root_delta,
            after.stats().size as i64 - before.stats().size as i64
        );
        let photos = after.find_path(&dir.path().join("photos")).unwrap();
        let c = after
            .find_path(&dir.path().join("photos/2024/c.jpg"))
            .unwrap();
        let music = after.find_path(&dir.path().join("music")).unwrap();
        let newdir = after.find_path(&dir.path().join("new-folder")).unwrap();
        assert!(cmp.delta(&after, photos).unwrap() > 0);
        assert!(cmp.is_new(&after, c));
        assert!(cmp.is_new(&after, newdir));
        assert!(!cmp.is_new(&after, photos));
        assert!(cmp.delta(&after, music).unwrap() < 0);
        let notes = after.find_path(&dir.path().join("notes.txt")).unwrap();
        assert_eq!(cmp.delta(&after, notes), Some(0));

        // Root children that changed, biggest change first; unchanged ones left out.
        let changes = cmp.changes(&after, 0, 10);
        assert_eq!(changes.len(), 3);
        assert!(changes[0].delta.unsigned_abs() >= changes[1].delta.unsigned_abs());
        assert!(changes
            .iter()
            .all(|c| c.name != "notes.txt" && c.name != "empty"));

        let mut view = after.view(0, 3, 0.0).unwrap();
        cmp.annotate(&after, &mut view);
        assert_eq!(view.delta, Some(root_delta));
        assert!(view
            .children
            .iter()
            .any(|c| c.new && c.name == "new-folder"));
    }

    #[test]
    fn finds_paths_and_folders_by_name() {
        let dir = sample();
        fs::create_dir_all(dir.path().join("app/node_modules/x/node_modules")).unwrap();
        fs::create_dir_all(dir.path().join("web/node_modules")).unwrap();
        let tree = scan_sample(&dir);
        assert_eq!(tree.find_path(dir.path()), Some(0));
        let year = tree.find_path(&dir.path().join("photos/2024")).unwrap();
        assert_eq!(&*tree.get(year).unwrap().name, "2024");
        assert!(tree.find_path(&dir.path().join("photos/nope")).is_none());
        assert!(tree.find_path(std::path::Path::new("/elsewhere")).is_none());
        // Outermost matches only.
        assert_eq!(tree.dirs_named(0, "node_modules").len(), 2);
    }

    fn scan_core_copy(name: &str) -> bool {
        duplicates::looks_like_copy(std::path::Path::new(name))
    }

    #[test]
    fn finds_duplicate_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        fs::create_dir_all(root.join("a/b")).unwrap();
        let big: Vec<u8> = (0..200_000u32).map(|i| (i % 251) as u8).collect();
        let mut other = big.clone();
        other[100_000] ^= 0xff; // Same size, same start and end, different middle.
        fs::write(root.join("a/b/movie.mkv"), &big).unwrap();
        fs::write(root.join("movie copy.mkv"), &big).unwrap();
        fs::write(root.join("a/movie-edit.mkv"), &other).unwrap();
        fs::write(root.join("note.txt"), b"same small text").unwrap();
        fs::write(root.join("a/note.txt"), b"same small text").unwrap();
        fs::write(root.join("a/other.txt"), b"different text!").unwrap();
        #[cfg(unix)]
        fs::hard_link(root.join("a/b/movie.mkv"), root.join("a/hardlink.mkv")).unwrap();

        let tree = scan(root, &ScanOptions::default(), &Progress::new()).unwrap();
        let progress = DupProgress::new();
        let groups = find_duplicates(&tree, 0, 1, &progress);
        let names = |g: &DupGroup| -> Vec<String> {
            g.ids
                .iter()
                .map(|&id| tree.get(id).unwrap().name.to_string())
                .collect()
        };
        assert_eq!(
            groups.len(),
            2,
            "{:?}",
            groups.iter().map(names).collect::<Vec<_>>()
        );
        // The original (under either of its hard-linked names; the scanner
        // counts whichever it meets first) stays first; the copy comes after.
        let first = names(&groups[0]);
        assert_eq!(first.len(), 2, "{first:?}");
        assert!(
            first[0] == "movie.mkv" || first[0] == "hardlink.mkv",
            "{first:?}"
        );
        assert_eq!(first[1], "movie copy.mkv");
        for (name, copy) in [
            ("a/photo copy.jpg", true),
            ("photo copy 2.jpg", true),
            ("photo (1).jpg", true),
            ("photo - Copy.jpg", true),
            ("foto kopie.jpg", true),
            ("photo.jpg", false),
            ("Copy of plan.pdf", false),
            ("report (draft).pdf", false),
            ("2024 (1)", true),
        ] {
            assert_eq!(scan_core_copy(name), copy, "{name}");
        }
        assert_eq!(groups[0].len, 200_000);
        assert!(groups[0].wasted() > 0);
        assert_eq!(names(&groups[1]), vec!["note.txt", "note.txt"]);

        // A minimum size leaves the small ones out.
        assert_eq!(
            find_duplicates(&tree, 0, 100_000, &DupProgress::new()).len(),
            1
        );
    }
}
