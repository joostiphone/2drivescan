//! Scan a folder from the command line:
//!   cargo run --release -p scan-core --example bench -- /path
//! Runs a full scan, saves and loads it, then a rescan that reuses it.
use std::sync::Arc;
use std::time::Instant;

use scan_core::{scan, store, Progress, ScanOptions};

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let root = std::fs::canonicalize(&path).expect("path");

    let t = Instant::now();
    let tree = scan(&root, &ScanOptions::default(), &Progress::new()).expect("scan failed");
    let s = tree.stats();
    println!(
        "full scan: {} files, {} dirs, {:.2} GB, {} unreadable in {:.2?}",
        s.files,
        s.dirs,
        s.size as f64 / 1e9,
        s.unreadable,
        t.elapsed()
    );

    let file = std::env::temp_dir().join("2drivescan-bench.2ds");
    let t = Instant::now();
    store::save(&tree, &file, 0).expect("save");
    let bytes = std::fs::metadata(&file).map(|m| m.len()).unwrap_or(0);
    println!("saved: {:.1} MB in {:.2?}", bytes as f64 / 1e6, t.elapsed());
    let t = Instant::now();
    let loaded = store::load(&file).expect("load");
    println!("loaded: {} nodes in {:.2?}", loaded.len(), t.elapsed());

    let progress = Progress::new();
    let opts = ScanOptions {
        previous: Some(Arc::new(loaded)),
        ..Default::default()
    };
    let t = Instant::now();
    let again = scan(&root, &opts, &progress).expect("rescan failed");
    println!(
        "rescan: {} files, {} of {} folders reused in {:.2?}",
        again.stats().files,
        progress.reused.load(std::sync::atomic::Ordering::Relaxed),
        again.stats().dirs,
        t.elapsed()
    );
    let _ = std::fs::remove_file(&file);
}
