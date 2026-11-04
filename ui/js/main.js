import { api, listen, isPreview } from './api.js';
import { t, lang, setLanguage } from './i18n.js';
import { bytes, bytesParts, count, percent, duration, truncateMiddle, escapeHtml as esc, setBinaryUnits } from './format.js';
import { icon } from './icons.js';
import { Sunburst } from './sunburst.js';
import { CATEGORIES, CATEGORY_COLORS, HIDDEN_COLOR, GROUP_COLOR, css } from './colors.js';
import { getPref, setPref, animationsOn } from './prefs.js';
import { initSettings, openSettings, toggleSettings, settingsOpen, closeSettings, refreshSettings, settingsProgress, URLS } from './settings.js';

const LIST_LIMIT = 1500;
const REPO_URL = URLS.repo;
const FDA_URL = URLS.fda;
/** Folder levels the chart shows (a setting). */
const rings = () => +getPref('rings');

const $ = (sel, root = document) => root.querySelector(sel);

const state = {
  info: { os: 'linux', version: '' },
  volumes: [],
  scanId: null,
  scanPath: null,
  scanLabel: '',
  result: null,
  rootId: 0,
  focusId: 0,
  view: null,
  tab: 'contents',
  colorMode: getPref('colorMode'),
  query: '',
  collector: new Map(),
  rows: new Map(),
  saved: new Map(),
  navToken: 0,
  panelToken: 0,
};

let chart = null;

// ------------------------------------------------------------------ helpers

const isWindows = () => state.info.os === 'windows';
const isMac = () => state.info.os === 'macos';

function revealLabel() {
  if (isMac()) return t('reveal_mac');
  if (isWindows()) return t('reveal_win');
  return t('reveal_linux');
}

function itemsLabel(n) {
  return n === 1 ? t('item') : t('items', { n: count(n) });
}

function displayName(node) {
  if (!node) return '';
  if (node.kind === 'hidden') return t('hidden_space');
  if (node.kind === 'group') return t('smaller_items', { n: count(node.items) });
  if (node.id === state.rootId) return state.scanLabel || node.name;
  return node.name;
}

function colorFor(item) {
  if (item.kind === 'hidden') return css(HIDDEN_COLOR);
  if (item.kind === 'group') return css(GROUP_COLOR);
  return chart?.colorOf(item.id) || css(CATEGORY_COLORS[item.cat ?? 7]);
}

function setScreen(name) {
  document.body.dataset.screen = name;
  for (const s of document.querySelectorAll('.screen')) s.hidden = s.id !== `screen-${name}`;
  hideTooltip();
  hideMenu();
}

function toast(message, kind = 'ok') {
  const root = $('#toast-root');
  const el = document.createElement('div');
  el.className = `toast ${kind}`;
  el.innerHTML = `${icon(kind === 'ok' ? 'check' : 'warn')}<span>${esc(message)}</span>`;
  root.appendChild(el);
  setTimeout(() => el.classList.add('out'), 3600);
  setTimeout(() => el.remove(), 4000);
}

function errText(e) {
  return typeof e === 'string' ? e : e?.message || String(e);
}

async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const ta = document.createElement('textarea');
    ta.value = text;
    document.body.appendChild(ta);
    ta.select();
    document.execCommand('copy');
    ta.remove();
  }
}

// ------------------------------------------------------------------ theme

const systemTheme = () => (matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark');

function loadTheme() {
  const saved = getPref('theme');
  const fromUrl = new URLSearchParams(location.search).get('theme');
  return fromUrl || (saved === 'system' ? systemTheme() : saved);
}

function applyTheme(theme) {
  document.documentElement.dataset.theme = theme;
  $('#btn-theme').innerHTML = icon(theme === 'dark' ? 'sun' : 'moon');
  syncChartTheme();
}

function syncChartTheme() {
  if (!chart) return;
  const cs = getComputedStyle(document.documentElement);
  const center = cs.getPropertyValue('--center').trim().split(/\s+/).map(Number);
  chart.setTheme({ center, ring: cs.getPropertyValue('--ring').trim() });
}

function toggleTheme() {
  const next = document.documentElement.dataset.theme === 'dark' ? 'light' : 'dark';
  setPref('theme', next);
  applyTheme(next);
  refreshSettings();
}

// ------------------------------------------------------------------ preferences

function applyUnits() {
  const u = getPref('units');
  setBinaryUnits(u === 'binary' || (u === 'auto' && isWindows()));
}

function applyMotion() {
  const on = animationsOn();
  document.body.classList.toggle('no-motion', !on);
  chart?.setAnimate(on);
}

/** Applies a changed setting (called by the settings dialog). */
function applyPref(name, value) {
  setPref(name, value);
  switch (name) {
    case 'theme':
      applyTheme(value === 'system' ? systemTheme() : value);
      break;
    case 'language':
      setLanguage(value);
      refreshUI();
      break;
    case 'units':
      applyUnits();
      refreshUI();
      break;
    case 'colorMode':
      if (document.body.dataset.screen === 'result') setColorMode(value);
      else state.colorMode = value;
      break;
    case 'rings':
      if (document.body.dataset.screen === 'result') rebuildResult();
      break;
    case 'motion':
      applyMotion();
      break;
  }
}

/** Re-renders everything with text or sizes in it (language or units changed). */
function refreshUI() {
  labelTitlebar();
  const screen = document.body.dataset.screen;
  if (screen === 'home') renderHome();
  else if (screen === 'scan') renderScan();
  else if (screen === 'result') rebuildResult();
  $('.preview-badge') && ($('.preview-badge').textContent = t('preview_mode'));
  renderUpdateBanner();
  refreshSettings();
}

/** Builds the result screen again at the folder that is open. */
function rebuildResult() {
  if (!state.result) return;
  state.query = '';
  hideTooltip();
  hideMenu();
  renderResultShell();
  setColorMode(state.colorMode);
  navigate(state.focusId, 'pop');
}

// ------------------------------------------------------------------ home

function gauge(used, total, id) {
  const pct = total ? used / total : 0;
  const r = 36;
  const c = 2 * Math.PI * r;
  const stops =
    pct > 0.9 ? ['#ff5c7a', '#ff8a4c'] : pct > 0.75 ? ['#ff9f43', '#ffd43b'] : ['#7c5cff', '#ff4fa3'];
  return `
    <div class="gauge">
      <svg viewBox="0 0 86 86">
        <defs><linearGradient id="g${id}" x1="0" y1="0" x2="1" y2="1">
          <stop offset="0" stop-color="${stops[0]}"/><stop offset="1" stop-color="${stops[1]}"/>
        </linearGradient></defs>
        <circle class="track" cx="43" cy="43" r="${r}" fill="none" stroke-width="9"/>
        <circle class="value" cx="43" cy="43" r="${r}" fill="none" stroke="url(#g${id})" stroke-width="9"
          stroke-linecap="round" stroke-dasharray="${c}" stroke-dashoffset="${c}" data-target="${c * (1 - pct)}"/>
      </svg>
      <div class="gauge-label">${Math.round(pct * 100)}%</div>
    </div>`;
}

function volumeLabel(v) {
  return v.name || (v.is_root ? t('system_disk') : v.mount_point);
}

function renderHome() {
  const el = $('#screen-home');
  el.innerHTML = `
    <div class="home">
      <div class="hero">
        <img class="hero-logo" src="img/logo.png" alt="" />
        <h1>2DriveScan</h1>
        <p>${esc(t('tagline'))}</p>
      </div>
      <div class="section-title">${esc(t('volumes'))}</div>
      <div class="vol-grid" id="vol-grid"><div class="empty">${esc(t('loading_volumes'))}</div></div>
      <div class="home-footer">
        <span>${esc(t('version', { v: state.info.version }))}</span>
        <button class="link-btn" data-url="${REPO_URL}">${icon('github')} GitHub</button>
        <span>${esc(t('free_open_source'))}</span>
        <button class="link-btn" id="home-settings" type="button">${icon('gear')} ${esc(t('settings'))}</button>
      </div>
    </div>`;
  el.querySelector('[data-url]').addEventListener('click', (e) => api.openExternal(e.currentTarget.dataset.url).catch(() => {}));
  $('#home-settings').addEventListener('click', () => openSettings('general'));
  loadVolumes();
}

function timeAgo(ms) {
  const diff = (ms - Date.now()) / 1000;
  const rtf = new Intl.RelativeTimeFormat(lang(), { numeric: 'auto' });
  const steps = [
    [60, 'second'],
    [3600, 'minute', 60],
    [86400, 'hour', 3600],
    [604800, 'day', 86400],
    [Infinity, 'week', 604800],
  ];
  const abs = Math.abs(diff);
  if (abs < 45) return t('just_now');
  for (const [limit, unit, div = 1] of steps) {
    if (abs < limit) return rtf.format(Math.round(diff / div), unit);
  }
  return new Date(ms).toLocaleDateString(lang());
}

/** Line and buttons for a saved scan of `path`, shown on its card. */
function savedBlock(saved) {
  if (!saved) return '';
  if (saved.complete) {
    return `
      <div class="vol-saved">${icon('check')}<span>${esc(t('scanned_ago', { when: timeAgo(saved.scanned_at_ms) }))}</span></div>
      <div class="vol-actions">
        <button class="btn btn-sm primary" type="button" data-act="open">${esc(t('open_scan'))}</button>
        <button class="btn btn-sm" type="button" data-act="rescan">${icon('refresh')}${esc(t('rescan'))}</button>
      </div>`;
  }
  return `
    <div class="vol-saved partial">${icon('refresh')}<span>${esc(t('unfinished_scan', { when: timeAgo(saved.scanned_at_ms) }))}</span></div>
    <div class="vol-actions">
      <button class="btn btn-sm primary" type="button" data-act="continue">${esc(t('continue_scan'))}</button>
    </div>`;
}

async function loadVolumes() {
  const [volumes, saved] = await Promise.all([
    api.listVolumes().catch(() => []),
    api.savedScans().catch(() => []),
  ]);
  state.volumes = volumes;
  state.saved = new Map(saved.map((s) => [s.root_path, s]));
  const grid = $('#vol-grid');
  if (!grid) return;
  const targets = [];
  const cards = state.volumes.map((v, i) => {
    const used = Math.max(0, v.total - v.available);
    const chips = [v.file_system && v.file_system.toUpperCase(), v.kind !== 'unknown' && v.kind.toUpperCase(), v.removable && t('removable')]
      .filter(Boolean)
      .map((c) => `<span class="chip">${esc(c)}</span>`)
      .join('');
    targets.push({ path: v.mount_point, label: volumeLabel(v) });
    return `
      <div class="vol-card glass" data-target="${targets.length - 1}" role="button" tabindex="0">
        ${gauge(used, v.total, i)}
        <div class="vol-info">
          <div class="vol-name">${icon(v.removable ? 'usb' : 'drive')}<span>${esc(volumeLabel(v))}</span></div>
          <div class="vol-sub">${esc(v.mount_point)}</div>
          <div class="vol-free">${esc(t('free_of', { free: bytes(v.available), total: bytes(v.total) }))}</div>
          ${state.saved.has(v.mount_point) ? savedBlock(state.saved.get(v.mount_point)) : `<div class="chips">${chips}</div>`}
        </div>
      </div>`;
  });
  if (state.info.home) {
    targets.push({ path: state.info.home, label: t('home_folder') });
    cards.push(`
      <div class="vol-card glass action" data-target="${targets.length - 1}" role="button" tabindex="0">
        <div class="action-icon">${icon('user')}</div>
        <div class="vol-info">
          <div class="vol-name">${esc(t('home_folder'))}</div>
          <div class="vol-sub">${esc(state.info.home)}</div>
          ${state.saved.has(state.info.home) ? savedBlock(state.saved.get(state.info.home)) : `<div class="vol-free">${esc(t('home_folder_hint'))}</div>`}
        </div>
      </div>`);
  }
  // Other folders scanned before.
  const shown = new Set(targets.map((x) => x.path));
  for (const s of state.saved.values()) {
    if (shown.has(s.root_path)) continue;
    const label = s.root_path.split(/[\\/]/).filter(Boolean).pop() || s.root_path;
    targets.push({ path: s.root_path, label });
    cards.push(`
      <div class="vol-card glass action" data-target="${targets.length - 1}" role="button" tabindex="0">
        <div class="action-icon">${icon('folder')}</div>
        <div class="vol-info">
          <div class="vol-name">${esc(label)}</div>
          <div class="vol-sub">${esc(s.root_path)} · ${esc(bytes(s.size))}</div>
          ${savedBlock(s)}
        </div>
      </div>`);
  }
  cards.push(`
    <div class="vol-card glass action" data-action="pick" role="button" tabindex="0">
      <div class="action-icon grad">${icon('folderPlus')}</div>
      <div class="vol-info">
        <div class="vol-name">${esc(t('scan_folder'))}</div>
        <div class="vol-free">${esc(t('scan_folder_hint'))}</div>
      </div>
    </div>`);
  grid.innerHTML = cards.join('');
  if (!state.volumes.length) grid.insertAdjacentHTML('afterbegin', `<div class="empty" style="grid-column:1/-1">${esc(t('no_volumes'))}</div>`);

  const activate = async (card, act) => {
    if (card.dataset.action === 'pick') {
      const path = await api.pickFolder().catch(() => null);
      if (path) startScan(path, path.split(/[\\/]/).filter(Boolean).pop() || path);
      return;
    }
    const target = targets[+card.dataset.target];
    const saved = state.saved.get(target.path);
    // Default action: open a finished scan, continue an unfinished one, or scan.
    const action = act || (saved ? (saved.complete ? 'open' : 'continue') : 'scan');
    if (action === 'open') openSaved(target.path, target.label);
    else startScan(target.path, target.label);
  };
  grid.querySelectorAll('.vol-card').forEach((card) => {
    card.addEventListener('pointermove', (e) => {
      const r = card.getBoundingClientRect();
      card.style.setProperty('--mx', `${e.clientX - r.left}px`);
      card.style.setProperty('--my', `${e.clientY - r.top}px`);
    });
    card.addEventListener('click', (e) => {
      const b = e.target.closest('button[data-act]');
      activate(card, b ? b.dataset.act : null);
    });
    card.addEventListener('keydown', (e) => {
      if (e.target === card && (e.key === 'Enter' || e.key === ' ')) {
        e.preventDefault();
        activate(card, null);
      }
    });
  });
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      grid.querySelectorAll('.gauge .value').forEach((c) => (c.style.strokeDashoffset = c.dataset.target));
    }),
  );
}

async function openSaved(path, label) {
  state.scanPath = path;
  state.scanLabel = label || path;
  state.scanId = null;
  try {
    const d = await api.openSaved(path);
    onDone(d, true);
  } catch (e) {
    toast(errText(e), 'err');
  }
}

function showHome() {
  setScreen('home');
  renderHome();
}

// ------------------------------------------------------------------ scanning

function renderScan() {
  $('#screen-scan').innerHTML = `
    <div class="scan">
      <div class="radar">
        <div class="glow"></div>
        <div class="track"></div>
        <div class="ring r1"></div>
        <div class="ring r2"></div>
        <div class="ring r3"></div>
        <div class="core"><div><div class="core-size" id="sc-bytes">0 B</div><div class="core-label">${esc(t('found'))}</div></div></div>
      </div>
      <div class="scan-title">${esc(t('scanning'))} ${esc(state.scanLabel)}<small>${esc(state.scanPath)}</small></div>
      <div class="scan-stats">
        <div class="stat glass"><div class="stat-value" id="sc-files">0</div><div class="stat-label">${esc(t('files'))}</div></div>
        <div class="stat glass"><div class="stat-value" id="sc-dirs">0</div><div class="stat-label">${esc(t('folders'))}</div></div>
        <div class="stat glass"><div class="stat-value" id="sc-time">0 s</div><div class="stat-label">${esc(t('elapsed'))}</div></div>
      </div>
      <div class="scan-reused" id="sc-reused" hidden>${icon('bolt')}<span class="reused-text"></span></div>
      <div class="scan-path" id="sc-path">${esc(t('preparing'))}</div>
      <button class="btn" id="sc-cancel" type="button">${icon('x')} ${esc(t('cancel'))}</button>
      <p class="scan-hint">${esc(t('scan_resume_hint'))}</p>
    </div>`;
  $('#sc-cancel').addEventListener('click', () => api.cancelScan());
}

async function startScan(path, label, { full = false } = {}) {
  if (!path) return;
  state.scanPath = path;
  state.scanLabel = label || path;
  state.scanId = null;
  renderScan();
  setScreen('scan');
  try {
    state.scanId = await api.startScan(path, full);
  } catch (e) {
    toast(`${t('scan_failed')}: ${errText(e)}`, 'err');
    showHome();
  }
}

function onProgress(p) {
  if (document.body.dataset.screen !== 'scan') return;
  if (state.scanId != null && p.scan_id !== state.scanId) return;
  $('#sc-bytes').textContent = bytes(p.bytes);
  $('#sc-files').textContent = count(p.files);
  $('#sc-dirs').textContent = count(p.dirs);
  $('#sc-time').textContent = duration(p.elapsed_ms);
  if (p.current) $('#sc-path').textContent = truncateMiddle(p.current, 96);
  const reused = $('#sc-reused');
  if (reused && p.incremental) {
    reused.hidden = false;
    reused.querySelector('.reused-text').textContent = t('reused_folders', { n: count(p.reused || 0) });
  }
}

function onDone(d, opened = false) {
  if (!opened && state.scanId != null && d.scan_id !== state.scanId) return;
  state.result = d;
  state.rootId = d.stats.root_id;
  state.collector.clear();
  state.query = '';
  showResult();
}

function onFailed(f) {
  if (state.scanId != null && f.scan_id !== state.scanId) return;
  if (!f.cancelled) toast(`${t('scan_failed')}: ${f.message}`, 'err');
  showHome();
}

// ------------------------------------------------------------------ result

function renderResultShell() {
  $('#screen-result').innerHTML = `
    <div class="result">
      <div class="chart-wrap">
        <div class="chart-top">
          <div class="usage glass" id="usage"></div>
          <div class="segmented glass" id="colormode">
            <button type="button" data-mode="folder">${icon('pie')}${esc(t('color_folders'))}</button>
            <button type="button" data-mode="type">${icon('layers')}${esc(t('color_types'))}</button>
          </div>
        </div>
        <div class="chart-canvas-host" id="chart-host">
          <canvas id="chart"></canvas>
          <div class="center-label" id="center"></div>
        </div>
        <div class="chart-status" id="status"></div>
      </div>
      <aside class="panel glass">
        <div class="panel-head">
          <div class="panel-title"><h2 id="p-title"></h2><p id="p-sub"></p></div>
          <button class="icon-btn" id="p-reveal" type="button" title="${esc(revealLabel())}">${icon('eye')}</button>
        </div>
        <div class="search" id="search">
          ${icon('search')}
          <input id="search-input" type="text" spellcheck="false" autocomplete="off" placeholder="${esc(t('search_placeholder'))}" />
          <button class="clear" type="button">${icon('x')}</button>
        </div>
        <div class="tabs" id="tabs">
          <button type="button" data-tab="contents">${esc(t('contents'))}</button>
          <button type="button" data-tab="largest">${esc(t('largest'))}</button>
          <button type="button" data-tab="types">${esc(t('types'))}</button>
        </div>
        <div class="panel-body" id="p-body"></div>
      </aside>
      <div class="collector glass" id="collector"></div>
    </div>`;

  chart = new Sunburst($('#chart'), {
    rings: rings(),
    animate: animationsOn(),
    hiddenLabel: t('hidden_space'),
    canGoUp: () => state.focusId !== state.rootId,
    onGeometry: positionCenter,
    onHover: onChartHover,
    onClick: (node) => {
      if (node.id < 0) return;
      if (node.kind === 'dir' && node.has_children) zoomIn(node.id);
      else highlightRow(node.id, true);
    },
    onDblClick: (node) => {
      if (node.id >= 0 && node.kind !== 'dir' && node.kind !== 'hidden') api.revealNode(node.id).catch((e) => toast(errText(e), 'err'));
    },
    onCenterClick: goUp,
    onContext: (node, e) => showMenu(node, e),
    onDragStart: (node, e) => beginDrag(node, e),
  });
  chart.setMode(state.colorMode);
  syncChartTheme();

  $('#colormode').addEventListener('click', (e) => {
    const b = e.target.closest('button[data-mode]');
    if (b) setColorMode(b.dataset.mode);
  });
  $('#tabs').addEventListener('click', (e) => {
    const b = e.target.closest('button[data-tab]');
    if (!b) return;
    state.tab = b.dataset.tab;
    if (state.query) clearSearch(false);
    refreshPanel();
  });
  $('#p-reveal').addEventListener('click', () => api.revealNode(state.focusId).catch((e) => toast(errText(e), 'err')));

  const input = $('#search-input');
  let debounce = 0;
  input.addEventListener('input', () => {
    $('#search').classList.toggle('has-text', !!input.value);
    clearTimeout(debounce);
    debounce = setTimeout(() => {
      state.query = input.value.trim();
      refreshPanel();
    }, 180);
  });
  input.addEventListener('keydown', (e) => {
    if (e.key === 'Escape') {
      e.stopPropagation();
      clearSearch();
      input.blur();
    }
  });
  $('#search .clear').addEventListener('click', () => clearSearch());

  setupRowEvents($('#p-body'));
  renderUsage();
  renderStatus();
  renderCollector();
}

function clearSearch(refresh = true) {
  const input = $('#search-input');
  if (input) input.value = '';
  $('#search')?.classList.remove('has-text');
  state.query = '';
  if (refresh) refreshPanel();
}

function setColorMode(mode) {
  state.colorMode = mode;
  setPref('colorMode', mode);
  chart?.setMode(mode);
  document.querySelectorAll('#colormode button').forEach((b) => b.classList.toggle('active', b.dataset.mode === mode));
  // Row colors depend on the chart colors.
  requestAnimationFrame(() => refreshPanel());
  renderCollector();
}

function showResult() {
  setScreen('result');
  renderResultShell();
  setColorMode(state.colorMode);
  navigate(state.rootId, 'sweep');
}

function positionCenter(geom) {
  const el = $('#center');
  if (!el) return;
  const size = geom.r0 * 2 * 0.86;
  el.style.left = `${geom.cx}px`;
  el.style.top = `${geom.cy}px`;
  el.style.width = `${size}px`;
  el.style.height = `${size}px`;
  el.style.setProperty('--cs', `${Math.max(18, geom.r0 * 0.3)}px`);
  el.querySelector('.center-size')?.style.setProperty('font-size', `${Math.max(18, geom.r0 * 0.3)}px`);
}

function renderCenter(node = null, { hoverCenter = false } = {}) {
  const el = $('#center');
  if (!el || !state.view) return;
  const focus = state.view.root;
  const n = node || focus;
  const [num, unit] = bytesParts(n.size);
  let sub;
  if (node && node !== focus) sub = percent(node.size, focus.size);
  else if (n.kind === 'dir') sub = itemsLabel(n.items);
  else sub = '';
  if (node && node.kind === 'group') sub = `${count(node.items)} · ${percent(node.size, focus.size)}`;
  const fs = Math.max(18, (chart?.geom?.r0 || 100) * 0.3);
  el.innerHTML = `
    <div class="center-name">${esc(displayName(n))}</div>
    <div class="center-size" style="font-size:${fs}px">${esc(num)}<span class="unit">${esc(unit)}</span></div>
    <div class="center-sub">${esc(sub)}</div>
    <div class="center-up">${icon('up')}${esc(t('go_up_hint'))}</div>`;
  el.classList.toggle('can-up', state.focusId !== state.rootId);
  el.classList.toggle('hovering', hoverCenter);
}

function onChartHover(node, e, info) {
  if (node) {
    renderCenter(node);
    showTooltip(node, e);
    markRow(node.id);
  } else {
    renderCenter(null, { hoverCenter: info?.center });
    hideTooltip();
    markRow(null);
  }
}

function renderUsage() {
  const el = $('#usage');
  if (!el || !state.result) return;
  const v = state.result.volume;
  if (v) {
    const used = Math.max(0, v.total - v.available);
    el.innerHTML = `
      <div class="usage-bar"><div style="width:${(used / v.total) * 100}%"></div></div>
      <div class="usage-text"><b>${esc(bytes(used))}</b> ${esc(t('used').toLowerCase())} · <b>${esc(bytes(v.available))}</b> ${esc(t('free').toLowerCase())} · ${esc(bytes(v.total))}</div>`;
  } else {
    el.innerHTML = `<div class="usage-text">${icon('folder')} <b>${esc(bytes(state.result.stats.size))}</b> ${esc(t('total_on_disk'))}</div>`;
    el.querySelector('.icon').style.cssText = 'width:14px;height:14px;vertical-align:-2px;margin-right:4px';
  }
}

function renderStatus() {
  const el = $('#status');
  if (!el || !state.result) return;
  const { stats, elapsed_ms, saved } = state.result;
  let html = saved
    ? `<span>${esc(t('saved_summary', { files: count(stats.files), when: new Date(stats.scanned_at_ms).toLocaleString(lang(), { dateStyle: 'medium', timeStyle: 'short' }) }))}</span>`
    : `<span>${esc(t('scanned_summary', { files: count(stats.files), time: duration(elapsed_ms) }))}</span>`;
  if (stats.unreadable > 0) {
    html += `<span class="status-warn">${icon('lock')}${esc(t('unreadable', { n: count(stats.unreadable) }))}</span>`;
    if (isMac()) html += `<button class="status-link" id="fda" type="button">${esc(t('grant_access'))}</button>`;
  }
  html += `<span class="tip">${esc(t('shortcuts'))}</span>`;
  el.innerHTML = html;
  $('#fda')?.addEventListener('click', () => api.openExternal(FDA_URL).catch((e) => toast(errText(e), 'err')));
}

// ------------------------------------------------------------------ navigation

async function navigate(id, mode = 'pop', anchorId = null) {
  const token = ++state.navToken;
  let view;
  try {
    view = await api.getView(id, mode === 'out' ? rings() + 1 : rings());
  } catch (e) {
    if (id !== state.rootId) return navigate(state.rootId, 'pop');
    toast(errText(e), 'err');
    return;
  }
  if (token !== state.navToken) return;
  state.focusId = id;
  state.view = view;
  chart.setData(view.root, mode, anchorId);
  hideTooltip();
  renderCrumbs();
  renderCenter();
  renderPanelHead();
  $('#btn-up').disabled = id === state.rootId;
  refreshPanel();
}

function zoomIn(id) {
  if (id === state.focusId) return;
  navigate(id, 'in', id);
}

function goUp() {
  const crumbs = state.view?.crumbs || [];
  const idx = crumbs.findIndex((c) => c.id === state.rootId);
  if (crumbs.length < 2 || state.focusId === state.rootId || idx < 0) return;
  navigate(crumbs[crumbs.length - 2].id, 'out', state.focusId);
}

function renderCrumbs() {
  const nav = $('#crumbs');
  const crumbs = (state.view?.crumbs || []).filter((c, i, arr) => i >= arr.findIndex((x) => x.id === state.rootId));
  nav.innerHTML = crumbs
    .map((c, i) => {
      const name = c.id === state.rootId ? state.scanLabel : c.name;
      const cur = i === crumbs.length - 1;
      return `${i ? `<span class="crumb-sep">${icon('chevron')}</span>` : ''}<button class="crumb${cur ? ' current' : ''}" data-id="${c.id}" type="button" title="${esc(name)}">${esc(name)}</button>`;
    })
    .join('');
  nav.querySelectorAll('.crumb').forEach((b) =>
    b.addEventListener('click', () => {
      const id = +b.dataset.id;
      if (id !== state.focusId) navigate(id, 'out', state.focusId);
    }),
  );
  nav.scrollLeft = nav.scrollWidth;
}

function renderPanelHead() {
  const root = state.view.root;
  $('#p-title').textContent = displayName(root);
  $('#p-title').title = state.view.path;
  $('#p-sub').textContent = `${bytes(root.size)} · ${itemsLabel(root.items)}`;
}

// ------------------------------------------------------------------ side panel

function rowHtml(item, { total, sub = null, showPct = true }) {
  const color = colorFor(item);
  const isDir = item.kind === 'dir';
  const glyphIcon = item.kind === 'group' ? 'layers' : item.kind === 'hidden' ? 'lock' : isDir ? 'folder' : 'file';
  const clickable = item.id >= 0 && item.kind !== 'hidden';
  const pct = total ? Math.min(1, item.size / total) : 0;
  const collected = state.collector.has(item.id) ? ' collected' : '';
  return `
    <div class="row${clickable ? ' clickable' : ''}${collected}" data-id="${item.id}">
      <div class="bar" style="--pct:${pct};background:${color}"></div>
      <div class="glyph${item.kind === 'group' ? ' group' : ''}" style="background:${color}">${icon(glyphIcon)}</div>
      <div class="label">
        <div class="name" title="${esc(displayName(item))}">${esc(displayName(item))}</div>
        ${sub ? `<div class="sub" title="${esc(sub)}">${esc(sub)}</div>` : ''}
      </div>
      <div class="meta"><div class="size">${esc(bytes(item.size))}</div>${showPct ? `<div class="pct">${esc(percent(item.size, total))}</div>` : ''}</div>
    </div>`;
}

async function refreshPanel() {
  const body = $('#p-body');
  if (!body || !state.view) return;
  const token = ++state.panelToken;
  const id = state.focusId;
  const total = state.view.root.size;
  document.querySelectorAll('#tabs button').forEach((b) => b.classList.toggle('active', !state.query && b.dataset.tab === state.tab));
  state.rows.clear();
  try {
    if (state.query) {
      body.innerHTML = `<div class="empty">${esc(t('searching'))}</div>`;
      const hits = await api.search(id, state.query, 300);
      if (token !== state.panelToken) return;
      hits.forEach((h) => state.rows.set(h.id, h));
      body.innerHTML = hits.length
        ? hits.map((h) => rowHtml(h, { total, sub: h.rel_path, showPct: false })).join('')
        : `<div class="empty">${esc(t('no_results'))}</div>`;
    } else if (state.tab === 'contents') {
      const items = await api.listChildren(id, LIST_LIMIT);
      if (token !== state.panelToken) return;
      items.forEach((h) => h.id >= 0 && state.rows.set(h.id, h));
      body.innerHTML = items.length
        ? items.map((h) => rowHtml(h, { total, sub: h.kind === 'dir' ? itemsLabel(h.items) : null })).join('')
        : `<div class="empty">${esc(t('empty_folder'))}</div>`;
    } else if (state.tab === 'largest') {
      const hits = await api.largestFiles(id, 150);
      if (token !== state.panelToken) return;
      hits.forEach((h) => state.rows.set(h.id, h));
      body.innerHTML = hits.length
        ? hits.map((h) => rowHtml(h, { total, sub: h.rel_path })).join('')
        : `<div class="empty">${esc(t('empty_folder'))}</div>`;
    } else {
      const tt = await api.typeTotals(id);
      if (token !== state.panelToken) return;
      body.innerHTML = typesHtml(tt);
      body.querySelectorAll('.type-row').forEach((r) => r.addEventListener('click', () => setColorMode('type')));
    }
  } catch (e) {
    if (token === state.panelToken) body.innerHTML = `<div class="empty">${esc(errText(e))}</div>`;
  }
}

function typesHtml(tt) {
  const entries = CATEGORIES.map((c, i) => ({ key: c, size: tt.cats[i], color: css(CATEGORY_COLORS[i]), label: t(`cat_${c}`) }));
  if (tt.hidden > 0) entries.push({ key: 'hidden', size: tt.hidden, color: css(HIDDEN_COLOR), label: t('cat_hidden') });
  const shown = entries.filter((e) => e.size > 0).sort((a, b) => b.size - a.size);
  if (!shown.length) return `<div class="empty">${esc(t('empty_folder'))}</div>`;
  const total = shown.reduce((s, e) => s + e.size, 0) || 1;
  const bar = shown.map((e) => `<div style="flex-grow:${e.size};background:${e.color}" title="${esc(e.label)}"></div>`).join('');
  const rows = shown
    .map(
      (e) => `
      <div class="row type-row clickable">
        <div class="bar" style="--pct:${e.size / total};background:${e.color}"></div>
        <div class="glyph" style="background:${e.color}"></div>
        <div class="label"><div class="name">${esc(e.label)}</div></div>
        <div class="meta"><div class="size">${esc(bytes(e.size))}</div><div class="pct">${esc(percent(e.size, total))}</div></div>
      </div>`,
    )
    .join('');
  return `<div class="types-bar">${bar}</div>${rows}`;
}

function markRow(id) {
  document.querySelectorAll('#p-body .row.hl').forEach((r) => r.classList.remove('hl'));
  if (id == null || id < 0) return;
  document.querySelector(`#p-body .row[data-id="${id}"]`)?.classList.add('hl');
}

function highlightRow(id, scroll) {
  markRow(id);
  chart.setHighlight(id);
  if (scroll) document.querySelector(`#p-body .row[data-id="${id}"]`)?.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
}

function setupRowEvents(body) {
  let down = null;
  const rowItem = (el) => {
    const row = el.closest('.row[data-id]');
    if (!row) return null;
    return state.rows.get(+row.dataset.id) || null;
  };
  body.addEventListener('pointerover', (e) => {
    const item = rowItem(e.target);
    chart?.setHighlight(item ? item.id : null);
  });
  body.addEventListener('pointerleave', () => chart?.setHighlight(null));
  body.addEventListener('pointerdown', (e) => {
    if (e.button !== 0) return;
    const item = rowItem(e.target);
    if (item) down = { x: e.clientX, y: e.clientY, item };
  });
  body.addEventListener('pointermove', (e) => {
    if (!down) return;
    if (Math.hypot(e.clientX - down.x, e.clientY - down.y) > 6) {
      const item = down.item;
      down = null;
      beginDrag(item, e);
    }
  });
  body.addEventListener('pointerup', () => (down = null));
  body.addEventListener('click', (e) => {
    const item = rowItem(e.target);
    if (!item) return;
    if (item.kind === 'dir') {
      clearSearch(false);
      navigate(item.id, 'in', item.id);
    } else highlightRow(item.id, false);
  });
  body.addEventListener('dblclick', (e) => {
    const item = rowItem(e.target);
    if (item && item.kind !== 'dir') api.revealNode(item.id).catch((err) => toast(errText(err), 'err'));
  });
  body.addEventListener('contextmenu', (e) => {
    const item = rowItem(e.target);
    if (!item) return;
    e.preventDefault();
    showMenu(item, e);
  });
}

// ------------------------------------------------------------------ tooltip

function showTooltip(node, e) {
  const tip = $('#tooltip');
  const focus = state.view.root;
  let hint = '';
  if (node.kind === 'hidden') hint = `<div class="tt-hint">${esc(t('hidden_space_hint'))}</div>`;
  const sub = node.kind === 'group' ? t('smaller_items', { n: count(node.items) }) : node.kind === 'dir' ? itemsLabel(node.items) : '';
  tip.innerHTML = `
    <div class="tt-name"><span class="tt-dot" style="background:${colorFor(node)}"></span>${esc(node.kind === 'group' ? t('smaller_items', { n: count(node.items) }) : displayName(node))}</div>
    <div class="tt-size">${esc(bytes(node.size))}</div>
    <div class="tt-sub">${esc([t('of_parent', { pct: percent(node.size, focus.size), parent: displayName(focus) }), node.kind === 'group' ? '' : sub].filter(Boolean).join(' · '))}</div>
    ${hint}`;
  tip.hidden = false;
  const pad = 16;
  const r = tip.getBoundingClientRect();
  let x = e.clientX + pad;
  let y = e.clientY + pad;
  if (x + r.width > innerWidth - 8) x = e.clientX - r.width - pad;
  if (y + r.height > innerHeight - 8) y = e.clientY - r.height - pad;
  tip.style.left = `${Math.max(8, x)}px`;
  tip.style.top = `${Math.max(8, y)}px`;
}

function hideTooltip() {
  const tip = $('#tooltip');
  if (tip) tip.hidden = true;
}

// ------------------------------------------------------------------ context menu

function showMenu(item, e) {
  hideTooltip();
  const menu = $('#ctxmenu');
  if (item.id < 0 || item.kind === 'hidden') {
    hideMenu();
    return;
  }
  const isRoot = item.id === state.rootId;
  const inCollector = state.collector.has(item.id);
  const entries = [];
  if (item.kind === 'dir' && item.id !== state.focusId) entries.push(['zoom', 'zoom', t('zoom_in')]);
  entries.push(['open', 'external', t('open')]);
  entries.push(['reveal', 'eye', revealLabel()]);
  entries.push(['copy', 'copy', t('copy_path')]);
  if (!isRoot) {
    entries.push(null);
    entries.push(inCollector ? ['uncollect', 'x', t('remove_from_collector')] : ['collect', 'trash', t('add_to_collector')]);
  }
  menu.innerHTML =
    `<div class="ctx-title">${esc(displayName(item))} · ${esc(bytes(item.size))}</div>` +
    entries.map((en) => (en ? `<button type="button" data-act="${en[0]}">${icon(en[1])}<span>${esc(en[2])}</span></button>` : '<hr/>')).join('');
  menu.hidden = false;
  const r = menu.getBoundingClientRect();
  let x = e.clientX;
  let y = e.clientY;
  if (x + r.width > innerWidth - 8) x = innerWidth - r.width - 8;
  if (y + r.height > innerHeight - 8) y = innerHeight - r.height - 8;
  menu.style.left = `${x}px`;
  menu.style.top = `${y}px`;
  menu.onclick = async (ev) => {
    const b = ev.target.closest('button[data-act]');
    if (!b) return;
    hideMenu();
    try {
      switch (b.dataset.act) {
        case 'zoom':
          navigate(item.id, 'in', item.id);
          break;
        case 'open':
          await api.openNode(item.id);
          break;
        case 'reveal':
          await api.revealNode(item.id);
          break;
        case 'copy':
          await copyText(await api.nodePath(item.id));
          toast(t('copied'));
          break;
        case 'collect':
          addToCollector(item);
          break;
        case 'uncollect':
          removeFromCollector(item.id);
          break;
      }
    } catch (err) {
      toast(errText(err), 'err');
    }
  };
}

function hideMenu() {
  const m = $('#ctxmenu');
  if (m) m.hidden = true;
}

// ------------------------------------------------------------------ drag & drop

function beginDrag(item, e) {
  if (item.id < 0 || item.kind === 'hidden' || item.id === state.rootId) return;
  hideTooltip();
  const ghost = $('#drag-ghost');
  const collector = $('#collector');
  ghost.innerHTML = `<span class="dot" style="background:${colorFor(item)}"></span>${esc(displayName(item))}<span class="gsize">${esc(bytes(item.size))}</span>`;
  ghost.hidden = false;
  collector.classList.add('drop-ready');
  const move = (ev) => {
    ghost.style.left = `${ev.clientX}px`;
    ghost.style.top = `${ev.clientY}px`;
    const r = collector.getBoundingClientRect();
    const over = ev.clientX >= r.left && ev.clientX <= r.right && ev.clientY >= r.top - 12 && ev.clientY <= r.bottom;
    collector.classList.toggle('drop-hover', over);
  };
  const up = (ev) => {
    window.removeEventListener('pointermove', move);
    window.removeEventListener('pointerup', up);
    window.removeEventListener('blur', cancel);
    ghost.hidden = true;
    const over = collector.classList.contains('drop-hover');
    collector.classList.remove('drop-ready', 'drop-hover');
    chart?.cancelDrag();
    if (over && ev) addToCollector(item);
  };
  const cancel = () => up(null);
  window.addEventListener('pointermove', move);
  window.addEventListener('pointerup', up);
  window.addEventListener('blur', cancel);
  move(e);
}

// ------------------------------------------------------------------ collector

async function addToCollector(item) {
  if (item.id < 0 || item.kind === 'hidden' || item.id === state.rootId) return;
  if (state.collector.has(item.id)) {
    toast(t('already_collected'));
    return;
  }
  state.collector.set(item.id, { id: item.id, name: item.name, size: item.size, kind: item.kind, cat: item.cat, covered: false });
  await resolveCollector();
}

function removeFromCollector(id) {
  state.collector.delete(id);
  resolveCollector();
}

async function resolveCollector() {
  const ids = [...state.collector.keys()];
  if (ids.length) {
    try {
      const resolved = await api.resolveItems(ids);
      const next = new Map();
      for (const r of resolved) next.set(r.id, r);
      state.collector = next;
    } catch {
      /* keep the optimistic state */
    }
  }
  renderCollector();
  document.querySelectorAll('#p-body .row[data-id]').forEach((r) => r.classList.toggle('collected', state.collector.has(+r.dataset.id)));
}

function collectorTotal() {
  let total = 0;
  for (const it of state.collector.values()) if (!it.covered) total += it.size;
  return total;
}

function renderCollector() {
  const el = $('#collector');
  if (!el) return;
  const items = [...state.collector.values()];
  if (!items.length) {
    el.innerHTML = `
      <div class="collector-icon">${icon('inbox')}</div>
      <div class="collector-hint">${esc(t('collector_hint'))}</div>`;
    return;
  }
  const total = collectorTotal();
  const live = items.filter((i) => !i.covered).length;
  el.innerHTML = `
    <div class="collector-icon">${icon('trash')}<span class="badge">${live}</span></div>
    <div class="collector-items">
      ${items
        .map(
          (it) => `
        <div class="citem${it.covered ? ' covered' : ''}" title="${esc(it.path || it.name)}">
          <span class="dot" style="background:${colorFor(it)}"></span>
          <span class="cname">${esc(it.name)}</span>
          <span class="csize">${esc(bytes(it.size))}</span>
          <button type="button" data-remove="${it.id}">${icon('x')}</button>
        </div>`,
        )
        .join('')}
    </div>
    <div class="collector-actions">
      <div class="collector-total"><b>${esc(bytes(total))}</b><span>${esc(t('collector_total', { n: live, size: '' }).replace(/ · $/, ''))}</span></div>
      <button class="btn ghost" id="c-clear" type="button">${esc(t('clear'))}</button>
      <button class="btn danger" id="c-trash" type="button">${icon('trash')}${esc(t(isWindows() ? 'move_to_recycle' : 'move_to_trash'))}</button>
    </div>`;
  el.querySelectorAll('[data-remove]').forEach((b) => b.addEventListener('click', () => removeFromCollector(+b.dataset.remove)));
  $('#c-clear').addEventListener('click', () => {
    state.collector.clear();
    resolveCollector();
  });
  $('#c-trash').addEventListener('click', confirmTrash);
}

function confirmTrash() {
  const items = [...state.collector.values()].filter((i) => !i.covered);
  if (!items.length) return;
  const recycle = isWindows();
  const total = collectorTotal();
  const title =
    items.length === 1
      ? t(recycle ? 'confirm_title_recycle_one' : 'confirm_title_one', { name: items[0].name })
      : t(recycle ? 'confirm_title_recycle' : 'confirm_title', { n: count(items.length) });
  const shown = items.slice(0, 50);
  const root = $('#modal-root');
  root.innerHTML = `
    <div class="modal-overlay">
      <div class="modal" role="dialog" aria-modal="true">
        <div class="modal-icon">${icon('trash')}</div>
        <h3>${esc(title)}</h3>
        <p>${esc(t(recycle ? 'confirm_body_recycle' : 'confirm_body', { size: bytes(total) }))}</p>
        <div class="modal-list">
          ${shown.map((i) => `<div class="mrow"><span class="mpath" title="${esc(i.path)}">&lrm;${esc(i.path || i.name)}</span><b>${esc(bytes(i.size))}</b></div>`).join('')}
          ${items.length > shown.length ? `<div class="mrow"><span>${esc(t('and_more', { n: items.length - shown.length }))}</span><span></span></div>` : ''}
        </div>
        <div class="modal-actions">
          <button class="btn" id="m-cancel" type="button">${esc(t('cancel'))}</button>
          <button class="btn danger" id="m-ok" type="button">${icon('trash')}${esc(t(recycle ? 'move_to_recycle' : 'move_to_trash').replace('…', ''))}</button>
        </div>
      </div>
    </div>`;
  const close = () => (root.innerHTML = '');
  $('#m-cancel').addEventListener('click', close);
  root.querySelector('.modal-overlay').addEventListener('click', (e) => e.target === e.currentTarget && close());
  $('#m-ok').addEventListener('click', async () => {
    $('#m-ok').disabled = true;
    $('#m-cancel').disabled = true;
    try {
      const report = await api.trashNodes(items.map((i) => i.id));
      close();
      for (const id of report.removed) state.collector.delete(id);
      if (report.stats) state.result.stats = report.stats;
      if (report.failed.length && report.removed.length) {
        toast(t('trash_partial', { ok: report.removed.length, failed: report.failed.length }), 'err');
      } else if (report.failed.length) {
        toast(t('trash_failed', { name: report.failed[0].name, error: report.failed[0].error }), 'err');
      } else {
        toast(t('trash_done', { size: bytes(report.freed) }));
      }
      await resolveCollector();
      renderStatus();
      renderUsage();
      navigate(state.focusId, 'pop');
    } catch (e) {
      close();
      toast(errText(e), 'err');
    }
  });
  $('#m-ok').focus();
}

// ------------------------------------------------------------------ updates

const CHANGELOG_URL = URLS.changelog;
const DOWNLOAD_URL = URLS.download;
const update = {
  info: null,
  checked: false,
  checking: false,
  checkError: null,
  installing: false,
  error: null,
  progress: null,
  dismissed: false,
};

/** Returns true when an update was found, false when none, undefined on error. */
async function checkForUpdate(manual = false) {
  if (isPreview || update.installing || update.checking) return false;
  update.checking = true;
  update.checkError = null;
  refreshSettings();
  try {
    update.info = await api.checkUpdate();
    update.checked = true;
    update.error = null;
    if (manual) update.dismissed = false;
    setPref('updateChecked', Date.now());
  } catch (e) {
    update.checkError = errText(e);
    if (manual && !settingsOpen()) toast(t('upd_check_failed', { error: update.checkError }), 'err');
    return undefined;
  } finally {
    update.checking = false;
    renderUpdateBanner();
    refreshSettings();
  }
  return !!update.info;
}

function onUpdateProgress(p) {
  update.progress = p;
  renderUpdateBanner();
  settingsProgress(p);
}

function updateBadge() {
  const b = $('#btn-settings');
  if (b) b.classList.toggle('has-badge', !!update.info && !update.installing);
}

function renderUpdateBanner() {
  updateBadge();
  const el = $('#update-banner');
  if (!el) return;
  if (!update.info || (update.dismissed && !update.installing && !update.error)) {
    el.hidden = true;
    return;
  }
  el.hidden = false;
  if (update.error) {
    el.innerHTML = `
      <div class="ub-icon warn">${icon('warn')}</div>
      <div class="ub-text" title="${esc(update.error)}">${esc(t('update_failed', { error: update.error }))}</div>
      <button class="btn btn-sm" type="button" data-act="retry">${esc(t('update_install'))}</button>
      <button class="btn btn-sm primary" type="button" data-act="download">${esc(t('update_download_manually'))}</button>
      <button class="icon-btn" type="button" data-act="later" title="${esc(t('update_later'))}">${icon('x')}</button>`;
    el.onclick = (e) => {
      const act = e.target.closest('[data-act]')?.dataset.act;
      if (act === 'download') api.openExternal(DOWNLOAD_URL).catch(() => {});
      if (act === 'retry') installUpdate();
      if (act === 'later') {
        update.error = null;
        update.dismissed = true;
        renderUpdateBanner();
      }
    };
    return;
  }
  if (update.installing) {
    const p = update.progress;
    const pct = p && p.total ? Math.round((p.downloaded / p.total) * 100) : null;
    const done = pct === 100;
    el.innerHTML = `
      <div class="ub-icon">${icon('refresh')}</div>
      <div class="ub-text">${esc(done ? t('update_installing') : t('update_downloading', { pct: pct == null ? '' : `${pct}%` }))}
        <div class="ub-bar"><div style="width:${pct ?? 8}%"></div></div>
      </div>`;
    el.onclick = null;
    return;
  }
  el.innerHTML = `
    <div class="ub-icon">${icon('sparkle')}</div>
    <div class="ub-text"><b>${esc(t('update_available', { version: update.info.version }))}</b>
      <button class="ub-link" type="button" data-act="notes">${esc(t('update_whats_new'))}</button></div>
    <button class="btn btn-sm primary" type="button" data-act="install">${esc(t('update_install'))}</button>
    <button class="icon-btn" type="button" data-act="later" title="${esc(t('update_later'))}">${icon('x')}</button>`;
  el.onclick = (e) => {
    const act = e.target.closest('[data-act]')?.dataset.act;
    if (act === 'notes') api.openExternal(`${CHANGELOG_URL}#v${update.info.version}`).catch(() => {});
    if (act === 'later') {
      update.dismissed = true;
      renderUpdateBanner();
    }
    if (act === 'install') installUpdate();
  };
}

async function installUpdate() {
  if (document.body.dataset.screen === 'scan' || update.installing || !update.info) return;
  update.installing = true;
  update.error = null;
  update.progress = null;
  update.dismissed = false;
  renderUpdateBanner();
  refreshSettings();
  try {
    await api.installUpdate(); // Restarts the app when it succeeds.
  } catch (e) {
    update.installing = false;
    update.error = errText(e);
    renderUpdateBanner();
    refreshSettings();
  }
}

// ------------------------------------------------------------------ setup

function setupTitlebar() {
  $('#btn-home').innerHTML = icon('home');
  $('#btn-home').addEventListener('click', showHome);
  $('#btn-up').innerHTML = icon('up');
  $('#btn-up').addEventListener('click', goUp);
  $('#btn-rescan').innerHTML = icon('refresh');
  $('#btn-rescan').addEventListener('click', rescanCurrent);
  $('#btn-theme').addEventListener('click', toggleTheme);
  $('#btn-settings').innerHTML = `${icon('gear')}<i class="badge-dot"></i>`;
  $('#btn-settings').addEventListener('click', () => toggleSettings());
  labelTitlebar();
}

/** Tooltips of the title bar buttons (again after the language changes). */
function labelTitlebar() {
  const label = (sel, text) => {
    $(sel).title = text;
    $(sel).setAttribute('aria-label', text);
  };
  label('#btn-home', t('back_home'));
  label('#btn-up', t('up'));
  label('#btn-rescan', t('rescan_tip'));
  label('#btn-theme', t('toggle_theme'));
  label('#btn-settings', `${t('settings')} (${isMac() ? '⌘' : 'Ctrl+'},)`);
}

/** Rescan reuses unchanged folders; hold Alt/Option for a full rescan. */
function rescanCurrent(e) {
  if (state.result) startScan(state.result.stats.root_path, state.scanLabel, { full: !!(e && e.altKey) });
}

function setupKeyboard() {
  window.addEventListener('keydown', (e) => {
    const inInput = e.target instanceof HTMLInputElement;
    const mod = e.metaKey || e.ctrlKey;
    if (e.key === 'Escape') {
      if (settingsOpen()) {
        closeSettings();
        return;
      }
      if ($('#modal-root').innerHTML) {
        $('#modal-root').innerHTML = '';
        return;
      }
      if (!$('#ctxmenu').hidden) {
        hideMenu();
        return;
      }
      if (document.body.dataset.screen === 'result') goUp();
      return;
    }
    if (mod && e.key === ',') {
      e.preventDefault();
      toggleSettings();
      return;
    }
    // Shortcuts below act on the screen behind an open dialog: not now.
    if ($('#modal-root').innerHTML) return;
    if (mod && e.key.toLowerCase() === 'r') {
      e.preventDefault();
      if (document.body.dataset.screen === 'result') rescanCurrent();
      return;
    }
    if (document.body.dataset.screen !== 'result') return;
    if ((mod && e.key.toLowerCase() === 'f') || (!inInput && e.key === '/')) {
      e.preventDefault();
      $('#search-input')?.focus();
      return;
    }
    if (!inInput && (e.key === 'Backspace' || (e.altKey && e.key === 'ArrowUp'))) {
      e.preventDefault();
      goUp();
    }
  });
}

function setupGlobalDismiss() {
  window.addEventListener('pointerdown', (e) => {
    if (!e.target.closest('#ctxmenu')) hideMenu();
  });
  window.addEventListener('blur', () => {
    hideMenu();
    hideTooltip();
  });
  window.addEventListener('resize', hideMenu);
  // Suppress the browser context menu everywhere except text inputs.
  window.addEventListener('contextmenu', (e) => {
    if (!(e.target instanceof HTMLInputElement)) e.preventDefault();
  });
}

function reportError(message) {
  console.error(message);
  if (document.body) toast(String(message).slice(0, 300), 'err');
}

window.addEventListener('error', (e) => reportError(e.message || e.error));
window.addEventListener('unhandledrejection', (e) => reportError(errText(e.reason)));

const withTimeout = (promise, ms, label) =>
  Promise.race([promise, new Promise((_, reject) => setTimeout(() => reject(new Error(`${label} timed out`)), ms))]);

async function init() {
  applyTheme(loadTheme());
  try {
    state.info = { ...state.info, ...(await withTimeout(api.appInfo(), 4000, 'app_info')) };
  } catch (e) {
    reportError(errText(e));
  }
  // The platform class reserves room for native window controls, which the
  // browser preview (website demo) does not have.
  if (!isPreview) document.body.classList.add(`platform-${state.info.os}`);
  document.documentElement.lang = lang();
  applyUnits();
  applyMotion();
  setupTitlebar();
  initSettings({
    info: () => state.info,
    update,
    scanning: () => document.body.dataset.screen === 'scan',
    timeAgo,
    apply: applyPref,
    checkForUpdate,
    installUpdate,
    toast,
    onScansCleared: () => {
      state.saved.clear();
      if (document.body.dataset.screen === 'home') loadVolumes();
    },
  });
  setupKeyboard();
  setupGlobalDismiss();
  await listen('scan-progress', onProgress);
  await listen('scan-done', onDone);
  await listen('scan-failed', onFailed);
  await listen('update-progress', onUpdateProgress);
  matchMedia('(prefers-color-scheme: light)').addEventListener?.('change', (ev) => {
    if (getPref('theme') === 'system') applyTheme(ev.matches ? 'light' : 'dark');
  });
  matchMedia('(prefers-reduced-motion: reduce)').addEventListener?.('change', applyMotion);

  if (isPreview) {
    const b = document.createElement('div');
    b.className = 'preview-badge';
    b.textContent = t('preview_mode');
    document.body.appendChild(b);
  }

  // Look for a new version a few seconds after start (can be switched off).
  if (!isPreview && getPref('updateCheck') === 'on') setTimeout(() => checkForUpdate(false), 4000);

  const params = new URLSearchParams(location.search);
  if (params.get('demo') === 'scan') {
    showHome();
    startScan(params.get('path') || '/', params.get('label') || 'Macintosh HD');
  } else {
    showHome();
  }
}

init();
