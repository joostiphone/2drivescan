//! Core of 2DriveScan: a fast parallel disk usage scanner and the tree model
//! the UI navigates.

pub mod category;
pub mod scanner;
pub mod tree;

pub use category::{categorize, Category, CATEGORY_COUNT};
pub use scanner::{scan, Progress, ProgressSnapshot, ScanError, ScanOptions};
pub use tree::{kind_str, Crumb, ItemHit, Kind, Node, NodeId, Tree, TreeStats, ViewNode};

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;

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

    #[test]
    fn cancel_stops_scan() {
        let dir = sample();
        let progress = Progress::new();
        progress.cancel();
        assert!(matches!(
            scan(dir.path(), &ScanOptions::default(), &progress),
            Err(ScanError::Cancelled)
        ));
    }
}
