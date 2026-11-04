// The settings dialog: appearance, language, chart, updates, saved scans
// and about. Changes apply immediately; main.js does the work through hooks.

import { api, isPreview } from './api.js';
import { t, LANGUAGES } from './i18n.js';
import { bytes, escapeHtml as esc } from './format.js';
import { icon } from './icons.js';
import { getPref } from './prefs.js';

export const URLS = {
  site: 'https://2drivescan.app',
  download: 'https://2drivescan.app/download',
  changelog: 'https://2drivescan.app/changelog',
  repo: 'https://github.com/joostiphone/2drivescan',
  bug: 'https://github.com/joostiphone/2drivescan/issues/new?template=bug_report.yml',
  idea: 'https://github.com/joostiphone/2drivescan/issues/new?template=feature_request.yml',
  fda: 'x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles',
};

const PAGES = [
  ['general', 'sliders', 'set_general'],
  ['updates', 'sparkle', 'set_updates'],
  ['scans', 'drive', 'set_scans'],
  ['about', 'info', 'set_about'],
];

/**
 * hooks: { info(), update, scanning(), timeAgo(ms), apply(name, value),
 *          checkForUpdate(manual), installUpdate(), onScansCleared(), toast(msg, kind) }
 */
let hooks = null;
let page = 'general';
let storage = null;
let confirmClear = false;
let busy = false;
let opener = null;

export function initSettings(h) {
  hooks = h;
}

const root = () => document.getElementById('modal-root');
const dialog = () => root().querySelector('.settings');

export function settingsOpen() {
  return !!dialog();
}

export function openSettings(which = page) {
  page = PAGES.some((p) => p[0] === which) ? which : 'general';
  confirmClear = false;
  storage = null;
  opener = document.activeElement;
  document.body.classList.add('settings-open');
  root().innerHTML = `<div class="modal-overlay settings-overlay"><div class="settings" role="dialog" aria-modal="true" aria-labelledby="set-h"></div></div>`;
  const overlay = root().querySelector('.settings-overlay');
  overlay.addEventListener('pointerdown', (e) => {
    if (e.target === overlay) closeSettings();
  });
  const el = dialog();
  el.addEventListener('click', onClick);
  el.addEventListener('keydown', onKeydown);
  render();
  el.querySelector('.set-nav button.active')?.focus();
}

export function closeSettings() {
  if (!settingsOpen()) return;
  root().innerHTML = '';
  document.body.classList.remove('settings-open');
  if (opener && document.contains(opener)) opener.focus?.();
  opener = null;
}

export function toggleSettings() {
  if (settingsOpen()) closeSettings();
  else openSettings();
}

/** Re-renders the open dialog, e.g. after the language or update state changed. */
export function refreshSettings() {
  if (!settingsOpen()) return;
  render();
}

/** Moves the install progress bar without re-rendering the dialog. */
export function settingsProgress(progress) {
  if (!settingsOpen() || page !== 'updates') return;
  const bar = dialog().querySelector('.upd-bar > div');
  const label = dialog().querySelector('.upd-status');
  if (!bar || !label) return render();
  const pct = progress && progress.total ? Math.round((progress.downloaded / progress.total) * 100) : null;
  bar.style.width = `${pct ?? 8}%`;
  label.textContent = pct === 100 ? t('update_installing') : t('update_downloading', { pct: pct == null ? '' : `${pct}%` });
}

// ---------------------------------------------------------------- rendering

function render() {
  const el = dialog();
  if (!el) return;
  const focused = document.activeElement?.dataset?.focusKey;
  const scroll = el.querySelector('.set-body')?.scrollTop || 0;
  const title = PAGES.find((p) => p[0] === page)[2];
  el.innerHTML = `
    <nav class="set-nav" aria-label="${esc(t('settings'))}">
      <div class="set-nav-title" id="set-h">${esc(t('settings'))}</div>
      ${PAGES.map(
        ([id, ic, label]) => `
        <button type="button" data-page="${id}" data-focus-key="page-${id}" class="${id === page ? 'active' : ''}" aria-current="${id === page ? 'page' : 'false'}">
          ${icon(ic)}<span>${esc(t(label))}</span>${id === 'updates' && hooks.update.info && !isPreview ? '<i class="set-dot"></i>' : ''}
        </button>`,
      ).join('')}
    </nav>
    <div class="set-main">
      <div class="set-head">
        <h3>${esc(t(title))}</h3>
        <button class="icon-btn" type="button" data-act="close" title="${esc(t('close'))}" aria-label="${esc(t('close'))}">${icon('x')}</button>
      </div>
      <div class="set-body">${PAGE_RENDER[page]()}</div>
    </div>`;
  el.querySelector('.set-body').scrollTop = scroll;
  if (focused) el.querySelector(`[data-focus-key="${focused}"]`)?.focus();
  if (page === 'scans' && !storage) loadStorage();
}

function seg(pref, options, value = getPref(pref)) {
  return `
    <div class="set-seg" role="radiogroup">
      ${options
        .map(
          ([v, label]) =>
            `<button type="button" role="radio" aria-checked="${v === value}" class="${v === value ? 'active' : ''}" data-pref="${pref}" data-value="${esc(v)}" data-focus-key="${pref}-${esc(v)}">${esc(label)}</button>`,
        )
        .join('')}
    </div>`;
}

function row(label, hint, control) {
  return `
    <div class="set-row">
      <div class="set-label"><b>${esc(label)}</b>${hint ? `<span>${esc(hint)}</span>` : ''}</div>
      <div class="set-control">${control}</div>
    </div>`;
}

function toggle(pref, on) {
  return `<button type="button" class="switch${on ? ' on' : ''}" role="switch" aria-checked="${on}" data-toggle="${pref}" data-focus-key="${pref}"><i></i></button>`;
}

const PAGE_RENDER = {
  general() {
    return `
      <div class="set-group">
        <div class="set-group-title">${esc(t('set_appearance'))}</div>
        <div class="set-card">
          ${row(t('set_theme'), '', seg('theme', [['system', t('opt_system')], ['light', t('theme_light')], ['dark', t('theme_dark')]]))}
          ${row(t('set_language'), '', seg('language', [['system', t('opt_system')], ...LANGUAGES]))}
          ${row(t('set_units'), t('set_units_hint'), seg('units', [['auto', t('opt_auto')], ['decimal', '1000'], ['binary', '1024']]))}
        </div>
      </div>
      <div class="set-group">
        <div class="set-group-title">${esc(t('set_chart'))}</div>
        <div class="set-card">
          ${row(t('color_by'), t('set_color_hint'), seg('colorMode', [['folder', t('color_folders')], ['type', t('color_types')]]))}
          ${row(t('set_rings'), t('set_rings_hint'), seg('rings', ['4', '5', '6', '7'].map((n) => [n, n])))}
          ${row(t('set_motion'), t('set_motion_hint'), seg('motion', [['auto', t('opt_auto')], ['on', t('opt_on')], ['off', t('opt_off')]]))}
        </div>
      </div>`;
  },

  updates() {
    const u = hooks.update;
    const info = hooks.info();
    let status = '';
    let cls = '';
    let actions = '';
    let extra = '';
    if (isPreview) {
      status = t('upd_preview');
      actions = `<button class="btn primary btn-sm" type="button" data-url="download">${icon('download')}${esc(t('upd_download_app'))}</button>`;
    } else if (u.installing) {
      const p = u.progress;
      const pct = p && p.total ? Math.round((p.downloaded / p.total) * 100) : null;
      status = pct === 100 ? t('update_installing') : t('update_downloading', { pct: pct == null ? '' : `${pct}%` });
      cls = 'new';
      extra = `<div class="upd-bar"><div style="width:${pct ?? 8}%"></div></div>`;
    } else if (u.info && u.error) {
      status = t('update_failed', { error: u.error });
      cls = 'err';
      actions = `
        <button class="btn btn-sm primary" type="button" data-act="install">${icon('refresh')}${esc(t('update_install'))}</button>
        <button class="btn btn-sm" type="button" data-url="download">${esc(t('update_download_manually'))}</button>`;
    } else if (u.info) {
      status = t('update_available', { version: u.info.version });
      cls = 'new';
      const scanning = hooks.scanning();
      actions = `
        <button class="btn btn-sm primary" type="button" data-act="install" ${scanning ? 'disabled' : ''}>${icon('download')}${esc(t('update_install'))}</button>
        <button class="btn btn-sm ghost" type="button" data-url="notes">${esc(t('update_whats_new'))}</button>`;
      if (scanning) extra = `<p class="set-note warn">${esc(t('upd_after_scan'))}</p>`;
    } else {
      if (u.checking) status = t('update_checking');
      else if (u.checkError) {
        status = t('upd_check_failed', { error: u.checkError });
        cls = 'err';
      } else if (u.checked) {
        status = t('upd_latest');
        cls = 'ok';
      } else status = t('upd_not_checked');
      const when = +getPref('updateChecked');
      if (!u.checking && !u.checkError && when) extra = `<p class="set-note">${esc(t('upd_checked_ago', { when: hooks.timeAgo(when) }))}</p>`;
      actions = `<button class="btn btn-sm" type="button" data-act="check" data-focus-key="check" ${u.checking ? 'disabled' : ''}>${icon('refresh')}${esc(u.checking ? t('update_checking') : t('upd_check_now'))}</button>`;
    }
    const statusIcon = cls === 'ok' ? icon('check') : cls === 'err' ? icon('warn') : cls === 'new' ? icon('sparkle') : '';
    return `
      <div class="upd-card">
        <img src="img/logo.png" alt="" width="56" height="56" />
        <div class="upd-info">
          <b>2DriveScan ${esc(info.version || '')}</b>
          <span class="upd-status ${cls}">${statusIcon}<span>${esc(status)}</span></span>
        </div>
        <div class="upd-actions">${actions}</div>
        ${extra ? `<div class="upd-extra">${extra}</div>` : ''}
      </div>
      ${
        isPreview
          ? ''
          : `<div class="set-group">
        <div class="set-card">${row(t('upd_auto'), t('upd_auto_hint'), toggle('updateCheck', getPref('updateCheck') === 'on'))}</div>
      </div>
      <p class="set-note">${icon('lock')}${esc(t('upd_signed'))}</p>`
      }`;
  },

  scans() {
    const scanning = hooks.scanning();
    let summary = esc(t('loading'));
    if (storage) {
      const size = bytes(storage.bytes);
      if (!storage.count && !storage.bytes) summary = esc(t('scans_none'));
      else summary = esc(storage.count === 1 ? t('scans_count_one', { size }) : t('scans_count', { n: storage.count, size }));
    }
    const empty = storage && !storage.count && !storage.bytes;
    const confirm = confirmClear
      ? `<div class="set-confirm">
          <p>${esc(t('scans_clear_confirm'))}</p>
          <div class="set-confirm-actions">
            <button class="btn btn-sm" type="button" data-act="clear-cancel" data-focus-key="clear-cancel">${esc(t('cancel'))}</button>
            <button class="btn btn-sm danger" type="button" data-act="clear-ok" ${busy ? 'disabled' : ''}>${esc(t('scans_clear_ok'))}</button>
          </div>
        </div>`
      : '';
    const showFda = hooks.info().os === 'macos' && !isPreview;
    return `
      <div class="set-group">
        <div class="set-group-title">${esc(t('scans_saved'))}</div>
        <div class="set-card">
          <div class="set-row">
            <div class="set-label"><b class="scans-summary">${summary}</b><span>${esc(t('scans_saved_hint', { keep: storage?.keep ?? 12 }))}</span>${scanning ? `<span class="warn">${esc(t('scans_after_scan'))}</span>` : ''}</div>
            <div class="set-control">
              <button class="btn btn-sm" type="button" data-act="clear" data-focus-key="clear" ${!storage || empty || scanning || confirmClear ? 'disabled' : ''}>${esc(t('scans_clear'))}</button>
            </div>
          </div>
          ${confirm}
        </div>
      </div>
      ${
        showFda
          ? `<div class="set-group">
        <div class="set-group-title">${esc(t('fda_title'))}</div>
        <div class="set-card">${row(t('fda_title'), t('fda_hint'), `<button class="btn btn-sm" type="button" data-url="fda">${icon('lock')}${esc(t('fda_open'))}</button>`)}</div>
      </div>`
          : ''
      }`;
  },

  about() {
    const info = hooks.info();
    const mac = info.os === 'macos';
    const mod = mac ? '⌘' : 'Ctrl';
    const alt = mac ? '⌥' : 'Alt';
    const k = (...keys) => keys.map((x) => `<kbd>${esc(x)}</kbd>`).join('<span class="kplus">+</span>');
    const shortcuts = [
      [t('sc_up'), `${k('Esc')} <span class="kor">/</span> ${k('⌫')}`],
      [t('sc_search'), `${k('/')} <span class="kor">/</span> ${k(mod, 'F')}`],
      [t('sc_rescan'), k(mod, 'R')],
      [t('sc_full'), `${k(alt)}<span class="kplus">+</span><span class="kword">${esc(t('sc_click_rescan'))}</span>`],
      [t('sc_settings'), k(mod, ',')],
    ];
    const link = (key, ic, label) => `<button class="about-link" type="button" data-url="${key}">${icon(ic)}<span>${esc(label)}</span></button>`;
    return `
      <div class="about-head">
        <img src="img/logo.png" alt="" width="72" height="72" />
        <div>
          <h4>2DriveScan</h4>
          <div class="about-version">${esc(t('version', { v: info.version || '' }))}</div>
          <p>${esc(t('tagline'))}</p>
        </div>
      </div>
      <div class="about-links">
        ${link('site', 'globe', t('about_website'))}
        ${link('repo', 'github', 'GitHub')}
        ${link('changelog', 'list', t('about_changelog'))}
        ${link('bug', 'bug', t('about_report'))}
        ${link('idea', 'sparkle', t('about_idea'))}
      </div>
      <div class="set-group">
        <div class="set-group-title">${esc(t('about_shortcuts'))}</div>
        <div class="set-card shortcuts">
          ${shortcuts.map(([label, keys]) => `<div class="sc-row"><span>${esc(label)}</span><span class="sc-keys">${keys}</span></div>`).join('')}
        </div>
      </div>
      <p class="set-note center">${esc(t('about_license'))} ${esc(t('about_built'))}</p>`;
  },
};

// ---------------------------------------------------------------- actions

async function loadStorage() {
  try {
    storage = await api.scanStorage();
  } catch {
    storage = { count: 0, bytes: 0, keep: 12 };
  }
  if (page === 'scans') render();
}

function urlFor(key) {
  if (key === 'notes') return `${URLS.changelog}#v${hooks.update.info?.version ?? ''}`;
  return URLS[key];
}

async function onClick(e) {
  const target = e.target.closest('button');
  if (!target || target.disabled) return;
  const d = target.dataset;
  if (d.page) {
    page = d.page;
    confirmClear = false;
    render();
    dialog().querySelector(`[data-page="${page}"]`)?.focus();
    return;
  }
  if (d.pref) {
    hooks.apply(d.pref, d.value);
    render();
    return;
  }
  if (d.toggle) {
    hooks.apply(d.toggle, getPref(d.toggle) === 'on' ? 'off' : 'on');
    render();
    return;
  }
  if (d.url) {
    api.openExternal(urlFor(d.url)).catch(() => {});
    return;
  }
  switch (d.act) {
    case 'close':
      closeSettings();
      break;
    case 'check':
      await hooks.checkForUpdate(true);
      break;
    case 'install':
      hooks.installUpdate();
      break;
    case 'clear':
      confirmClear = true;
      render();
      dialog().querySelector('[data-act="clear-cancel"]')?.focus();
      break;
    case 'clear-cancel':
      confirmClear = false;
      render();
      break;
    case 'clear-ok':
      await clearScans();
      break;
  }
}

async function clearScans() {
  busy = true;
  render();
  const before = storage?.bytes ?? 0;
  try {
    storage = await api.clearSavedScans();
    hooks.toast(t('scans_cleared', { size: bytes(Math.max(0, before - storage.bytes)) }));
    hooks.onScansCleared();
  } catch (e) {
    hooks.toast(typeof e === 'string' ? e : e?.message || String(e), 'err');
    storage = null;
  }
  busy = false;
  confirmClear = false;
  render();
}

function onKeydown(e) {
  // Arrow keys move between the options of a choice, like native controls.
  const b = e.target.closest?.('.set-seg button, .set-nav button');
  if (!b || !['ArrowLeft', 'ArrowRight', 'ArrowUp', 'ArrowDown'].includes(e.key)) return;
  const group = [...b.parentElement.querySelectorAll('button')];
  const step = e.key === 'ArrowLeft' || e.key === 'ArrowUp' ? -1 : 1;
  const next = group[(group.indexOf(b) + step + group.length) % group.length];
  e.preventDefault();
  next.focus();
  next.click();
}
