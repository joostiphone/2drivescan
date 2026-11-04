// Thin wrapper around the Tauri IPC. When the UI runs in a normal browser
// (e.g. for design work or the website demo) a mock backend is used instead.

const tauri = window.__TAURI__;
export const isPreview = !tauri;

let mockPromise = null;
function mock() {
  mockPromise ||= import('./mock.js').then((m) => m.createMock());
  return mockPromise;
}

async function invoke(cmd, args = {}) {
  if (tauri) return tauri.core.invoke(cmd, args);
  return (await mock()).invoke(cmd, args);
}

export async function listen(event, cb) {
  if (tauri) return tauri.event.listen(event, (e) => cb(e.payload));
  return (await mock()).listen(event, cb);
}

export const api = {
  appInfo: () => invoke('app_info'),
  listVolumes: () => invoke('list_volumes'),
  pickFolder: () => invoke('pick_folder'),
  savedScans: () => invoke('saved_scans'),
  openSaved: (path) => invoke('open_saved', { path }),
  scanStorage: () => invoke('scan_storage'),
  clearSavedScans: () => invoke('clear_saved_scans'),
  startScan: (path, full = false) => invoke('start_scan', { path, full }),
  cancelScan: () => invoke('cancel_scan'),
  getView: (id, depth) => invoke('get_view', { id, depth }),
  listChildren: (id, limit) => invoke('list_children', { id, limit }),
  largestFiles: (id, limit) => invoke('largest_files', { id, limit }),
  search: (id, query, limit) => invoke('search', { id, query, limit }),
  typeTotals: (id) => invoke('type_totals', { id }),
  nodePath: (id) => invoke('node_path', { id }),
  resolveItems: (ids) => invoke('resolve_items', { ids }),
  revealNode: (id) => invoke('reveal_node', { id }),
  openNode: (id) => invoke('open_node', { id }),
  openExternal: (url) => invoke('open_external', { url }),
  trashNodes: (ids) => invoke('trash_nodes', { ids }),
  checkUpdate: () => invoke('check_update'),
  installUpdate: () => invoke('install_update'),
};
