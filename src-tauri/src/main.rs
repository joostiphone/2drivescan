// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod volumes;

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use scan_core::{store, Crumb, ItemHit, NodeId, Progress, SavedScan, Tree, TreeStats, ViewNode};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, RunEvent, State, WindowEvent};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::volumes::Volume;

/// Arcs narrower than this fraction of the full circle are grouped.
const MIN_ARC_FRACTION: f64 = 0.5 / 360.0;
const MAX_VIEW_DEPTH: u32 = 8;
/// How many saved scans are kept (the oldest are removed first).
const KEEP_SAVED_SCANS: usize = 12;

#[derive(Default)]
struct AppState {
    tree: RwLock<Option<Tree>>,
    active: Mutex<Option<(u64, Arc<Progress>)>>,
    next_id: AtomicU64,
    /// Scan time of the tree in `tree`, kept for re-saving it after changes.
    elapsed_ms: AtomicU64,
    /// The app is closing and waits for a running scan to save its progress.
    quitting: AtomicBool,
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
fn save_scan(app: &AppHandle, tree: &Tree, elapsed_ms: u64) {
    let Some(file) = scan_file(app, tree.root_path()) else {
        return;
    };
    if let Err(e) = store::save(tree, &file, elapsed_ms) {
        eprintln!("could not save scan: {e}");
        return;
    }
    for (old, _) in saved_scan_files(app).into_iter().skip(KEEP_SAVED_SCANS) {
        let _ = std::fs::remove_file(old);
    }
}

/// Re-saves the tree that is open (after items were moved to the trash).
fn resave_current(app: &AppHandle) {
    let state = app.state::<AppState>();
    let Ok(guard) = state.tree.read() else {
        return;
    };
    if let Some(tree) = guard.as_ref() {
        save_scan(app, tree, state.elapsed_ms.load(Ordering::Relaxed));
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
    state.elapsed_ms.store(elapsed_ms, Ordering::Relaxed);
    *state.tree.write().map_err(|_| "state poisoned")? = Some(tree);
    Ok(DoneEvent {
        scan_id: 0,
        stats,
        elapsed_ms,
        volume,
        saved: true,
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
) -> CmdResult<u64> {
    let full = full.unwrap_or(false);
    let root = volumes::normalize(&path)?;
    let plan = volumes::plan_for(&root);
    let scan_id = state.next_id.fetch_add(1, Ordering::Relaxed) + 1;
    let progress = Arc::new(Progress::new());
    {
        let mut active = state.active.lock().map_err(|_| "state poisoned")?;
        if let Some((_, old)) = active.replace((scan_id, progress.clone())) {
            old.cancel();
        }
    }
    // Free the previous tree right away; scans can be big.
    if let Ok(mut tree) = state.tree.write() {
        *tree = None;
    }
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
            if let Ok(mut slot) = state.tree.write() {
                *slot = Some(tree);
            }
            if let Ok(mut a) = state.active.lock() {
                *a = None;
            }
            let _ = app.emit(
                "scan-done",
                DoneEvent {
                    scan_id,
                    stats,
                    elapsed_ms,
                    volume: plan.volume.clone(),
                    saved: false,
                },
            );
            resave_current(&app);
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

#[tauri::command(async)]
fn get_view(state: State<'_, AppState>, id: NodeId, depth: u32) -> CmdResult<ViewResponse> {
    with_tree(&state, |t| {
        let root = t
            .view(id, depth.min(MAX_VIEW_DEPTH), MIN_ARC_FRACTION)
            .ok_or_else(|| "item no longer exists".to_string())?;
        Ok(ViewResponse {
            root,
            crumbs: t.crumbs(id),
            path: t.path_of(id).display().to_string(),
        })
    })
}

#[tauri::command(async)]
fn list_children(state: State<'_, AppState>, id: NodeId, limit: usize) -> CmdResult<Vec<ViewNode>> {
    with_tree(&state, |t| Ok(t.list_children(id, limit.clamp(1, 5000))))
}

#[tauri::command(async)]
fn largest_files(state: State<'_, AppState>, id: NodeId, limit: usize) -> CmdResult<Vec<ItemHit>> {
    with_tree(&state, |t| Ok(t.largest_files(id, limit.clamp(1, 1000))))
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
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            app_info,
            list_volumes,
            pick_folder,
            saved_scans,
            open_saved,
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
