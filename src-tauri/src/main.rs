// Prevents an additional console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod volumes;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use scan_core::{Crumb, ItemHit, NodeId, Progress, ScanError, Tree, TreeStats, ViewNode};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_dialog::DialogExt;
use tauri_plugin_opener::OpenerExt;

use crate::volumes::Volume;

/// Arcs narrower than this fraction of the full circle are grouped.
const MIN_ARC_FRACTION: f64 = 0.5 / 360.0;
const MAX_VIEW_DEPTH: u32 = 8;

#[derive(Default)]
struct AppState {
    tree: RwLock<Option<Tree>>,
    active: Mutex<Option<(u64, Arc<Progress>)>>,
    next_id: AtomicU64,
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
    current: String,
    elapsed_ms: u64,
}

#[derive(Serialize, Clone)]
struct DoneEvent {
    scan_id: u64,
    stats: TreeStats,
    elapsed_ms: u64,
    volume: Option<Volume>,
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
fn start_scan(app: AppHandle, state: State<'_, AppState>, path: String) -> CmdResult<u64> {
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
        .spawn(move || run_scan(app, scan_id, root, plan, progress))
        .map_err(|e| e.to_string())?;
    Ok(scan_id)
}

fn run_scan(
    app: AppHandle,
    scan_id: u64,
    root: PathBuf,
    plan: volumes::Plan,
    progress: Arc<Progress>,
) {
    let started = Instant::now();
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
                        current: s.current,
                        elapsed_ms: started.elapsed().as_millis() as u64,
                    },
                );
                std::thread::sleep(Duration::from_millis(90));
            }
        })
    };

    let result = scan_core::scan(&root, &plan.opts, &progress);
    finished.store(true, Ordering::Relaxed);
    let _ = ticker.join();

    let state = app.state::<AppState>();
    let is_current = |state: &AppState| {
        state
            .active
            .lock()
            .map(|a| matches!(&*a, Some((id, _)) if *id == scan_id))
            .unwrap_or(false)
    };
    if !is_current(&state) {
        return; // Superseded by a newer scan.
    }
    let elapsed_ms = started.elapsed().as_millis() as u64;
    match result {
        Ok(mut tree) => {
            if let Some(v) = &plan.volume {
                let used = v.total.saturating_sub(v.available);
                let hidden = used.saturating_sub(tree.stats().size);
                if hidden > 64 * 1024 * 1024 && hidden > used / 200 {
                    tree.add_hidden_space(hidden);
                }
            }
            let stats = tree.stats();
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
                    cancelled: matches!(err, ScanError::Cancelled),
                    message: err.to_string(),
                },
            );
        }
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
async fn trash_nodes(state: State<'_, AppState>, ids: Vec<NodeId>) -> CmdResult<TrashReport> {
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

    let mut guard = state
        .tree
        .write()
        .map_err(|_| "state poisoned".to_string())?;
    let mut freed = 0;
    let stats = guard.as_mut().map(|tree| {
        for &id in &removed {
            freed += tree.remove(id).unwrap_or(0);
        }
        tree.stats()
    });
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
        .run(tauri::generate_context!())
        .expect("error while running 2DriveScan");
}
