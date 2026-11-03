import { lang } from './i18n.js';

let binaryUnits = false;

/** Windows Explorer uses 1024-based units, macOS and most Linux desktops 1000. */
export function setBinaryUnits(on) {
  binaryUnits = !!on;
}

const UNITS = ['B', 'KB', 'MB', 'GB', 'TB', 'PB'];

export function bytes(n, { precise = false } = {}) {
  if (n == null || isNaN(n)) return '—';
  const base = binaryUnits ? 1024 : 1000;
  let i = 0;
  let v = Math.abs(n);
  while (v >= base && i < UNITS.length - 1) {
    v /= base;
    i++;
  }
  const digits = i === 0 ? 0 : v >= 100 ? 0 : v >= 10 ? 1 : precise ? 2 : 1;
  return `${v.toLocaleString(lang(), { maximumFractionDigits: digits, minimumFractionDigits: i === 0 ? 0 : Math.min(digits, 1) })} ${UNITS[i]}`;
}

/** Splits a size into number and unit for big typographic displays. */
export function bytesParts(n) {
  const s = bytes(n);
  const idx = s.lastIndexOf(' ');
  return idx < 0 ? [s, ''] : [s.slice(0, idx), s.slice(idx + 1)];
}

export function count(n) {
  return (n ?? 0).toLocaleString(lang());
}

export function percent(part, whole) {
  if (!whole) return '0%';
  const p = (part / whole) * 100;
  if (p > 0 && p < 0.1) return '<0.1%';
  return `${p.toLocaleString(lang(), { maximumFractionDigits: p < 10 ? 1 : 0 })}%`;
}

export function duration(ms) {
  if (ms < 1000) return `${Math.round(ms)} ms`;
  const s = ms / 1000;
  if (s < 60) return `${s.toLocaleString(lang(), { maximumFractionDigits: 1 })} s`;
  const m = Math.floor(s / 60);
  const rest = Math.round(s % 60);
  return `${m}:${String(rest).padStart(2, '0')} min`;
}

export function truncateMiddle(text, max = 64) {
  if (!text || text.length <= max) return text || '';
  const keep = max - 1;
  const head = Math.ceil(keep * 0.4);
  return `${text.slice(0, head)}…${text.slice(text.length - (keep - head))}`;
}

export function escapeHtml(s) {
  return String(s ?? '').replace(/[&<>"']/g, (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c]);
}
