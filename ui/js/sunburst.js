// Interactive, animated sunburst chart rendered on a 2D canvas.
//
// Zoom transitions work by moving a "camera" over the normalized layout of
// two views at once: the view we come from and the view we go to. Because a
// zoomed-into arc maps exactly onto the next view's root, the animation is
// seamless: rings slide inwards/outwards and the clicked segment grows into
// the center disc.

import { nodeColor, mix, lighten, css } from './colors.js';

const TAU = Math.PI * 2;
const START = -Math.PI / 2;
const EPS = 1e-6;

const easeInOut = (t) => (t < 0.5 ? 4 * t * t * t : 1 - Math.pow(-2 * t + 2, 3) / 2);
const easeOut = (t) => 1 - Math.pow(1 - t, 3);
const clamp01 = (v) => (v < 0 ? 0 : v > 1 ? 1 : v);

function layout(root) {
  const arcs = [];
  const byDepth = [];
  const visit = (node, x0, x1, depth, parent) => {
    const arc = { node, x0, x1, depth, parent };
    arcs.push(arc);
    (byDepth[depth] ||= []).push(arc);
    const kids = node.children;
    if (kids && kids.length) {
      const total = node.size || 1;
      const span = x1 - x0;
      let x = x0;
      for (const c of kids) {
        const w = (span * c.size) / total;
        if (w > EPS) visit(c, x, Math.min(x1, x + w), depth + 1, arc);
        x += w;
      }
    }
    return arc;
  };
  visit(root, 0, 1, 0, null);
  return { root, arcs, byDepth };
}

function isWithin(arc, ancestor) {
  for (let a = arc; a; a = a.parent) if (a === ancestor) return true;
  return false;
}

export class Sunburst {
  constructor(canvas, opts = {}) {
    this.canvas = canvas;
    this.ctx = canvas.getContext('2d');
    this.rings = opts.rings ?? 5;
    this.animate = opts.animate ?? true;
    this.cb = opts;
    this.mode = 'folder';
    this.view = null;
    this.transition = null;
    this.hover = null;
    this.highlightId = null;
    this.labelAlpha = 1;
    this.dpr = 1;
    this.w = 0;
    this.h = 0;
    this.theme = { center: [18, 20, 42], gap: 'rgba(0,0,0,0)', label: '#fff', ring: 'rgba(255,255,255,0.06)' };
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
      if (hit && hit.arc) this.cb.onDblClick?.(hit.arc.node, e);
    });
    canvas.addEventListener('contextmenu', (e) => {
      const hit = this.hitTest(e);
      if (hit && (hit.arc || hit.center)) {
        e.preventDefault();
        this.cb.onContext?.(hit.arc ? hit.arc.node : this.view.root, e);
      }
    });
  }

  setTheme(theme) {
    this.theme = { ...this.theme, ...theme };
    this.render();
  }

  /** Turns the zoom and appear animations on or off. */
  setAnimate(on) {
    this.animate = !!on;
  }

  setMode(mode) {
    this.mode = mode;
    this.render();
  }

  resize() {
    const parent = this.canvas.parentElement;
    const rect = parent.getBoundingClientRect();
    this.dpr = window.devicePixelRatio || 1;
    this.w = Math.max(10, rect.width);
    this.h = Math.max(10, rect.height);
    this.canvas.width = Math.round(this.w * this.dpr);
    this.canvas.height = Math.round(this.h * this.dpr);
    this.canvas.style.width = `${this.w}px`;
    this.canvas.style.height = `${this.h}px`;
    this.computeGeometry();
    this.render();
  }

  computeGeometry() {
    const R = Math.max(40, Math.min(this.w, this.h) / 2 - 14);
    const r0 = R * 0.27;
    const weights = [];
    let sum = 0;
    for (let i = 0; i < this.rings; i++) {
      const w = Math.pow(0.86, i);
      weights.push(w);
      sum += w;
    }
    const edges = [r0];
    for (let i = 0; i < this.rings; i++) edges.push(edges[i] + ((R - r0) * weights[i]) / sum);
    this.geom = { cx: this.w / 2, cy: this.h / 2, R, r0, edges, last: edges[this.rings] - edges[this.rings - 1] };
    this.cb.onGeometry?.(this.geom);
  }

  radiusAt(k) {
    const { r0, edges, last } = this.geom;
    if (k <= -1) return 0;
    if (k <= 0) return r0 * (1 + k);
    if (k >= this.rings) return edges[this.rings] + (k - this.rings) * last;
    const i = Math.floor(k);
    return edges[i] + (edges[i + 1] - edges[i]) * (k - i);
  }

  /** Replace the data. mode: 'sweep' | 'pop' | 'in' | 'out'. */
  setData(root, mode = 'pop', anchorId = null) {
    const next = layout(root);
    const prev = this.view;
    const now = performance.now();
    // Without animations every transition finishes on the next frame.
    const dur = (ms) => (this.animate ? ms : 1);
    this.transition = null;
    if (prev && mode === 'in') {
      const a = prev.arcs.find((x) => x.depth > 0 && x.node.id === anchorId);
      if (a) {
        const skip = new Set(prev.arcs.filter((x) => isWithin(x, a)));
        this.transition = { type: 'in', prev, next, a, skip, t0: now, dur: dur(620) };
      }
    } else if (prev && mode === 'out') {
      const b = next.arcs.find((x) => x.depth > 0 && x.node.id === anchorId);
      if (b) {
        const skip = new Set(next.arcs.filter((x) => isWithin(x, b)));
        this.transition = { type: 'out', prev, next, b, skip, t0: now, dur: dur(620) };
      }
    }
    if (!this.transition) {
      this.transition = { type: mode === 'sweep' ? 'sweep' : 'pop', next, t0: now, dur: dur(mode === 'sweep' ? 1100 : 420) };
    }
    this.view = next;
    this.hover = null;
    this.labelAlpha = this.animate ? 0 : 1;
    this.render();
  }

  setHighlight(id) {
    if (this.highlightId === id) return;
    this.highlightId = id;
    this.render();
  }

  /** Arc color for a node currently in view, used by the side list. */
  colorOf(id) {
    if (!this.view) return null;
    const arc = this.view.arcs.find((a) => a.node.id === id && a.depth > 0);
    if (!arc) return null;
    return css(nodeColor(arc.node, (arc.x0 + arc.x1) / 2, arc.depth, this.mode));
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
    if (!this.view) return;
    const { cx, cy, R } = this.geom;

    // Soft glow behind the chart.
    const glow = ctx.createRadialGradient(cx, cy, R * 0.2, cx, cy, R * 1.15);
    glow.addColorStop(0, 'rgba(124,92,255,0.10)');
    glow.addColorStop(1, 'rgba(124,92,255,0)');
    ctx.fillStyle = glow;
    ctx.fillRect(0, 0, this.w, this.h);

    // Faint guide rings.
    ctx.strokeStyle = this.theme.ring;
    ctx.lineWidth = 1;
    for (let i = 1; i <= this.rings; i++) {
      ctx.beginPath();
      ctx.arc(cx, cy, this.geom.edges[i] - 0.5, 0, TAU);
      ctx.stroke();
    }

    const tr = this.transition;
    let animating = false;
    const now = performance.now();
    const id = { c0: 0, c1: 1, cd: 0 };

    if (tr) {
      const raw = clamp01((now - tr.t0) / tr.dur);
      animating = raw < 1;
      if (tr.type === 'in' || tr.type === 'out') {
        const e = easeInOut(raw);
        if (tr.type === 'in') {
          const { a } = tr;
          const w = a.x1 - a.x0;
          const start = { c0: -a.x0 / w, c1: (1 - a.x0) / w, cd: -a.depth };
          const camNew = lerpCam(start, id, e);
          const camOld = { c0: a.x0 + camNew.c0 * w, c1: a.x0 + camNew.c1 * w, cd: camNew.cd + a.depth };
          this.drawLayer(tr.prev, camOld, 1, tr.skip);
          this.drawLayer(tr.next, camNew, 1, null);
        } else {
          const { b } = tr;
          const w = b.x1 - b.x0;
          const start = { c0: b.x0, c1: b.x1, cd: b.depth };
          const camNew = lerpCam(start, id, e);
          const camOld = { c0: (camNew.c0 - b.x0) / w, c1: (camNew.c1 - b.x0) / w, cd: camNew.cd - b.depth };
          this.drawLayer(tr.next, camNew, 1, tr.skip);
          this.drawLayer(tr.prev, camOld, 1, null);
        }
      } else if (tr.type === 'sweep') {
        const e = easeOut(raw);
        ctx.save();
        ctx.beginPath();
        ctx.moveTo(cx, cy);
        ctx.arc(cx, cy, R * 2, START, START + TAU * Math.max(0.0001, e));
        ctx.closePath();
        ctx.clip();
        this.drawLayer(this.view, id, 1, null, R * (0.85 + 0.15 * e) / R);
        ctx.restore();
        this.drawCenter(1);
      } else {
        const e = easeOut(raw);
        ctx.save();
        ctx.globalAlpha = e;
        ctx.translate(cx, cy);
        const s = 0.94 + 0.06 * e;
        ctx.scale(s, s);
        ctx.translate(-cx, -cy);
        this.drawLayer(this.view, id, 1, null);
        ctx.restore();
      }
      if (!animating) this.transition = null;
    } else {
      this.drawLayer(this.view, id, 1, null);
    }

    if (!animating) {
      this.drawHighlights();
      this.labelAlpha = Math.min(1, this.labelAlpha + 0.12);
      this.drawLabels(this.labelAlpha);
      if (this.labelAlpha < 1) animating = true;
    }
    if (animating) this.render();
  }

  drawLayer(view, cam, alpha, skip, scale = 1) {
    const { ctx } = this;
    const { cx, cy } = this.geom;
    const span = cam.c1 - cam.c0;
    const centerColor = this.theme.center;
    const rings = this.rings;
    const sheen = new Map();
    for (const arc of view.arcs) {
      if (skip && skip.has(arc)) continue;
      const sd = arc.depth - cam.cd;
      if (sd > rings + 1 || sd < -0.999) continue;
      const sx0 = clamp01((arc.x0 - cam.c0) / span);
      const sx1 = clamp01((arc.x1 - cam.c0) / span);
      if (sx1 - sx0 < 0.0004) continue;
      let r0 = this.radiusAt(sd - 1) * scale;
      let r1 = this.radiusAt(sd) * scale;
      if (r1 - r0 < 0.4) continue;
      let a = alpha;
      if (sd > rings) a *= clamp01(1 - (sd - rings));
      if (a <= 0.01) continue;

      let color;
      const mid = (sx0 + sx1) / 2;
      if (sd <= 0) color = centerColor;
      else {
        color = nodeColor(arc.node, mid, Math.max(1, sd), this.mode);
        if (sd < 1) color = mix(centerColor, color, sd);
      }

      const a0 = START + sx0 * TAU;
      const a1 = START + sx1 * TAU;
      const full = sx1 - sx0 > 0.9999;
      ctx.beginPath();
      if (full) {
        ctx.arc(cx, cy, Math.max(0, r1 - (sd > 0 ? 0.75 : 0)), 0, TAU);
        if (r0 > 0.5) ctx.arc(cx, cy, r0 + 0.75, TAU, 0, true);
      } else {
        // Constant pixel gap between neighbours (skipped for slivers so
        // dense areas read as a calm surface instead of a barcode).
        const gap = (a1 - a0) * r1 > 5 ? 0.85 : 0;
        const pr1 = Math.min((a1 - a0) / 3, gap / Math.max(r1, 1));
        const pr0 = r0 > 1 ? Math.min((a1 - a0) / 3, gap / r0) : 0;
        const ro = r1 - 0.75;
        const ri = r0 > 0.5 ? r0 + 0.75 : 0;
        ctx.arc(cx, cy, Math.max(ri, ro), a0 + pr1, a1 - pr1);
        if (ri > 0) ctx.arc(cx, cy, ri, a1 - pr0, a0 + pr0, true);
        else ctx.lineTo(cx, cy);
        ctx.closePath();
      }
      ctx.globalAlpha = a;
      if (arc.node.kind === 'hidden' && sd > 0) {
        ctx.fillStyle = this.hiddenPattern(color);
      } else {
        ctx.fillStyle = css(color);
      }
      ctx.fill('evenodd');
      if (sd > 0.5 && arc.node.kind !== 'group' && (sx1 - sx0) * r1 * TAU > 5) {
        // Subtle radial sheen: lighter towards the inner edge.
        const key = `${r0.toFixed(1)}|${r1.toFixed(1)}`;
        let g = sheen.get(key);
        if (!g) {
          g = ctx.createRadialGradient(cx, cy, Math.max(0, r0), cx, cy, r1);
          g.addColorStop(0, 'rgba(255,255,255,0.10)');
          g.addColorStop(1, 'rgba(0,0,0,0.12)');
          sheen.set(key, g);
        }
        ctx.fillStyle = g;
        ctx.fill('evenodd');
      }
    }
    ctx.globalAlpha = 1;
  }

  hiddenPattern(color) {
    if (!this._pattern || this._patternColor !== color.join()) {
      const c = document.createElement('canvas');
      c.width = c.height = 10;
      const p = c.getContext('2d');
      p.fillStyle = css(color);
      p.fillRect(0, 0, 10, 10);
      p.strokeStyle = 'rgba(255,255,255,0.10)';
      p.lineWidth = 3;
      p.beginPath();
      p.moveTo(-2, 12);
      p.lineTo(12, -2);
      p.stroke();
      this._pattern = this.ctx.createPattern(c, 'repeat');
      this._patternColor = color.join();
    }
    return this._pattern;
  }

  drawCenter() {
    const { ctx } = this;
    const { cx, cy, r0 } = this.geom;
    ctx.beginPath();
    ctx.arc(cx, cy, r0, 0, TAU);
    ctx.fillStyle = css(this.theme.center);
    ctx.fill();
  }

  arcPath(arc) {
    const { ctx } = this;
    const { cx, cy } = this.geom;
    const r0 = this.radiusAt(arc.depth - 1);
    const r1 = this.radiusAt(arc.depth);
    const a0 = START + arc.x0 * TAU;
    const a1 = START + arc.x1 * TAU;
    ctx.beginPath();
    if (arc.x1 - arc.x0 > 0.9999) {
      ctx.arc(cx, cy, r1 - 0.75, 0, TAU);
      ctx.arc(cx, cy, r0 + 0.75, TAU, 0, true);
    } else {
      ctx.arc(cx, cy, r1 - 0.75, a0, a1);
      ctx.arc(cx, cy, r0 + 0.75, a1, a0, true);
      ctx.closePath();
    }
  }

  drawHighlights() {
    const { ctx } = this;
    const target = this.hover?.arc || (this.highlightId != null ? this.view.arcs.find((a) => a.depth > 0 && a.node.id === this.highlightId) : null);
    if (!target) return;
    // Brighten the hovered subtree.
    ctx.save();
    for (const arc of this.view.arcs) {
      if (arc.depth === 0 || arc.depth > this.rings || !isWithin(arc, target)) continue;
      this.arcPath(arc);
      ctx.fillStyle = arc === target ? 'rgba(255,255,255,0.28)' : 'rgba(255,255,255,0.14)';
      ctx.fill('evenodd');
    }
    this.arcPath(target);
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
    const { cx, cy } = this.geom;
    ctx.save();
    ctx.globalAlpha = alpha;
    ctx.font = '600 11.5px -apple-system, BlinkMacSystemFont, "Segoe UI Variable Text", "Segoe UI", Inter, Cantarell, "Noto Sans", sans-serif';
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.fillStyle = '#ffffff';
    ctx.shadowColor = 'rgba(0,0,0,0.55)';
    ctx.shadowBlur = 4;
    const depths = [1, 2];
    for (const d of depths) {
      const arcs = this.view.byDepth[d] || [];
      const r0 = this.radiusAt(d - 1);
      const r1 = this.radiusAt(d);
      const thickness = r1 - r0;
      if (thickness < 17) continue;
      const rm = (r0 + r1) / 2;
      for (const arc of arcs) {
        const node = arc.node;
        if (node.kind === 'group') continue;
        const span = (arc.x1 - arc.x0) * TAU;
        const chord = 2 * r0 * Math.sin(Math.min(span, Math.PI) / 2);
        const room = Math.min(span * rm, chord) - 12;
        if (room < 42) continue;
        let text = node.kind === 'hidden' ? this.cb.hiddenLabel || 'Hidden' : node.name;
        text = fitText(ctx, text, room);
        if (!text) continue;
        const mid = START + ((arc.x0 + arc.x1) / 2) * TAU;
        const x = cx + Math.cos(mid) * rm;
        const y = cy + Math.sin(mid) * rm;
        ctx.save();
        ctx.translate(x, y);
        let rot = mid + Math.PI / 2;
        const norm = ((mid % TAU) + TAU) % TAU;
        if (norm > 0 && norm < Math.PI) rot += Math.PI;
        ctx.rotate(rot);
        ctx.fillText(text, 0, 0);
        ctx.restore();
      }
    }
    ctx.restore();
  }

  // ------------------------------------------------------------ interaction

  hitTest(e) {
    if (!this.view || this.transition) return null;
    const rect = this.canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const y = e.clientY - rect.top;
    const { cx, cy, r0, edges } = this.geom;
    const dx = x - cx;
    const dy = y - cy;
    const r = Math.hypot(dx, dy);
    if (r <= r0) return { center: true };
    let depth = -1;
    for (let i = 1; i <= this.rings; i++) {
      if (r <= edges[i]) {
        depth = i;
        break;
      }
    }
    if (depth < 0) return null;
    let ang = Math.atan2(dy, dx) - START;
    ang = ((ang % TAU) + TAU) % TAU;
    const pos = ang / TAU;
    const arcs = this.view.byDepth[depth];
    if (!arcs) return null;
    let lo = 0;
    let hi = arcs.length - 1;
    while (lo <= hi) {
      const m = (lo + hi) >> 1;
      const a = arcs[m];
      if (pos < a.x0) hi = m - 1;
      else if (pos >= a.x1) lo = m + 1;
      else return { arc: a };
    }
    return null;
  }

  setHover(hit, e) {
    const arc = hit?.arc || null;
    const center = !!hit?.center;
    const changed = arc !== this.hover?.arc || center !== !!this.hover?.center;
    this.hover = hit ? { arc, center } : null;
    this.canvas.style.cursor = arc ? (arc.node.kind === 'dir' && arc.node.has_children ? 'zoom-in' : 'pointer') : center && this.cb.canGoUp?.() ? 'zoom-out' : 'default';
    if (changed) this.render();
    this.cb.onHover?.(arc ? arc.node : null, e, { center, arc });
  }

  onPointerMove(e) {
    if (this._down) {
      const dx = e.clientX - this._down.x;
      const dy = e.clientY - this._down.y;
      if (!this._down.dragging && Math.hypot(dx, dy) > 6 && this._down.arc && this._down.arc.node.id >= 0) {
        this._down.dragging = true;
        this.cb.onDragStart?.(this._down.arc.node, e);
      }
      if (this._down.dragging) return;
    }
    this.setHover(this.hitTest(e), e);
  }

  onPointerDown(e) {
    if (e.button !== 0) return;
    const hit = this.hitTest(e);
    this._down = { x: e.clientX, y: e.clientY, arc: hit?.arc || null, center: !!hit?.center, dragging: false };
  }

  onPointerUp(e) {
    const down = this._down;
    this._down = null;
    if (!down || down.dragging || e.button !== 0) return;
    const hit = this.hitTest(e);
    if (!hit) return;
    if (hit.center && down.center) this.cb.onCenterClick?.();
    else if (hit.arc && hit.arc === down.arc) this.cb.onClick?.(hit.arc.node, e);
  }

  cancelDrag() {
    this._down = null;
  }
}

function lerpCam(a, b, t) {
  return { c0: a.c0 + (b.c0 - a.c0) * t, c1: a.c1 + (b.c1 - a.c1) * t, cd: a.cd + (b.cd - a.cd) * t };
}

function fitText(ctx, text, max) {
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
  return best.length > 3 ? best : '';
}

export { lighten };
