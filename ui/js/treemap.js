// Squarified treemap, the second chart view. It has the same interface and
// callbacks as the sunburst, and the same colors: a node's color position is
// the middle of its share of the focused folder, exactly as in the sunburst.
//
// Zooming animates the clicked rectangle growing to fill the chart (and
// shrinking back when going up), so the user keeps their place.

import { nodeColor, mix, css, HIDDEN_COLOR } from './colors.js';

const easeInOut = (t) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2);
const easeOut = (t) => 1 - Math.pow(1 - t, 3);
const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);
const HEADER = 18;
const PAD = 2;
const MIN_SIDE = 2.5;

/** Lays out rectangles for a view tree inside `box`. */
function layout(root, box, maxDepth) {
  const rects = [];
  const visit = (node, rect, depth, x0, x1, parent) => {
    const r = { node, depth, x: rect.x, y: rect.y, w: rect.w, h: rect.h, mid: (x0 + x1) / 2, parent, header: false, open: false };
    rects.push(r);
    const kids = (node.children || []).filter((c) => c.size > 0);
    if (!kids.length || depth >= maxDepth) return;
    let inner = { x: rect.x, y: rect.y, w: rect.w, h: rect.h };
    if (depth > 0) {
      // Room for a name strip when the folder is big enough to be readable.
      r.header = rect.w > 64 && rect.h > HEADER + 26;
      const top = r.header ? HEADER : PAD;
      inner = { x: rect.x + PAD, y: rect.y + top, w: rect.w - PAD * 2, h: rect.h - top - PAD };
    }
    if (inner.w < MIN_SIDE * 2 || inner.h < MIN_SIDE * 2) return;
    const total = kids.reduce((s, c) => s + c.size, 0) || 1;
    let x = x0;
    const items = kids.map((c) => {
      const span = ((x1 - x0) * c.size) / total;
      const it = { node: c, value: (c.size / total) * inner.w * inner.h, x0: x, x1: x + span };
      x += span;
      return it;
    });
    squarify(items, inner);
    for (const it of items) {
      if (it.rect.w >= MIN_SIDE && it.rect.h >= MIN_SIDE) {
        r.open = true;
        visit(it.node, it.rect, depth + 1, it.x0, it.x1, r);
      }
    }
  };
  visit(root, box, 0, 0, 1, null);
  return rects;
}

/** Squarified layout (Bruls, Huizing, van Wijk). Items are sorted largest first. */
function squarify(items, box) {
  let rect = { ...box };
  let row = [];
  const worst = (r, side) => {
    const s = r.reduce((a, it) => a + it.value, 0);
    let max = 0;
    let min = Infinity;
    for (const it of r) {
      max = Math.max(max, it.value);
      min = Math.min(min, it.value);
    }
    return Math.max((side * side * max) / (s * s), (s * s) / (side * side * min));
  };
  const place = (r) => {
    const s = r.reduce((a, it) => a + it.value, 0);
    if (rect.w >= rect.h) {
      const w = rect.h > 0 ? s / rect.h : 0;
      let y = rect.y;
      for (const it of r) {
        const h = w > 0 ? it.value / w : 0;
        it.rect = { x: rect.x, y, w, h };
        y += h;
      }
      rect = { x: rect.x + w, y: rect.y, w: rect.w - w, h: rect.h };
    } else {
      const h = rect.w > 0 ? s / rect.w : 0;
      let x = rect.x;
      for (const it of r) {
        const w = h > 0 ? it.value / h : 0;
        it.rect = { x, y: rect.y, w, h };
        x += w;
      }
      rect = { x: rect.x, y: rect.y + h, w: rect.w, h: rect.h - h };
    }
  };
  for (const it of items) {
    const side = Math.min(rect.w, rect.h);
    if (!row.length || worst([...row, it], side) <= worst(row, side)) row.push(it);
    else {
      place(row);
      row = [it];
    }
  }
  if (row.length) place(row);
}

export class Treemap {
  constructor(canvas, opts = {}) {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d');
    this.rings = opts.rings ?? 5;
    this.animate = opts.animate ?? true;
    this.cb = opts;
    this.mode = 'folder';
    this.root = null;
    this.rects = [];
    this.transition = null;
    this.hover = null;
    this.highlightId = null;
    this.labelAlpha = 1;
    this.theme = { center: [18, 20, 42], ring: 'rgba(255,255,255,0.06)' };
    this._raf = 0;
    this._down = null;

    this._resize = new ResizeObserver(() => this.resize());
    this._resize.observe(canvas.parentElement);
    this.resize();

    canvas.addEventListener('pointermove', (e) => this.onPointerMove(e));
    canvas.addEventListener('pointerleave', () => this.setHover(null, null));
    canvas.addEventListener('pointerdown', (e) => this.onPointerDown(e));
    canvas.addEventListener('pointerup', (e) => this.onPointerUp(e));
    canvas.addEventListener('dblclick', (e) => {
      const hit = this.hitTest(e);
      if (hit) this.cb.onDblClick?.(hit.node, e);
    });
    canvas.addEventListener('contextmenu', (e) => {
      const hit = this.hitTest(e);
      e.preventDefault();
      this.cb.onContext?.(hit ? hit.node : this.root, e);
    });
  }

  destroy() {
    this._resize.disconnect();
  }

  setTheme(theme) {
    this.theme = { ...this.theme, ...theme };
    this.render();
  }

  setAnimate(on) {
    this.animate = !!on;
  }

  setMode(mode) {
    this.mode = mode;
    this.render();
  }

  get box() {
    const m = 10;
    return { x: m, y: m, w: Math.max(10, this.w - m * 2), h: Math.max(10, this.h - m * 2) };
  }

  resize() {
    const rect = this.canvas.parentElement.getBoundingClientRect();
    this.dpr = window.devicePixelRatio || 1;
    this.w = Math.max(10, rect.width);
    this.h = Math.max(10, rect.height);
    this.canvas.width = Math.round(this.w * this.dpr);
    this.canvas.height = Math.round(this.h * this.dpr);
    this.canvas.style.width = `${this.w}px`;
    this.canvas.style.height = `${this.h}px`;
    if (this.root) this.rects = layout(this.root, this.box, this.rings);
    this.render();
  }

  /** Replace the data. mode: 'sweep' | 'pop' | 'in' | 'out'. */
  setData(root, mode = 'pop', anchorId = null) {
    const prev = this.rects;
    const next = layout(root, this.box, this.rings);
    const now = performance.now();
    const dur = (ms) => (this.animate ? ms : 1);
    this.transition = null;
    if (mode === 'in' && prev.length) {
      const a = prev.find((r) => r.depth > 0 && r.node.id === anchorId);
      if (a) this.transition = { type: 'in', prev, from: a, t0: now, dur: dur(560) };
    } else if (mode === 'out' && prev.length) {
      const b = next.find((r) => r.depth > 0 && r.node.id === anchorId);
      if (b) this.transition = { type: 'out', prev, to: b, t0: now, dur: dur(560) };
    }
    if (!this.transition && mode !== 'none') this.transition = { type: 'pop', t0: now, dur: dur(mode === 'sweep' ? 700 : 380) };
    this.root = root;
    this.rects = next;
    this.hover = null;
    this.labelAlpha = this.animate && mode !== 'none' ? 0 : 1;
    this.render();
  }

  setHighlight(id) {
    if (this.highlightId === id) return;
    this.highlightId = id;
    this.render();
  }

  colorOf(id) {
    const r = this.rects.find((x) => x.depth > 0 && x.node.id === id);
    return r ? css(this.colorFor(r)) : null;
  }

  colorFor(r) {
    if (r.node.kind === 'hidden') return HIDDEN_COLOR;
    return nodeColor(r.node, r.mid, Math.max(1, r.depth), this.mode);
  }

  render() {
    if (this._raf) return;
    this._raf = requestAnimationFrame(() => {
      this._raf = 0;
      this.draw();
    });
  }

  // ---------------------------------------------------------------- drawing

  draw() {
    const { ctx, dpr } = this;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, this.w, this.h);
    if (!this.root) return;
    const full = this.box;
    const tr = this.transition;
    let animating = false;
    if (tr) {
      const raw = clamp01((performance.now() - tr.t0) / tr.dur);
      animating = raw < 1;
      if (tr.type === 'in') {
        const e = easeInOut(raw);
        const cur = lerpRect(tr.from, full, e);
        this.drawRects(tr.prev, mapRect(tr.from, cur), 1 - e);
        this.drawRects(this.rects, mapRect(full, cur), Math.min(1, e * 1.6));
      } else if (tr.type === 'out') {
        const e = easeInOut(raw);
        const cur = lerpRect(full, tr.to, e);
        this.drawRects(this.rects, mapRect(tr.to, cur), Math.min(1, e * 1.6));
        this.drawRects(tr.prev, mapRect(full, cur), 1 - e);
      } else {
        const e = easeOut(raw);
        const s = 0.96 + 0.04 * e;
        const cx = full.x + full.w / 2;
        const cy = full.y + full.h / 2;
        this.drawRects(this.rects, { sx: s, sy: s, tx: cx - cx * s, ty: cy - cy * s }, e);
      }
      if (!animating) this.transition = null;
    } else {
      this.drawRects(this.rects, null, 1);
    }
    if (!animating) {
      this.drawHighlight();
      this.labelAlpha = Math.min(1, this.labelAlpha + 0.14);
      this.drawLabels(this.labelAlpha);
      if (this.labelAlpha < 1) animating = true;
    }
    if (animating) this.render();
  }

  drawRects(rects, m, alpha) {
    if (alpha <= 0.01) return;
    const { ctx } = this;
    ctx.save();
    if (m) ctx.transform(m.sx, 0, 0, m.sy, m.tx, m.ty);
    ctx.globalAlpha = alpha;
    const center = this.theme.center;
    for (const r of rects) {
      if (r.depth === 0) continue;
      const color = this.colorFor(r);
      const isParent = r.open;
      const x = r.x + 0.5;
      const y = r.y + 0.5;
      const w = Math.max(0, r.w - 1);
      const h = Math.max(0, r.h - 1);
      if (w < 0.5 || h < 0.5) continue;
      if (isParent) {
        ctx.fillStyle = css(mix(center, color, 0.42));
        roundRect(ctx, x, y, w, h, Math.min(5, w / 4, h / 4));
        ctx.fill();
        if (r.header) {
          ctx.fillStyle = css(mix(center, color, 0.75));
          roundRect(ctx, x, y, w, HEADER - 1, Math.min(5, w / 4), true);
          ctx.fill();
        }
      } else {
        ctx.fillStyle = css(color);
        roundRect(ctx, x, y, w, h, Math.min(4, w / 4, h / 4));
        ctx.fill();
        if (w > 6 && h > 6) {
          const g = ctx.createLinearGradient(0, y, 0, y + h);
          g.addColorStop(0, 'rgba(255,255,255,0.12)');
          g.addColorStop(1, 'rgba(0,0,0,0.10)');
          ctx.fillStyle = g;
          ctx.fill();
        }
      }
    }
    ctx.restore();
  }

  target() {
    if (this.hover) return this.hover;
    if (this.highlightId == null) return null;
    return this.rects.find((r) => r.depth > 0 && r.node.id === this.highlightId) || null;
  }

  drawHighlight() {
    const r = this.target();
    if (!r) return;
    const { ctx } = this;
    ctx.save();
    roundRect(ctx, r.x + 0.5, r.y + 0.5, Math.max(0, r.w - 1), Math.max(0, r.h - 1), 4);
    ctx.fillStyle = 'rgba(255,255,255,0.16)';
    ctx.fill();
    ctx.lineWidth = 2;
    ctx.strokeStyle = 'rgba(255,255,255,0.95)';
    ctx.shadowColor = 'rgba(0,0,0,0.35)';
    ctx.shadowBlur = 8;
    ctx.stroke();
    ctx.restore();
  }

  drawLabels(alpha) {
    if (alpha <= 0) return;
    const { ctx } = this;
    const fmt = this.cb.formatSize || ((n) => String(n));
    ctx.save();
    ctx.globalAlpha = alpha;
    ctx.textBaseline = 'top';
    // White text with a shadow on dark fills, dark text on light ones.
    const ink = (fill) => {
      const light = (0.2126 * fill[0] + 0.7152 * fill[1] + 0.0722 * fill[2]) / 255 > 0.66;
      ctx.fillStyle = light ? '#1b1e36' : '#ffffff';
      ctx.shadowColor = light ? 'rgba(0,0,0,0)' : 'rgba(0,0,0,0.5)';
      ctx.shadowBlur = light ? 0 : 3;
    };
    const center = this.theme.center;
    for (const r of this.rects) {
      if (r.depth === 0 || r.node.kind === 'group') continue;
      const name = r.node.kind === 'hidden' ? this.cb.hiddenLabel || 'Hidden' : r.node.name;
      const color = this.colorFor(r);
      ink(r.header ? mix(center, color, 0.75) : color);
      if (r.header) {
        ctx.font = '600 11px -apple-system, BlinkMacSystemFont, "Segoe UI Variable Text", "Segoe UI", Inter, Cantarell, "Noto Sans", sans-serif';
        const size = fmt(r.node.size);
        const sw = ctx.measureText(size).width;
        const text = fitText(ctx, name, r.w - sw - 22);
        if (text) ctx.fillText(text, r.x + 7, r.y + 4);
        ctx.globalAlpha = alpha * 0.75;
        if (r.w > sw + 40) ctx.fillText(size, r.x + r.w - sw - 7, r.y + 4);
        ctx.globalAlpha = alpha;
        continue;
      }
      if (r.open || r.w < 46 || r.h < 22) continue;
      ctx.font = '600 11.5px -apple-system, BlinkMacSystemFont, "Segoe UI Variable Text", "Segoe UI", Inter, Cantarell, "Noto Sans", sans-serif';
      const text = fitText(ctx, name, r.w - 12);
      if (!text) continue;
      ctx.fillText(text, r.x + 6, r.y + 5);
      if (r.h > 38) {
        ctx.font = '500 11px -apple-system, BlinkMacSystemFont, "Segoe UI Variable Text", "Segoe UI", Inter, Cantarell, "Noto Sans", sans-serif';
        ctx.globalAlpha = alpha * 0.78;
        ctx.fillText(fitText(ctx, fmt(r.node.size), r.w - 12), r.x + 6, r.y + 20);
        ctx.globalAlpha = alpha;
      }
    }
    ctx.restore();
  }

  // ------------------------------------------------------------ interaction

  hitTest(e) {
    if (!this.root || this.transition) return null;
    const rect = this.canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    for (let i = this.rects.length - 1; i >= 0; i--) {
      const r = this.rects[i];
      if (r.depth > 0 && x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h) return r;
    }
    return null;
  }

  setHover(hit, e) {
    const changed = hit !== this.hover;
    this.hover = hit;
    this.canvas.style.cursor = hit ? (hit.node.kind === 'dir' && hit.node.has_children ? 'zoom-in' : 'pointer') : 'default';
    if (changed) this.render();
    this.cb.onHover?.(hit ? hit.node : null, e, { center: false, arc: hit });
  }

  onPointerMove(e) {
    if (this._down) {
      const dx = e.clientX - this._down.x;
      const dy = e.clientY - this._down.y;
      if (!this._down.dragging && Math.hypot(dx, dy) > 6 && this._down.hit && this._down.hit.node.id >= 0) {
        this._down.dragging = true;
        this.cb.onDragStart?.(this._down.hit.node, e);
      }
      if (this._down.dragging) return;
    }
    this.setHover(this.hitTest(e), e);
  }

  onPointerDown(e) {
    if (e.button !== 0) return;
    this._down = { x: e.clientX, y: e.clientY, hit: this.hitTest(e), dragging: false };
  }

  onPointerUp(e) {
    const down = this._down;
    this._down = null;
    if (!down || down.dragging || e.button !== 0) return;
    const hit = this.hitTest(e);
    if (hit && hit === down.hit) this.cb.onClick?.(hit.node, e);
  }

  cancelDrag() {
    this._down = null;
  }
}

function lerpRect(a, b, t) {
  return { x: a.x + (b.x - a.x) * t, y: a.y + (b.y - a.y) * t, w: a.w + (b.w - a.w) * t, h: a.h + (b.h - a.h) * t };
}

/** Transform that maps rectangle `from` onto rectangle `to`. */
function mapRect(from, to) {
  const sx = to.w / Math.max(from.w, 1e-6);
  const sy = to.h / Math.max(from.h, 1e-6);
  return { sx, sy, tx: to.x - from.x * sx, ty: to.y - from.y * sy };
}

function roundRect(ctx, x, y, w, h, r, topOnly = false) {
  r = Math.max(0, Math.min(r, w / 2, h / 2));
  ctx.beginPath();
  ctx.moveTo(x + r, y);
  ctx.lineTo(x + w - r, y);
  ctx.quadraticCurveTo(x + w, y, x + w, y + r);
  if (topOnly) {
    ctx.lineTo(x + w, y + h);
    ctx.lineTo(x, y + h);
  } else {
    ctx.lineTo(x + w, y + h - r);
    ctx.quadraticCurveTo(x + w, y + h, x + w - r, y + h);
    ctx.lineTo(x + r, y + h);
    ctx.quadraticCurveTo(x, y + h, x, y + h - r);
  }
  ctx.lineTo(x, y + r);
  ctx.quadraticCurveTo(x, y, x + r, y);
  ctx.closePath();
}

function fitText(ctx, text, max) {
  if (max < 18) return '';
  if (ctx.measureText(text).width <= max) return text;
  let lo = 1;
  let hi = text.length;
  let best = '';
  while (lo <= hi) {
    const m = (lo + hi) >> 1;
    const s = `${text.slice(0, m)}…`;
    if (ctx.measureText(s).width <= max) {
      best = s;
      lo = m + 1;
    } else hi = m - 1;
  }
  return best.length > 2 ? best : '';
}
