//! Scan a folder from the command line: `cargo run --release --example bench -- /path`
use std::time::Instant;

fn main() {
    let path = std::env::args().nth(1).unwrap_or_else(|| ".".into());
    let progress = scan_core::Progress::new();
    let started = Instant::now();
    let tree = scan_core::scan(path.as_ref(), &Default::default(), &progress).expect("scan failed");
    let stats = tree.stats();
    println!(
        "{}: {} files, {} dirs, {:.2} GB, {} unreadable in {:.2?}",
        stats.root_path,
        stats.files,
        stats.dirs,
        stats.size as f64 / 1e9,
        stats.unreadable,
        started.elapsed()
    );
    for hit in tree.largest_files(tree.root(), 5) {
        println!("  {:>10.1} MB  {}", hit.size as f64 / 1e6, hit.rel_path);
    }
}
