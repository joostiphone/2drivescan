// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cleanup;
mod volumes;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use scan_core::{
    store, ChangeItem, Comparison, Crumb, DupProgress, DupSnapshot, ItemHit, NodeId, Progress,
    SavedScan, Tree, TreeStats, ViewNode,
};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WindowEvent};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::volumes::Volume;

/// Arcs narrower than this fraction of the full circle are grouped.
const MIN_ARC_FRACTION: f64 = 0.5 / 360.0;
const MAX_VIEW_DEPTH: u32 = 8;
/// How many saved scans are kept (the oldest are removed first).
const KEEP_SAVED_SCANS: usize = 12;

#[derive(Default)]
struct AppState {
    tree: RwLock<Option<Tree>>,
    /// Bumped whenever `tree` is replaced, so background work on an old tree
    /// can tell its results are stale.
    tree_gen: AtomicU64,
    /// The open tree compared with the previous scan of the same folder.
    compare: RwLock<Option<Arc<Comparison>>>,
    /// Running duplicate search: (job id, progress).
    dup: Mutex<Option<(u64, Arc<DupProgress>)>>,
    /// Held while a scan is written, so comparing waits for the previous
    /// scan to have been moved aside.
    save_lock: Mutex<()>,
    active: Mutex<Option<(u64, Arc<Progress>)>>,
    next_id: AtomicU64,
    /// Scan time of the tree in `tree`, kept for re-saving it after changes.
    elapsed_ms: AtomicU64,
    /// The app is closing and waits for a running scan to save its progress.
    quitting: AtomicBool,
    /// Update found by `check_update`, installed by `install_update`.
    pending_update: Mutex<Option<Update>>,
}

#[derive(Serialize)]
struct UpdateInfo {
    version: String,
    current_version: String,
}

#[derive(Serialize, Clone)]
struct UpdateProgress {
    downloaded: u64,
    total: Option<u64>,
}

type CmdResult<T> = Result<T, String>;

#[derive(Serialize)]
struct AppInfo {
    version: String,
    os: &'static str,
    arch: &'static str,
    home: Option<String>,
}

#[derive(Serialize, Clone)]
struct ProgressEvent {
    scan_id: u64,
    files: u64,
    dirs: u64,
    bytes: u64,
    errors: u64,
    /// Folders taken over unchanged from the previous scan.
    reused: u64,
    current: String,
    elapsed_ms: u64,
    /// A previous scan of this folder is being used.
    incremental: bool,
}

#[derive(Serialize, Clone)]
struct DoneEvent {
    scan_id: u64,
    stats: TreeStats,
    elapsed_ms: u64,
    volume: Option<Volume>,
    /// Opened from disk instead of scanned just now.
    saved: bool,
    /// When the previous scan of this folder was made, if it is kept for
    /// comparing (milliseconds since the Unix epoch).
    previous_ms: Option<i64>,
    /// Files carry modification dates (scans saved before 0.5 do not).
    file_dates: bool,
}

#[derive(Serialize, Clone)]
struct FailedEvent {
    scan_id: u64,
    cancelled: bool,
    message: String,
}

#[derive(Serialize)]
struct ViewResponse {
    root: ViewNode,
    crumbs: Vec<Crumb>,
    path: String,
}

#[derive(Serialize)]
struct TypeTotals {
    cats: [u64; scan_core::CATEGORY_COUNT],
    hidden: u64,
    total: u64,
}

#[derive(Serialize)]
struct ResolvedItem {
    id: NodeId,
    name: String,
    size: u64,
    kind: &'static str,
    cat: u8,
    path: String,
    /// True when an ancestor of this item is also in the list.
    covered: bool,
}

#[derive(Serialize)]
struct TrashFailure {
    id: NodeId,
    name: String,
    error: String,
}

#[derive(Serialize)]
struct TrashReport {
    removed: Vec<NodeId>,
    failed: Vec<TrashFailure>,
    freed: u64,
    stats: Option<TreeStats>,
}

fn with_tree<T>(state: &AppState, f: impl FnOnce(&Tree) -> CmdResult<T>) -> CmdResult<T> {
    let guard = state
        .tree
        .read()
        .map_err(|_| "state poisoned".to_string())?;
    let tree = guard.as_ref().ok_or_else(|| "no scan loaded".to_string())?;
    f(tree)
}

fn node_path_of(state: &AppState, id: NodeId) -> CmdResult<PathBuf> {
    with_tree(state, |t| {
        t.get(id)
            .ok_or_else(|| "item no longer exists".to_string())?;
        Ok(t.path_of(id))
    })
}

/// Replaces the open tree. Comparisons and duplicate searches on the old tree
/// are dropped.
fn set_tree(state: &AppState, tree: Option<Tree>) {
    cancel_dup(state);
    state.tree_gen.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut c) = state.compare.write() {
        *c = None;
    }
    if let Ok(mut slot) = state.tree.write() {
        *slot = tree;
    }
}

fn cancel_dup(state: &AppState) {
    if let Ok(dup) = state.dup.lock() {
        if let Some((_, p)) = &*dup {
            p.cancel();
        }
    }
}

/// The previous scan of a folder, kept for comparing (see [`save_scan`]).
fn previous_file(app: &AppHandle, root: &Path) -> Option<PathBuf> {
    scan_file(app, root).map(|f| f.with_extension("prev"))
}

fn previous_ms(app: &AppHandle, root: &Path) -> Option<i64> {
    previous_file(app, root)
        .and_then(|f| store::read_summary(&f).ok())
        .map(|s| s.scanned_at_ms)
}

/// The previous scan once a new scan (started at `scanned_at_ms`) has been
/// saved: the saved scan it replaces, or the one kept before.
fn previous_after_save(app: &AppHandle, root: &Path, scanned_at_ms: i64) -> Option<i64> {
    match scan_file(app, root).map(|f| store::read_summary(&f)) {
        Some(Ok(s)) if s.complete && s.scanned_at_ms != scanned_at_ms => Some(s.scanned_at_ms),
        _ => previous_ms(app, root),
    }
}

/// Folder where scans are saved.
fn scans_dir(app: &AppHandle) -> Option<PathBuf> {
    app.path()
        .app_local_data_dir()
        .ok()
        .map(|d| d.join("scans"))
}

/// File a scan of `root` is saved in (one per scanned folder).
fn scan_file(app: &AppHandle, root: &Path) -> Option<PathBuf> {
    // FNV-1a: stable across versions and platforms, unlike std's hasher.
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for b in root.to_string_lossy().as_bytes() {
        hash ^= *b as u64;
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    scans_dir(app).map(|d| d.join(format!("{hash:016x}.2ds")))
}

/// Saved scans, newest first, with their files.
fn saved_scan_files(app: &AppHandle) -> Vec<(PathBuf, SavedScan)> {
    let Some(dir) = scans_dir(app) else {
        return Vec::new();
    };
    let mut out: Vec<(PathBuf, SavedScan)> = std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|x| x == "2ds"))
                .filter_map(|p| store::read_summary(&p).ok().map(|s| (p, s)))
                .collect()
        })
        .unwrap_or_default();
    out.sort_by_key(|(_, s)| std::cmp::Reverse(s.scanned_at_ms));
    out
}

/// Saves a scan and removes the oldest saved scans beyond the limit.
///
/// When a new, different scan replaces a finished one, the finished one is
/// kept as `<hash>.prev` so the next scan can show what changed. Re-saving the
/// same scan (after moving items to the Trash) or saving an interrupted scan
/// over an interrupted one keeps the existing previous scan.
fn save_scan(app: &AppHandle, tree: &Tree, elapsed_ms: u64) {
    let state = app.state::<AppState>();
    let _saving = state.save_lock.lock();
    write_scan(app, tree, elapsed_ms);
}

/// [`save_scan`] for callers that already hold the save lock.
fn write_scan(app: &AppHandle, tree: &Tree, elapsed_ms: u64) {
    let Some(file) = scan_file(app, tree.root_path()) else {
        return;
    };
    if let Ok(existing) = store::read_summary(&file) {
        if existing.complete && existing.scanned_at_ms != tree.stats().scanned_at_ms {
            let _ = std::fs::rename(&file, file.with_extension("prev"));
        }
    }
    if let Err(e) = store::save(tree, &file, elapsed_ms) {
        eprintln!("could not save scan: {e}");
        return;
    }
    for (old, _) in saved_scan_files(app).into_iter().skip(KEEP_SAVED_SCANS) {
        let _ = std::fs::remove_file(old.with_extension("prev"));
        let _ = std::fs::remove_file(old);
    }
}

/// Re-saves the tree that is open (after items were moved to the trash).
fn resave_current(app: &AppHandle) {
    let state = app.state::<AppState>();
    let _saving = state.save_lock.lock();
    write_current(app);
}

/// [`resave_current`] for callers that already hold the save lock.
fn write_current(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Ok(guard) = state.tree.read() else {
        return;
    };
    if let Some(tree) = guard.as_ref() {
        write_scan(app, tree, state.elapsed_ms.load(Ordering::Relaxed));
    }
}

fn add_hidden_space(tree: &mut Tree, volume: Option<&Volume>) {
    if let Some(v) = volume {
        let used = v.total.saturating_sub(v.available);
        let hidden = used.saturating_sub(tree.stats().size);
        if hidden > 64 * 1024 * 1024 && hidden > used / 200 {
            tree.add_hidden_space(hidden);
        }
    }
}

/// Called when the app is about to close. A running scan is stopped and its
/// progress saved first, so the next scan can continue. Returns true when
/// closing has to wait for that.
fn defer_quit_for_scan(app: &AppHandle) -> bool {
    let state = app.state::<AppState>();
    if state.quitting.load(Ordering::Relaxed) {
        return false;
    }
    let progress = match state.active.lock() {
        Ok(active) => active.as_ref().map(|(_, p)| p.clone()),
        Err(_) => None,
    };
    let Some(progress) = progress else {
        return false;
    };
    state.quitting.store(true, Ordering::Relaxed);
    progress.cancel();
    // Never hang on a stuck file system: give saving a few seconds at most.
    let app = app.clone();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_secs(8));
        app.exit(0);
    });
    true
}

/// Asks GitHub whether a newer version exists. The endpoint and the public key
/// that update packages must be signed with are in tauri.conf.json.
#[tauri::command]
async fn check_update(app: AppHandle, state: State<'_, AppState>) -> CmdResult<Option<UpdateInfo>> {
    let mut builder = app.updater_builder();
    // For testing an update locally against a hand-made manifest.
    if let Ok(url) = std::env::var("TWODRIVESCAN_UPDATE_URL") {
        let url = url.parse().map_err(|e| format!("bad update URL: {e}"))?;
        builder = builder.endpoints(vec![url]).map_err(|e| e.to_string())?;
    }
    let updater = builder.build().map_err(|e| e.to_string())?;
    let update = updater.check().await.map_err(|e| e.to_string())?;
    let info = update.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        current_version: u.current_version.clone(),
    });
    *state.pending_update.lock().map_err(|_| "state poisoned")? = update;
    Ok(info)
}

/// Downloads the update found by `check_update`, verifies its signature,
/// installs it and restarts the app.
#[tauri::command]
async fn install_update(app: AppHandle, state: State<'_, AppState>) -> CmdResult<()> {
    if state.active.lock().map(|a| a.is_some()).unwrap_or(false) {
        return Err("A scan is running; update after it finishes.".into());
    }
    // Cloned, not taken: after a failed attempt the user can try again.
    let update = state
        .pending_update
        .lock()
        .map_err(|_| "state poisoned")?
        .clone()
        .ok_or("no update to install")?;
    let progress_app = app.clone();
    let mut downloaded = 0u64;
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress_app.emit("update-progress", UpdateProgress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|e| e.to_string())?;
    app.restart()
}

#[tauri::command]
fn app_info(app: AppHandle) -> AppInfo {
    AppInfo {
        version: app.package_info().version.to_string(),
        os: std::env::consts::OS,
        arch: std::env::consts::ARCH,
        home: app.path().home_dir().ok().map(|p| p.display().to_string()),
    }
}

#[tauri::command(async)]
fn list_volumes() -> Vec<Volume> {
    volumes::list()
}

#[tauri::command]
async fn pick_folder(app: AppHandle) -> Option<String> {
    app.dialog()
        .file()
        .set_title("Choose a folder to scan")
        .blocking_pick_folder()
        .and_then(|p| p.into_path().ok())
        .map(|p| p.display().to_string())
}

#[tauri::command(async)]
fn saved_scans(app: AppHandle) -> Vec<SavedScan> {
    saved_scan_files(&app).into_iter().map(|(_, s)| s).collect()
}

#[derive(Serialize)]
struct ScanStorage {
    /// Saved scans that can be opened.
    count: usize,
    /// Space the scans folder takes, including unreadable leftovers.
    bytes: u64,
    /// How many scans are kept before the oldest are removed.
    keep: usize,
}

/// Files in the scans folder: saved scans and temporary files of a save
/// that was interrupted.
fn scan_folder_files(app: &AppHandle) -> Vec<PathBuf> {
    let Some(dir) = scans_dir(app) else {
        return Vec::new();
    };
    std::fs::read_dir(dir)
        .map(|rd| {
            rd.flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.extension()
                        .is_some_and(|x| x == "2ds" || x == "prev" || x == "tmp")
                })
                .collect()
        })
        .unwrap_or_default()
}

fn scan_storage_of(app: &AppHandle) -> ScanStorage {
    ScanStorage {
        count: saved_scan_files(app).len(),
        bytes: scan_folder_files(app)
            .iter()
            .filter_map(|p| std::fs::metadata(p).ok())
            .map(|m| m.len())
            .sum(),
        keep: KEEP_SAVED_SCANS,
    }
}

/// How many scans are saved and how much space they take.
#[tauri::command(async)]
fn scan_storage(app: AppHandle) -> ScanStorage {
    scan_storage_of(&app)
}

/// Removes every saved scan. These are 2DriveScan's own files, not the
/// user's, so they are deleted directly (like the oldest scans beyond
/// `KEEP_SAVED_SCANS`) instead of being moved to the Trash. The next scan of
/// each folder reads everything again.
#[tauri::command(async)]
fn clear_saved_scans(app: AppHandle, state: State<'_, AppState>) -> CmdResult<ScanStorage> {
    if state.active.lock().map(|a| a.is_some()).unwrap_or(false) {
        return Err("A scan is running; try again after it finishes.".into());
    }
    let mut first_error = None;
    for file in scan_folder_files(&app) {
        if let Err(e) = std::fs::remove_file(&file) {
            first_error.get_or_insert(e.to_string());
        }
    }
    match first_error {
        Some(e) => Err(format!("not every saved scan could be removed: {e}")),
        None => Ok(scan_storage_of(&app)),
    }
}

/// Opens the saved scan of a folder without scanning.
#[tauri::command(async)]
fn open_saved(app: AppHandle, state: State<'_, AppState>, path: String) -> CmdResult<DoneEvent> {
    let root = volumes::normalize(&path)?;
    let file = scan_file(&app, &root).ok_or("no saved scan")?;
    let mut tree = store::load(&file).map_err(|e| format!("could not open the saved scan: {e}"))?;
    let elapsed_ms = store::read_summary(&file)
        .map(|s| s.elapsed_ms)
        .unwrap_or(0);
    let volume = volumes::plan_for(&root).volume;
    add_hidden_space(&mut tree, volume.as_ref());
    let stats = tree.stats();
    let file_dates = tree.has_file_dates();
    state.elapsed_ms.store(elapsed_ms, Ordering::Relaxed);
    set_tree(&state, Some(tree));
    Ok(DoneEvent {
        scan_id: 0,
        stats,
        elapsed_ms,
        volume,
        saved: true,
        previous_ms: previous_ms(&app, &root),
        file_dates,
    })
}

/// Starts a scan. Unless `full` is set, the saved scan of the same folder is
/// used to skip folders that did not change, and an interrupted scan
/// continues where it stopped.
#[tauri::command(async)]
fn start_scan(
    app: AppHandle,
    state: State<'_, AppState>,
    path: String,
    full: Option<bool>,
    exclude: Option<Vec<String>>,
) -> CmdResult<u64> {
    let full = full.unwrap_or(false);
    let root = volumes::normalize(&path)?;
    let mut plan = volumes::plan_for(&root);
    // Folders the user excluded in Settings, as far as they are inside this scan.
    for e in exclude.unwrap_or_default() {
        if let Ok(p) = volumes::normalize(&e) {
            if p != root && p.starts_with(&root) && !plan.opts.excluded.contains(&p) {
                plan.opts.excluded.push(p);
            }
        }
    }
    plan.opts.excluded.sort();
    let scan_id = state.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    let progress = Arc::new(Progress::new());
    {
        let mut active = state.active.lock().map_err(|_| "state poisoned")?;
        if let Some((_, old)) = active.replace((scan_id, progress.clone())) {
            old.cancel();
        }
    }
    // Free the previous tree right away; scans can be big.
    set_tree(&state, None);
    std::thread::Builder::new()
        .name("scan-coordinator".into())
        .stack_size(64 * 1024 * 1024)
        .spawn(move || run_scan(app, scan_id, root, plan, progress, full))
        .map_err(|e| e.to_string())?;
    Ok(scan_id)
}

fn run_scan(
    app: AppHandle,
    scan_id: u64,
    root: PathBuf,
    plan: volumes::Plan,
    progress: Arc<Progress>,
    full: bool,
) {
    let started = Instant::now();
    let mut opts = plan.opts.clone();
    if !full {
        opts.previous = scan_file(&app, &root)
            .filter(|f| f.exists())
            .and_then(|f| store::load(&f).ok())
            .filter(|t| t.root_path() == root)
            .map(Arc::new);
    }
    let incremental = opts.previous.is_some();

    let finished = Arc::new(AtomicBool::new(false));
    let ticker = {
        let app = app.clone();
        let progress = progress.clone();
        let finished = finished.clone();
        std::thread::spawn(move || {
            while !finished.load(Ordering::Relaxed) {
                let s = progress.snapshot();
                let _ = app.emit(
                    "scan-progress",
                    ProgressEvent {
                        scan_id,
                        files: s.files,
                        dirs: s.dirs,
                        bytes: s.bytes,
                        errors: s.errors,
                        reused: s.reused,
                        current: s.current,
                        elapsed_ms: started.elapsed().as_millis() as u64,
                        incremental,
                    },
                );
                std::thread::sleep(Duration::from_millis(90));
            }
        })
    };

    let result = scan_core::scan(&root, &opts, &progress);
    drop(opts); // Free the previous scan before keeping the new one.
    finished.store(true, Ordering::Relaxed);
    let _ = ticker.join();

    let state = app.state::<AppState>();
    let is_current = state
        .active
        .lock()
        .map(|a| matches!(&*a, Some((id, _)) if *id == scan_id))
        .unwrap_or(false);
    if !is_current {
        return; // Superseded by a newer scan.
    }
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(mut tree) if tree.is_complete() => {
            add_hidden_space(&mut tree, plan.volume.as_ref());
            let stats = tree.stats();
            state.elapsed_ms.store(elapsed_ms, Ordering::Relaxed);
            let previous_ms = previous_after_save(&app, &root, stats.scanned_at_ms);
            let file_dates = tree.has_file_dates();
            set_tree(&state, Some(tree));
            if let Ok(mut a) = state.active.lock() {
                *a = None;
            }
            // Hold the save lock until the scan is written, so a comparison
            // asked for right after this event waits for it.
            let saving = state.save_lock.lock();
            let _ = app.emit(
                "scan-done",
                DoneEvent {
                    scan_id,
                    stats,
                    elapsed_ms,
                    volume: plan.volume.clone(),
                    saved: false,
                    previous_ms,
                    file_dates,
                },
            );
            write_current(&app);
            drop(saving);
        }
        Ok(partial) => {
            // Stopped early: keep what was read so the next scan continues.
            save_scan(&app, &partial, elapsed_ms);
            drop(partial);
            if let Ok(mut a) = state.active.lock() {
                *a = None;
            }
            let _ = app.emit(
                "scan-failed",
                FailedEvent {
                    scan_id,
                    cancelled: true,
                    message: "scan cancelled".into(),
                },
            );
        }
        Err(err) => {
            if let Ok(mut a) = state.active.lock() {
                *a = None;
            }
            let _ = app.emit(
                "scan-failed",
                FailedEvent {
                    scan_id,
                    cancelled: false,
                    message: err.to_string(),
                },
            );
        }
    }
    if state.quitting.load(Ordering::Relaxed) {
        app.exit(0);
    }
}

#[tauri::command]
fn cancel_scan(state: State<'_, AppState>) {
    if let Ok(active) = state.active.lock() {
        if let Some((_, p)) = &*active {
            p.cancel();
        }
    }
}

fn comparison(state: &AppState) -> Option<Arc<Comparison>> {
    state.compare.read().ok().and_then(|c| c.clone())
}

#[tauri::command(async)]
fn get_view(state: State<'_, AppState>, id: NodeId, depth: u32) -> CmdResult<ViewResponse> {
    let cmp = comparison(&state);
    with_tree(&state, |t| {
        let mut root = t
            .view(id, depth.min(MAX_VIEW_DEPTH), MIN_ARC_FRACTION)
            .ok_or_else(|| "item no longer exists".to_string())?;
        if let Some(c) = &cmp {
            c.annotate(t, &mut root);
        }
        Ok(ViewResponse {
            root,
            crumbs: t.crumbs(id),
            path: t.path_of(id).display().to_string(),
        })
    })
}

#[tauri::command(async)]
fn list_children(state: State<'_, AppState>, id: NodeId, limit: usize) -> CmdResult<Vec<ViewNode>> {
    let cmp = comparison(&state);
    with_tree(&state, |t| {
        let mut list = t.list_children(id, limit.clamp(1, 5000));
        if let Some(c) = &cmp {
            list.iter_mut().for_each(|v| c.annotate(t, v));
        }
        Ok(list)
    })
}

/// The largest files below `id`; with `older_than_days`, only files not
/// modified for that many days.
#[tauri::command(async)]
fn largest_files(
    state: State<'_, AppState>,
    id: NodeId,
    limit: usize,
    older_than_days: Option<u32>,
) -> CmdResult<Vec<ItemHit>> {
    let before = older_than_days.filter(|&d| d > 0).map(|d| {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos() as i64);
        now - i64::from(d) * 86_400 * 1_000_000_000
    });
    with_tree(&state, |t| {
        Ok(t.largest_files_before(id, limit.clamp(1, 1000), before))
    })
}

#[derive(Serialize)]
struct CompareInfo {
    previous_ms: i64,
    /// Change of the whole scan.
    delta: i64,
}

/// Compares the open scan with the previous scan of the same folder (loaded
/// from disk the first time). None when there is no previous scan.
#[tauri::command(async)]
fn load_comparison(app: AppHandle, state: State<'_, AppState>) -> CmdResult<Option<CompareInfo>> {
    let info = |c: &Comparison, t: &Tree| CompareInfo {
        previous_ms: c.previous_ms,
        delta: c.delta(t, t.root()).unwrap_or(0),
    };
    if let Some(c) = comparison(&state) {
        return with_tree(&state, |t| Ok(Some(info(&c, t))));
    }
    let gen = state.tree_gen.load(Ordering::Relaxed);
    let root = with_tree(&state, |t| Ok(t.root_path().to_path_buf()))?;
    // Wait until a scan that just finished has been saved (and the one it
    // replaced moved aside as the previous scan).
    drop(state.save_lock.lock());
    let Some(file) = previous_file(&app, &root).filter(|f| f.exists()) else {
        return Ok(None);
    };
    let earlier =
        store::load(&file).map_err(|e| format!("could not read the previous scan: {e}"))?;
    if earlier.root_path() != root {
        return Ok(None);
    }
    let cmp = Arc::new(with_tree(&state, |t| Ok(t.compare_with(&earlier)))?);
    drop(earlier);
    if state.tree_gen.load(Ordering::Relaxed) != gen {
        return Err("the scan changed".into());
    }
    if let Ok(mut slot) = state.compare.write() {
        *slot = Some(cmp.clone());
    }
    with_tree(&state, |t| Ok(Some(info(&cmp, t))))
}

#[derive(Serialize)]
struct ChangesResponse {
    previous_ms: i64,
    /// Change of the folder itself.
    delta: i64,
    size: u64,
    items: Vec<ChangeItem>,
}

/// Direct children of `id` that grew or shrank since the previous scan.
#[tauri::command(async)]
fn changes(
    state: State<'_, AppState>,
    id: NodeId,
    limit: usize,
) -> CmdResult<Option<ChangesResponse>> {
    let Some(cmp) = comparison(&state) else {
        return Ok(None);
    };
    with_tree(&state, |t| {
        let node = t
            .get(id)
            .ok_or_else(|| "item no longer exists".to_string())?;
        Ok(Some(ChangesResponse {
            previous_ms: cmp.previous_ms,
            delta: cmp.delta(t, id).unwrap_or(0),
            size: node.size,
            items: cmp.changes(t, id, limit.clamp(1, 5000)),
        }))
    })
}

#[derive(Serialize, Clone)]
struct DupFile {
    id: NodeId,
    name: String,
    path: String,
    size: u64,
    cat: u8,
}

#[derive(Serialize, Clone)]
struct DupGroupOut {
    len: u64,
    size: u64,
    wasted: u64,
    files: Vec<DupFile>,
}

#[derive(Serialize, Clone)]
struct DupProgressEvent {
    job: u64,
    #[serde(flatten)]
    progress: DupSnapshot,
}

#[derive(Serialize, Clone)]
struct DupDoneEvent {
    job: u64,
    root: NodeId,
    cancelled: bool,
    groups: Vec<DupGroupOut>,
    /// Number of groups found (only the largest are sent).
    total_groups: usize,
    /// Space all extra copies take.
    wasted: u64,
}

const MAX_DUP_GROUPS: usize = 400;

/// Starts looking for duplicate files below `id`. Progress arrives as
/// `dup-progress` events and the result as one `dup-done` event.
#[tauri::command(async)]
fn find_duplicates(
    app: AppHandle,
    state: State<'_, AppState>,
    id: NodeId,
    min_size: u64,
) -> CmdResult<u64> {
    let gen = state.tree_gen.load(Ordering::Relaxed);
    let candidates = with_tree(&state, |t| {
        t.get(id)
            .ok_or_else(|| "item no longer exists".to_string())?;
        Ok(scan_core::duplicate_candidates(t, id, min_size.max(1)))
    })?;
    let job = state.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    let progress = Arc::new(DupProgress::new());
    if let Ok(mut dup) = state.dup.lock() {
        if let Some((_, old)) = dup.replace((job, progress.clone())) {
            old.cancel();
        }
    }
    std::thread::Builder::new()
        .name("duplicates".into())
        .spawn(move || {
            let finished = Arc::new(AtomicBool::new(false));
            let ticker = {
                let (app, progress, finished) = (app.clone(), progress.clone(), finished.clone());
                std::thread::spawn(move || {
                    while !finished.load(Ordering::Relaxed) {
                        let _ = app.emit(
                            "dup-progress",
                            DupProgressEvent {
                                job,
                                progress: progress.snapshot(),
                            },
                        );
                        std::thread::sleep(Duration::from_millis(150));
                    }
                })
            };
            let groups = scan_core::compare_files(candidates, &progress);
            finished.store(true, Ordering::Relaxed);
            let _ = ticker.join();

            let state = app.state::<AppState>();
            let cancelled =
                progress.is_cancelled() || state.tree_gen.load(Ordering::Relaxed) != gen;
            let wasted = groups.iter().map(|g| g.wasted()).sum();
            let total_groups = groups.len();
            let out = if cancelled {
                Vec::new()
            } else {
                with_tree(&state, |t| {
                    Ok(groups
                        .iter()
                        .take(MAX_DUP_GROUPS)
                        .map(|g| DupGroupOut {
                            len: g.len,
                            size: g.size,
                            wasted: g.wasted(),
                            files: g
                                .ids
                                .iter()
                                .filter_map(|&f| {
                                    let n = t.get(f)?;
                                    Some(DupFile {
                                        id: f,
                                        name: n.name.to_string(),
                                        path: t.path_of(f).display().to_string(),
                                        size: n.size,
                                        cat: n.category as u8,
                                    })
                                })
                                .collect(),
                        })
                        .collect())
                })
                .unwrap_or_default()
            };
            if let Ok(mut dup) = state.dup.lock() {
                if matches!(&*dup, Some((j, _)) if *j == job) {
                    *dup = None;
                }
            }
            let _ = app.emit(
                "dup-done",
                DupDoneEvent {
                    job,
                    root: id,
                    cancelled,
                    groups: out,
                    total_groups,
                    wasted,
                },
            );
        })
        .map_err(|e| e.to_string())?;
    Ok(job)
}

#[tauri::command]
fn cancel_duplicates(state: State<'_, AppState>) {
    cancel_dup(&state);
}

/// Cleanup suggestions for the open scan.
#[tauri::command(async)]
fn cleanup_suggestions(
    app: AppHandle,
    state: State<'_, AppState>,
) -> CmdResult<Vec<cleanup::Suggestion>> {
    let home = app.path().home_dir().ok();
    with_tree(&state, |t| Ok(cleanup::suggestions(t, home.as_deref())))
}

/// Saves the folders (and files of 1 MB or more) below `id` as a CSV file the
/// user picks. Returns the file's path, or None when the dialog was cancelled.
#[tauri::command]
async fn export_csv(
    app: AppHandle,
    state: State<'_, AppState>,
    id: NodeId,
    file_name: String,
) -> CmdResult<Option<String>> {
    let Some(target) = app
        .dialog()
        .file()
        .set_file_name(file_name)
        .add_filter("CSV", &["csv"])
        .blocking_save_file()
        .and_then(|p| p.into_path().ok())
    else {
        return Ok(None);
    };
    let file = std::fs::File::create(&target).map_err(|e| e.to_string())?;
    let mut out = std::io::BufWriter::new(file);
    with_tree(&state, |t| {
        t.get(id)
            .ok_or_else(|| "item no longer exists".to_string())?;
        cleanup::write_csv(t, id, &mut out).map_err(|e| e.to_string())
    })?;
    std::io::Write::flush(&mut out).map_err(|e| e.to_string())?;
    Ok(Some(target.display().to_string()))
}

#[tauri::command(async)]
fn search(
    state: State<'_, AppState>,
    id: NodeId,
    query: String,
    limit: usize,
) -> CmdResult<Vec<ItemHit>> {
    with_tree(&state, |t| Ok(t.search(id, &query, limit.clamp(1, 1000))))
}

#[tauri::command(async)]
fn type_totals(state: State<'_, AppState>, id: NodeId) -> CmdResult<TypeTotals> {
    with_tree(&state, |t| {
        let node = t
            .get(id)
            .ok_or_else(|| "item no longer exists".to_string())?;
        Ok(TypeTotals {
            cats: t.category_totals(id),
            hidden: t.hidden_below(id),
            total: node.size,
        })
    })
}

#[tauri::command(async)]
fn node_path(state: State<'_, AppState>, id: NodeId) -> CmdResult<String> {
    Ok(node_path_of(&state, id)?.display().to_string())
}

/// Ids whose ancestor is also present in `ids`.
fn covered_ids(tree: &Tree, ids: &[NodeId]) -> std::collections::HashSet<NodeId> {
    let set: std::collections::HashSet<NodeId> = ids.iter().copied().collect();
    ids.iter()
        .copied()
        .filter(|&id| {
            let chain = tree.ancestors(id);
            chain[..chain.len().saturating_sub(1)]
                .iter()
                .any(|a| set.contains(a))
        })
        .collect()
}

#[tauri::command(async)]
fn resolve_items(state: State<'_, AppState>, ids: Vec<NodeId>) -> CmdResult<Vec<ResolvedItem>> {
    with_tree(&state, |t| {
        let live: Vec<NodeId> = ids.into_iter().filter(|&id| t.get(id).is_some()).collect();
        let covered = covered_ids(t, &live);
        Ok(live
            .iter()
            .map(|&id| {
                let n = t.get(id).expect("checked above");
                ResolvedItem {
                    id,
                    name: n.name.to_string(),
                    size: n.size,
                    kind: scan_core::kind_str(n.kind),
                    cat: n.category as u8,
                    path: t.path_of(id).display().to_string(),
                    covered: covered.contains(&id),
                }
            })
            .collect())
    })
}

#[tauri::command(async)]
fn reveal_node(app: AppHandle, state: State<'_, AppState>, id: NodeId) -> CmdResult<()> {
    let path = node_path_of(&state, id)?;
    app.opener()
        .reveal_item_in_dir(path)
        .map_err(|e| e.to_string())
}

#[tauri::command(async)]
fn open_node(app: AppHandle, state: State<'_, AppState>, id: NodeId) -> CmdResult<()> {
    let path = node_path_of(&state, id)?;
    app.opener()
        .open_path(path.display().to_string(), None::<&str>)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn open_external(app: AppHandle, url: String) -> CmdResult<()> {
    const ALLOWED: &[&str] = &[
        "https://2drivescan.app",
        "https://github.com/joostiphone/2drivescan",
        "x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles",
    ];
    if !ALLOWED.iter().any(|a| url.starts_with(a)) {
        return Err("URL not allowed".into());
    }
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|e| e.to_string())
}

fn move_to_trash(path: &std::path::Path) -> Result<(), String> {
    #[allow(unused_mut)]
    let mut ctx = trash::TrashContext::default();
    #[cfg(target_os = "macos")]
    {
        use trash::macos::{DeleteMethod, TrashContextExtMacos};
        // Avoids the "wants to control Finder" automation prompt.
        ctx.set_delete_method(DeleteMethod::NsFileManager);
    }
    ctx.delete(path).map_err(|e| e.to_string())
}

#[tauri::command]
async fn trash_nodes(
    app: AppHandle,
    state: State<'_, AppState>,
    ids: Vec<NodeId>,
) -> CmdResult<TrashReport> {
    // Resolve paths first (read lock), then trash, then update the tree.
    let targets: Vec<(NodeId, String, PathBuf)> = with_tree(&state, |t| {
        let covered = covered_ids(t, &ids);
        Ok(ids
            .iter()
            .filter_map(|&id| {
                let n = t.get(id)?;
                if id == t.root() || n.kind == scan_core::Kind::Hidden || covered.contains(&id) {
                    return None;
                }
                Some((id, n.name.to_string(), t.path_of(id)))
            })
            .collect())
    })?;

    let mut removed = Vec::new();
    let mut failed = Vec::new();
    for (id, name, path) in targets {
        match tauri::async_runtime::spawn_blocking(move || move_to_trash(&path)).await {
            Ok(Ok(())) => removed.push(id),
            Ok(Err(error)) => failed.push(TrashFailure { id, name, error }),
            Err(e) => failed.push(TrashFailure {
                id,
                name,
                error: e.to_string(),
            }),
        }
    }

    let mut freed = 0;
    cancel_dup(&state);
    let stats = {
        let mut guard = state
            .tree
            .write()
            .map_err(|_| "state poisoned".to_string())?;
        guard.as_mut().map(|tree| {
            for &id in &removed {
                freed += tree.remove(id).unwrap_or(0);
            }
            tree.stats()
        })
    };
    if !removed.is_empty() {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || resave_current(&app));
    }
    Ok(TrashReport {
        removed,
        failed,
        freed,
        stats,
    })
}

fn main() {
    #[cfg(target_os = "linux")]
    {
        // WebKitGTK's DMA-BUF renderer shows a blank window on many NVIDIA
        // setups; fall back to the stable path there.
        if std::env::var_os("WEBKIT_DISABLE_DMABUF_RENDERER").is_none()
            && std::path::Path::new("/proc/driver/nvidia").exists()
        {
            std::env::set_var("WEBKIT_DISABLE_DMABUF_RENDERER", "1");
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            app_info,
            list_volumes,
            pick_folder,
            saved_scans,
            scan_storage,
            clear_saved_scans,
            open_saved,
            load_comparison,
            changes,
            find_duplicates,
            cancel_duplicates,
            cleanup_suggestions,
            export_csv,
            start_scan,
            cancel_scan,
            get_view,
            list_children,
            largest_files,
            search,
            type_totals,
            node_path,
            resolve_items,
            reveal_node,
            open_node,
            open_external,
            trash_nodes,
            check_update,
            install_update,
        ])
        .on_window_event(|window, event| {
            // Closing the window during a scan: save its progress first.
            if let WindowEvent::CloseRequested { api, .. } = event {
                if defer_quit_for_scan(window.app_handle()) {
                    api.prevent_close();
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("error while starting 2DriveScan")
        .run(|app, event| {
            // Quitting from the menu (⌘Q) during a scan: same as closing.
            if let RunEvent::ExitRequested { api, .. } = event {
                if defer_quit_for_scan(app) {
                    api.prevent_exit();
                }
            }
        });
}
