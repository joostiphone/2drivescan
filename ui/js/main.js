import { api, listen, isPreview } from './api.js';
import { t } from './i18n.js';
import { bytes, bytesParts, count, percent, duration, truncateMiddle, escapeHtml as esc, setBinaryUnits } from './format.js';
import { icon } from './icons.js';
import { Sunburst } from './sunburst.js';
import { CATEGORIES, CATEGORY_COLORS, HIDDEN_COLOR, GROUP_COLOR, css } from './colors.js';

const RINGS = 5;
const LIST_LIMIT = 1500;
const REPO_URL = 'https://github.com/joostiphone/2drivescan';
const FDA_URL = 'x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles';

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
  colorMode: 'folder',
  query: '',
  collector: new Map(),
  rows: new Map(),
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

function loadTheme() {
  let saved = null;
  try {
    saved = localStorage.getItem('2ds-theme');
  } catch {
    /* storage unavailable */
  }
  const fromUrl = new URLSearchParams(location.search).get('theme');
  return fromUrl || saved || (matchMedia('(prefers-color-scheme: light)').matches ? 'light' : 'dark');
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
  try {
    localStorage.setItem('2ds-theme', next);
  } catch {
    /* ignore */
  }
  applyTheme(next);
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
      </div>
    </div>`;
  el.querySelector('[data-url]').addEventListener('click', (e) => api.openExternal(e.currentTarget.dataset.url).catch(() => {}));
  loadVolumes();
}

async function loadVolumes() {
  try {
    state.volumes = await api.listVolumes();
  } catch {
    state.volumes = [];
  }
  const grid = $('#vol-grid');
  if (!grid) return;
  const cards = state.volumes.map((v, i) => {
    const used = Math.max(0, v.total - v.available);
    const chips = [v.file_system && v.file_system.toUpperCase(), v.kind !== 'unknown' && v.kind.toUpperCase(), v.removable && t('removable')]
      .filter(Boolean)
      .map((c) => `<span class="chip">${esc(c)}</span>`)
      .join('');
    return `
      <button class="vol-card glass" data-vol="${i}" type="button">
        ${gauge(used, v.total, i)}
        <div class="vol-info">
          <div class="vol-name">${icon(v.removable ? 'usb' : 'drive')}<span>${esc(volumeLabel(v))}</span></div>
          <div class="vol-sub">${esc(v.mount_point)}</div>
          <div class="vol-free">${esc(t('free_of', { free: bytes(v.available), total: bytes(v.total) }))}</div>
          <div class="chips">${chips}</div>
        </div>
      </button>`;
  });
  if (state.info.home) {
    cards.push(`
      <button class="vol-card glass action" data-action="home" type="button">
        <div class="action-icon">${icon('user')}</div>
        <div class="vol-info">
          <div class="vol-name">${esc(t('home_folder'))}</div>
          <div class="vol-sub">${esc(state.info.home)}</div>
          <div class="vol-free">${esc(t('home_folder_hint'))}</div>
        </div>
      </button>`);
  }
  cards.push(`
    <button class="vol-card glass action" data-action="pick" type="button">
      <div class="action-icon grad">${icon('folderPlus')}</div>
      <div class="vol-info">
        <div class="vol-name">${esc(t('scan_folder'))}</div>
        <div class="vol-free">${esc(t('scan_folder_hint'))}</div>
      </div>
    </button>`);
  grid.innerHTML = cards.join('');
  if (!state.volumes.length) grid.insertAdjacentHTML('afterbegin', `<div class="empty" style="grid-column:1/-1">${esc(t('no_volumes'))}</div>`);

  grid.querySelectorAll('.vol-card').forEach((card) => {
    card.addEventListener('pointermove', (e) => {
      const r = card.getBoundingClientRect();
      card.style.setProperty('--mx', `${e.clientX - r.left}px`);
      card.style.setProperty('--my', `${e.clientY - r.top}px`);
    });
    card.addEventListener('click', async () => {
      if (card.dataset.vol != null) {
        const v = state.volumes[+card.dataset.vol];
        startScan(v.mount_point, volumeLabel(v));
      } else if (card.dataset.action === 'home') {
        startScan(state.info.home, t('home_folder'));
      } else {
        const path = await api.pickFolder().catch(() => null);
        if (path) startScan(path, path.split(/[\\/]/).filter(Boolean).pop() || path);
      }
    });
  });
  requestAnimationFrame(() =>
    requestAnimationFrame(() => {
      grid.querySelectorAll('.gauge .value').forEach((c) => (c.style.strokeDashoffset = c.dataset.target));
    }),
  );
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
      <div class="scan-path" id="sc-path">${esc(t('preparing'))}</div>
      <button class="btn" id="sc-cancel" type="button">${icon('x')} ${esc(t('cancel'))}</button>
    </div>`;
  $('#sc-cancel').addEventListener('click', () => api.cancelScan());
}

async function startScan(path, label) {
  if (!path) return;
  state.scanPath = path;
  state.scanLabel = label || path;
  state.scanId = null;
  renderScan();
  setScreen('scan');
  try {
    state.scanId = await api.startScan(path);
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
}

function onDone(d) {
  if (state.scanId != null && d.scan_id !== state.scanId) return;
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
    rings: RINGS,
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
  const { stats, elapsed_ms } = state.result;
  let html = `<span>${esc(t('scanned_summary', { files: count(stats.files), time: duration(elapsed_ms) }))}</span>`;
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
    view = await api.getView(id, mode === 'out' ? RINGS + 1 : RINGS);
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

// ------------------------------------------------------------------ setup

function setupTitlebar() {
  const home = $('#btn-home');
  home.innerHTML = icon('home');
  home.title = t('back_home');
  home.addEventListener('click', showHome);
  const up = $('#btn-up');
  up.innerHTML = icon('up');
  up.title = t('up');
  up.addEventListener('click', goUp);
  const rescan = $('#btn-rescan');
  rescan.innerHTML = icon('refresh');
  rescan.title = t('rescan');
  rescan.addEventListener('click', rescanCurrent);
  const theme = $('#btn-theme');
  theme.title = t('toggle_theme');
  theme.addEventListener('click', toggleTheme);
}

function rescanCurrent() {
  if (state.result) startScan(state.result.stats.root_path, state.scanLabel);
}

function setupKeyboard() {
  window.addEventListener('keydown', (e) => {
    const inInput = e.target instanceof HTMLInputElement;
    const mod = e.metaKey || e.ctrlKey;
    if (e.key === 'Escape') {
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
  document.body.classList.add(`platform-${state.info.os}`);
  setBinaryUnits(isWindows());
  setupTitlebar();
  setupKeyboard();
  setupGlobalDismiss();
  await listen('scan-progress', onProgress);
  await listen('scan-done', onDone);
  await listen('scan-failed', onFailed);
  matchMedia('(prefers-color-scheme: light)').addEventListener?.('change', (ev) => {
    let saved = null;
    try {
      saved = localStorage.getItem('2ds-theme');
    } catch {
      /* ignore */
    }
    if (!saved) applyTheme(ev.matches ? 'light' : 'dark');
  });

  if (isPreview) {
    const b = document.createElement('div');
    b.className = 'preview-badge';
    b.textContent = t('preview_mode');
    document.body.appendChild(b);
  }

  const params = new URLSearchParams(location.search);
  if (params.get('demo') === 'scan') {
    showHome();
    startScan(params.get('path') || '/', params.get('label') || 'Macintosh HD');
  } else {
    showHome();
  }
}

init();
