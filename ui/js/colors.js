// Shared color logic for the sunburst, lists and legends.
// The palette matches the app icon (assets/make_icon.py).

const hex = (h) => [1, 3, 5].map((i) => parseInt(h.slice(i, i + 2), 16));

export const PALETTE = [
  '#7c5cff', '#b64dff', '#ff4fa3', '#ff6b6b', '#ff9f43',
  '#ffd43b', '#5ee6a8', '#3ddbd9', '#4dabf7', '#7c5cff',
].map(hex);

export const CATEGORIES = ['video', 'audio', 'image', 'document', 'archive', 'code', 'app', 'other'];

export const CATEGORY_COLORS = [
  '#ff5c8a', // video
  '#ffa94d', // audio
  '#3ddbd9', // images
  '#4dabf7', // documents
  '#b197fc', // archives
  '#69db7c', // code
  '#ff8787', // apps
  '#8f96b3', // other
].map(hex);

export const GROUP_COLOR = [118, 124, 150];
export const HIDDEN_COLOR = [70, 74, 100];

export function mix(a, b, t) {
  return [a[0] + (b[0] - a[0]) * t, a[1] + (b[1] - a[1]) * t, a[2] + (b[2] - a[2]) * t];
}

export function lighten(c, t) {
  return mix(c, [255, 255, 255], t);
}

export function paletteAt(t) {
  t = ((t % 1) + 1) % 1;
  const x = t * (PALETTE.length - 1);
  const i = Math.floor(x);
  return mix(PALETTE[i], PALETTE[Math.min(i + 1, PALETTE.length - 1)], x - i);
}

export function css(c, alpha = 1) {
  const r = Math.round(c[0]);
  const g = Math.round(c[1]);
  const b = Math.round(c[2]);
  return alpha >= 1 ? `rgb(${r},${g},${b})` : `rgba(${r},${g},${b},${alpha})`;
}

/**
 * Color of a node.
 * @param node   view node ({kind, cat})
 * @param pos    angular mid position 0..1 (folder mode)
 * @param depth  ring depth (1 = innermost)
 * @param mode   'folder' | 'type'
 */
export function nodeColor(node, pos, depth, mode) {
  if (node.kind === 'group') return GROUP_COLOR;
  if (node.kind === 'hidden') return HIDDEN_COLOR;
  const d = Math.max(0, depth - 1);
  if (mode === 'type') {
    const base = CATEGORY_COLORS[node.cat ?? 7] || CATEGORY_COLORS[7];
    return node.kind === 'dir' ? lighten(mix(base, [40, 44, 70], 0.12), d * 0.07) : lighten(base, d * 0.05);
  }
  const base = paletteAt(pos);
  if (node.kind === 'dir') return lighten(base, Math.min(0.42, d * 0.085));
  // Files: a soft, desaturated version of the folder color.
  return lighten(mix(base, [150, 156, 182], 0.62), Math.min(0.3, d * 0.05));
}
