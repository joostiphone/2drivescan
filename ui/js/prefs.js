// User preferences, kept in the webview's local storage. Reading never
// throws: when storage is unavailable every preference has its default.

const PREFS = {
  // 'system' | 'light' | 'dark'. Stored as 'light' / 'dark'; system = no entry.
  theme: { key: '2ds-theme', def: 'system', values: ['system', 'light', 'dark'] },
  // 'system' | 'en' | 'nl'
  language: { key: '2ds-lang', def: 'system', values: ['system', 'en', 'nl'] },
  // 'auto' (binary on Windows, decimal elsewhere) | 'decimal' | 'binary'
  units: { key: '2ds-units', def: 'auto', values: ['auto', 'decimal', 'binary'] },
  // Chart coloring when a scan opens: 'folder' | 'type'
  colorMode: { key: '2ds-color', def: 'folder', values: ['folder', 'type'] },
  // Folder levels shown in the chart.
  rings: { key: '2ds-rings', def: '5', values: ['4', '5', '6', '7'] },
  // Chart view: rings or nested boxes.
  chartView: { key: '2ds-view', def: 'sunburst', values: ['sunburst', 'treemap'] },
  // 'auto' follows the system's reduce-motion setting.
  motion: { key: '2ds-motion', def: 'auto', values: ['auto', 'on', 'off'] },
  // Look for a new version at startup: 'on' | 'off'
  updateCheck: { key: '2ds-auto-update', def: 'on', values: ['on', 'off'] },
  // Time (ms) of the last successful update check.
  updateChecked: { key: '2ds-update-checked', def: '' },
};

export function getPref(name) {
  const p = PREFS[name];
  let v = null;
  try {
    v = localStorage.getItem(p.key);
  } catch {
    /* storage unavailable */
  }
  if (v == null || (p.values && !p.values.includes(v))) return p.def;
  return v;
}

export function setPref(name, value) {
  const p = PREFS[name];
  const v = String(value);
  try {
    if (v === p.def) localStorage.removeItem(p.key);
    else localStorage.setItem(p.key, v);
  } catch {
    /* ignore */
  }
}

// Lists of strings, stored as JSON.
const LISTS = {
  // Folders every scan skips.
  excluded: '2ds-excluded',
};

export function getList(name) {
  try {
    const v = JSON.parse(localStorage.getItem(LISTS[name]) || '[]');
    return Array.isArray(v) ? v.filter((x) => typeof x === 'string') : [];
  } catch {
    return [];
  }
}

export function setList(name, list) {
  try {
    if (list.length) localStorage.setItem(LISTS[name], JSON.stringify(list));
    else localStorage.removeItem(LISTS[name]);
  } catch {
    /* ignore */
  }
}

/** True when the system asks for less motion. */
export function systemReducesMotion() {
  return matchMedia('(prefers-reduced-motion: reduce)').matches;
}

/** Whether the chart and the interface should animate. */
export function animationsOn() {
  const m = getPref('motion');
  return m === 'auto' ? !systemReducesMotion() : m === 'on';
}
