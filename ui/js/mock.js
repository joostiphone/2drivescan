// Mock backend with a deterministic, realistic looking disk. Mirrors the
// behaviour of the Rust commands closely enough for design work and demos.

const CAT = { video: 0, audio: 1, image: 2, document: 3, archive: 4, code: 5, app: 6, other: 7 };
const GB = 1e9;
const MB = 1e6;

function rng(seed) {
  let s = seed >>> 0;
  return () => {
    s = (s + 0x6d2b79f5) >>> 0;
    let t = s;
    t = Math.imul(t ^ (t >>> 15), t | 1);
    t ^= t + Math.imul(t ^ (t >>> 7), t | 61);
    return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
  };
}

const FILES = {
  video: [['mov', 'Screen Recording ', 'Interview ', 'Drone footage ', 'Family ', 'Clip '], ['mp4', 'Export ', 'Holiday ', 'Webinar ', 'Render ']],
  audio: [['m4a', 'Voice memo ', 'Podcast ep '], ['mp3', 'Track ', 'Mix '], ['wav', 'Stem ', 'Take ']],
  image: [['heic', 'IMG_'], ['jpg', 'DSC_', 'Photo '], ['png', 'Screenshot ', 'Mockup '], ['psd', 'Design ']],
  document: [['pdf', 'Invoice ', 'Report ', 'Manual ', 'Contract '], ['docx', 'Proposal ', 'Notes '], ['xlsx', 'Budget ', 'Forecast '], ['key', 'Keynote ']],
  archive: [['dmg', 'Installer ', 'Xcode_', 'Setup '], ['zip', 'Archive ', 'Backup ', 'Assets '], ['iso', 'ubuntu-24.04-desktop-amd64 ']],
  code: [['js', 'index', 'bundle'], ['rlib', 'libcore'], ['json', 'package'], ['o', 'module'], ['pack', 'pack-']],
  app: [['dylib', 'libswift'], ['so', 'libnode'], ['bin', 'model'], ['exe', 'setup']],
  other: [['', 'data'], ['cache', 'cache_'], ['db', 'store'], ['log', 'system']],
};

function makeTree(seed = 7) {
  const rand = rng(seed);
  const nodes = [];
  const DAY = 86_400_000;
  const now = Date.now();
  const add = (name, parent, kind, size = 0, cat = CAT.other) => {
    const n = { id: nodes.length, name, parent, kind, size, items: kind === 'dir' ? 0 : 1, cat, children: [], removed: false };
    if (kind !== 'dir') n.mtime = Math.round(now - Math.pow(rand(), 1.7) * 1100 * DAY);
    nodes.push(n);
    if (parent >= 0) nodes[parent].children.push(n.id);
    return n;
  };

  const fileName = (catName) => {
    const groups = FILES[catName];
    const g = groups[Math.floor(rand() * groups.length)];
    const prefix = g[1 + Math.floor(rand() * (g.length - 1))];
    const num = Math.floor(rand() * 9000 + 100);
    return g[0] ? `${prefix}${num}.${g[0]}` : `${prefix}${num}`;
  };

  // Fills a directory with random files of the given mix until `size` bytes.
  const fill = (dir, size, mix, maxFiles = 40) => {
    const entries = Object.entries(mix);
    const total = entries.reduce((s, [, w]) => s + w, 0);
    let left = size;
    let n = 0;
    while (left > size * 0.02 && n < maxFiles) {
      let r = rand() * total;
      let catName = entries[0][0];
      for (const [k, w] of entries) {
        if ((r -= w) <= 0) {
          catName = k;
          break;
        }
      }
      const s = Math.min(left, Math.max(4096, left * (0.06 + rand() * 0.38)));
      add(fileName(catName), dir.id, 'file', Math.round(s), CAT[catName]);
      left -= s;
      n++;
    }
  };

  // spec: [name, sizeGB, mix | children[]]
  const build = (spec, parent) => {
    const [name, sizeGB, content] = spec;
    const dir = add(name, parent, 'dir');
    if (Array.isArray(content)) {
      let used = 0;
      for (const child of content) {
        build(child, dir.id);
        used += child[1];
      }
      const rest = sizeGB - used;
      if (rest > 0.001) fill(dir, rest * GB, { other: 3, document: 1, image: 1 }, 30);
    } else if (content) {
      fill(dir, sizeGB * GB, content, 60);
    }
    return dir;
  };

  const many = (prefix, count, totalGB, mix) => {
    const out = [];
    let left = totalGB;
    for (let i = 0; i < count; i++) {
      const s = i === count - 1 ? left : left * (0.15 + rand() * 0.35);
      out.push([`${prefix}${i + 1}`, s, mix]);
      left -= s;
    }
    return out;
  };

  const projects = ['aurora-web', 'tauri-playground', 'ml-experiments', 'website', 'mobile-app', 'design-system', 'api-server'].map((p, i) => [
    p, [6.2, 4.8, 9.4, 1.6, 3.9, 2.2, 2.9][i], [
      ['node_modules', [2.1, 1.2, 0.3, 0.6, 1.8, 0.9, 0.4][i], many('pkg-', 14, [2.1, 1.2, 0.3, 0.6, 1.8, 0.9, 0.4][i], { code: 6, app: 1, other: 1 })],
      ['target', [2.8, 3.1, 0.2, 0.1, 0.4, 0.3, 1.9][i], { code: 5, app: 2 }],
      ['.git', [0.5, 0.2, 0.4, 0.3, 0.6, 0.4, 0.2][i], { code: 1, other: 1 }],
      ['src', 0.05, { code: 1 }],
      ...(i === 2 ? [['datasets', 7.9, { archive: 2, other: 3, image: 2 }], ['checkpoints', 0.5, { app: 1 }]] : []),
    ],
  ]);

  const disk = ['Macintosh HD', 0, [
    ['Users', 0, [
      ['joost', 0, [
        ['Movies', 118, [
          ['Final Cut Projects', 64, [['Brand film 2026.fcpbundle', 38, { video: 5, other: 1 }], ['Wedding edit.fcpbundle', 26, { video: 4, audio: 1 }]]],
          ['iMovie Library.imovielibrary', 21, { video: 4, image: 1 }],
          ['Exports', 33, { video: 1 }],
        ]],
        ['Pictures', 86, [
          ['Photos Library.photoslibrary', 74, [['originals', 61, many('', 16, 61, { image: 6, video: 1 })], ['resources', 13, { image: 3, other: 1 }]]],
          ['Lightroom', 12, { image: 1 }],
        ]],
        ['Library', 71, [
          ['Developer', 31, [['Xcode', 19, [['DerivedData', 12, { code: 3, app: 1 }], ['iOS DeviceSupport', 7, { app: 2, other: 1 }]]], ['CoreSimulator', 12, { app: 1, other: 1 }]]],
          ['Caches', 14, [['com.spotify.client', 4.1, { audio: 1, other: 2 }], ['Google', 3.4, { other: 1 }], ['Homebrew', 2.6, { archive: 2 }], ['com.apple.Safari', 1.2, { other: 1, image: 1 }]]],
          ['Application Support', 19, [['Steam', 9.5, { app: 2, other: 3 }], ['Slack', 2.4, { other: 1, image: 1 }], ['Code', 1.8, { code: 1 }], ['MobileSync', 5.1, [['Backup', 5.1, many('iPhone backup ', 3, 5.1, { other: 1 })]]]]],
          ['Containers', 5, { other: 2, document: 1 }],
          ['Mail', 2, { document: 2, other: 1 }],
        ]],
        ['Developer', 32, projects],
        ['Downloads', 36, { archive: 4, document: 2, video: 1, image: 1, app: 1 }],
        ['Documents', 21, [['Work', 9, { document: 4, image: 1 }], ['Scans', 3.5, { document: 1 }], ['Archive', 6.5, { archive: 1, document: 1 }]]],
        ['Music', 16, [['Music', 11, many('Album ', 12, 11, { audio: 1 })], ['GarageBand', 5, { audio: 2 }]]],
        ['Desktop', 5.5, { image: 3, document: 2, video: 1 }],
        ['.Trash', 3.2, { archive: 2, video: 1, document: 1 }],
      ]],
      ['Shared', 2.4, { other: 1, archive: 1 }],
    ]],
    ['Applications', 62, [
      ['Xcode.app', 13.8, { app: 3, code: 1 }], ['Final Cut Pro.app', 6.1, { app: 2 }], ['Logic Pro.app', 3.9, { app: 1, audio: 2 }],
      ['Microsoft Word.app', 2.6, { app: 1 }], ['Docker.app', 2.2, { app: 1 }], ['Figma.app', 0.6, { app: 1 }], ['Spotify.app', 0.4, { app: 1 }],
      ['Slack.app', 0.5, { app: 1 }], ['Google Chrome.app', 1.3, { app: 1 }], ['Steam.app', 0.3, { app: 1 }],
    ]],
    ['Library', 38, [['Application Support', 12, { app: 1, other: 2 }], ['Developer', 15, { app: 2, code: 1 }], ['Audio', 7, { audio: 3 }], ['Fonts', 1.2, { other: 1 }]]],
    ['System', 21, [['Library', 19, { app: 2, other: 2 }]]],
    ['private', 11, [['var', 10.5, [['vm', 6, { other: 1 }], ['folders', 3, { other: 1 }], ['log', 1.5, { other: 1 }]]]]],
    ['opt', 6.5, [['homebrew', 6.5, { app: 2, code: 2 }]]],
    ['usr', 2.8, { app: 2, code: 1 }],
  ]];

  // Fill in sizes for specs given as 0 (sum of children).
  const sumSpec = (spec) => {
    if (Array.isArray(spec[2])) {
      const s = spec[2].reduce((acc, c) => acc + sumSpec(c), 0);
      if (!spec[1]) spec[1] = s;
      else spec[1] = Math.max(spec[1], s);
    }
    return spec[1];
  };
  sumSpec(disk);
  build(disk, -1);

  // Copies of some bigger files elsewhere, so the duplicate finder has
  // something to find. Copies share a `dup` key with the original.
  const byPath = (path) => path.split('/').reduce((cur, part) => cur != null && nodes[cur].children.find((c) => nodes[c].name === part), 0);
  const spots = ['Users/joost/Downloads', 'Users/joost/Desktop', 'Users/joost/Documents/Archive', 'Users/joost/Movies/Exports', 'Users/joost/Pictures/Lightroom']
    .map(byPath)
    .filter((x) => x);
  const big = nodes.filter((n) => n.kind === 'file' && n.size > 40 * MB && n.cat !== CAT.other);
  for (let i = 0; i < 28 && big.length; i++) {
    const orig = big.splice(Math.floor(rand() * big.length), 1)[0];
    orig.dup = orig.id;
    const copies = 1 + (rand() < 0.3 ? 1 : 0);
    for (let k = 0; k < copies; k++) {
      const dir = spots[Math.floor(rand() * spots.length)];
      const dot = orig.name.lastIndexOf('.');
      const name = dot > 0 ? `${orig.name.slice(0, dot)}${k ? ` (${k})` : ' copy'}${orig.name.slice(dot)}` : `${orig.name} copy`;
      const c = add(name, dir, 'file', orig.size, orig.cat);
      c.dup = orig.id;
    }
  }
  // Downloads are older on average: good material for the age filter.
  const dl = byPath('Users/joost/Downloads');
  if (dl) for (const c of nodes[dl].children) if (nodes[c].mtime) nodes[c].mtime -= Math.round(rand() * 240 * DAY);

  // Aggregate sizes bottom-up and sort children.
  const finalize = (id) => {
    const n = nodes[id];
    if (n.kind !== 'dir') return;
    let size = 0;
    let items = 0;
    const cats = new Array(8).fill(0);
    for (const c of n.children) {
      finalize(c);
      const k = nodes[c];
      size += k.size;
      items += k.items;
      if (k.kind === 'dir') k.cats.forEach((v, i) => (cats[i] += v));
      else cats[k.cat] += k.size;
    }
    n.size = size + 4096;
    n.items = items;
    n.cats = cats;
    n.cat = cats.indexOf(Math.max(...cats));
    n.children.sort((a, b) => nodes[b].size - nodes[a].size);
  };
  finalize(0);

  // A previous scan from six days ago: most files unchanged, some grew,
  // shrank or are new; a few folders lost files that were deleted since.
  const BUSY = ['Downloads', 'Desktop', 'Movies', 'Developer', 'Caches', 'DerivedData', 'Documents'];
  const before = (id, busy = false) => {
    const n = nodes[id];
    if (n.kind !== 'dir') {
      const r = busy ? rand() : 0.5 + rand() * 0.5 + (rand() < 0.02 ? -0.45 : 0);
      if (n.dup != null && n.dup !== n.id) n.prev = null; // copies are new
      else if (r < 0.06) n.prev = null;
      else if (r < 0.3) n.prev = Math.round(n.size * (0.6 + rand() * 0.37));
      else if (r < 0.33) n.prev = Math.round(n.size * (1.1 + rand() * 0.5));
      else n.prev = n.size;
      return n.prev ?? 0;
    }
    let sum = 4096;
    for (const c of n.children) sum += before(c, busy || BUSY.includes(n.name));
    if (n.name === 'DerivedData') sum += Math.round(n.size * (0.1 + rand() * 0.1));
    n.prev = n.name === 'checkpoints' ? null : sum;
    return n.prev ?? 0;
  };
  before(0);
  return nodes;
}

export function createMock() {
  const listeners = new Map();
  const emit = (event, payload) => (listeners.get(event) || []).forEach((cb) => cb(payload));

  let nodes = null;
  let rootId = 0;
  let rootPath = '/';
  let timer = null;
  let scanId = 0;

  const kindOf = (n) => n.kind;
  const live = (id) => nodes && nodes[id] && !nodes[id].removed;

  let comparing = false;
  const PREVIOUS_MS = Date.now() - 6 * 86_400_000 - 3 * 3_600_000;
  const delta = (n) => (n.kind === 'hidden' ? null : n.size - (n.id === rootId ? hiddenBytes : 0) - (n.prev ?? 0));
  const annotate = (v, n) => {
    if (!comparing || n.kind === 'hidden') return v;
    v.delta = delta(n);
    if (n.prev == null) v.new = true;
    return v;
  };
  const leaf = (id) => {
    const n = nodes[id];
    return annotate(
      {
        id: n.id,
        name: id === rootId ? rootPath : n.name,
        size: n.size,
        items: n.items,
        kind: kindOf(n),
        cat: n.cat,
        has_children: n.children.length > 0,
      },
      n,
    );
  };

  const view = (id, depth, minSize) => {
    const v = leaf(id);
    if (depth === 0) return v;
    const kids = nodes[id].children;
    v.children = [];
    let gs = 0;
    let gi = 0;
    for (let i = 0; i < kids.length; i++) {
      const c = nodes[kids[i]];
      if (c.size < minSize) {
        for (let j = i; j < kids.length; j++) {
          gs += nodes[kids[j]].size;
          gi++;
        }
        break;
      }
      v.children.push(view(c.id, depth - 1, minSize));
    }
    if (gi) v.children.push({ id: -1, name: '', size: gs, items: gi, kind: 'group', cat: 7 });
    return v;
  };

  const ancestors = (id) => {
    const chain = [];
    for (let c = id; c != null && c >= 0; c = nodes[c].parent) {
      chain.push(c);
      if (c === rootId) break;
    }
    return chain.reverse();
  };
  const pathOf = (id) => {
    const parts = ancestors(id).slice(1).map((a) => nodes[a].name);
    const base = rootPath === '/' ? '' : rootPath;
    return parts.length ? `${base}/${parts.join('/')}` : rootPath;
  };
  const relPath = (base, id) => {
    const chain = ancestors(id);
    const i = chain.indexOf(base);
    return chain.slice(i + 1).map((a) => nodes[a].name).join('/');
  };
  const descendants = (id, fn) => {
    const stack = [...nodes[id].children];
    while (stack.length) {
      const c = stack.pop();
      const n = nodes[c];
      if (n.removed) continue;
      fn(n);
      stack.push(...n.children);
    }
  };
  const hit = (base, n) => ({ id: n.id, name: n.name, size: n.size, kind: n.kind, cat: n.cat, rel_path: relPath(base, n.id), modified_ms: n.mtime || 0 });
  const findPath = (path) => {
    const parts = path.split('/').filter(Boolean);
    const base = rootPath.split('/').filter(Boolean);
    if (base.some((b, i) => parts[i] !== b)) return null;
    let cur = rootId;
    for (const part of parts.slice(base.length)) {
      cur = nodes[cur].children.find((c) => nodes[c].name === part && !nodes[c].removed);
      if (cur == null) return null;
    }
    return cur;
  };

  const stats = () => {
    let dirs = 0;
    descendants(rootId, (n) => n.kind === 'dir' && dirs++);
    return {
      root_id: rootId,
      root_path: rootPath,
      size: nodes[rootId].size,
      // The mock tree is small; scale counts to look like a real disk.
      files: nodes[rootId].items * 1873,
      dirs: (dirs + 1) * 211,
      unreadable: rootPath === '/' ? 14 : 0,
      hidden: hiddenBytes,
      complete: true,
      scanned_at_ms: Date.now(),
    };
  };
  let hiddenBytes = 0;

  const remove = (id) => {
    if (id === rootId || !live(id)) return 0;
    const n = nodes[id];
    const stack = [id];
    while (stack.length) {
      const c = stack.pop();
      nodes[c].removed = true;
      stack.push(...nodes[c].children);
    }
    const p = nodes[n.parent];
    p.children = p.children.filter((c) => c !== id);
    for (let a = n.parent; a != null && a >= 0; a = nodes[a].parent) {
      nodes[a].size -= n.size;
      nodes[a].items -= n.items;
      if (nodes[a].cats) {
        if (n.cats) n.cats.forEach((v, i) => (nodes[a].cats[i] -= v));
        else nodes[a].cats[n.cat] -= n.size;
      }
      const pa = nodes[a].parent;
      if (pa != null && pa >= 0) nodes[pa].children.sort((x, y) => nodes[y].size - nodes[x].size);
      if (a === rootId) break;
    }
    if (n.kind === 'hidden') hiddenBytes = 0;
    return n.size;
  };

  // Builds the sample tree for a path (re-rooted on a sub folder when the
  // path matches one) and returns the matching volume, if any.
  const prepare = (path) => {
    nodes = makeTree(7);
    rootPath = path;
    hiddenBytes = 0;
    rootId = 0;
    if (path !== '/') {
      const parts = path.split('/').filter(Boolean);
      let cur = 0;
      for (const p of parts) {
        const next = nodes[cur].children.find((c) => nodes[c].name === p);
        if (next == null) {
          cur = -1;
          break;
        }
        cur = next;
      }
      if (cur > 0) rootId = cur;
    }
    const vol = VOLUMES.find((v) => v.mount_point === path);
    if (vol && rootId === 0) {
      const used = vol.total - vol.available;
      hiddenBytes = Math.max(0, used - nodes[0].size);
      if (hiddenBytes > 0) {
        const h = { id: nodes.length, name: 'Hidden space', parent: 0, kind: 'hidden', size: hiddenBytes, items: 0, cat: 7, children: [], removed: false };
        nodes.push(h);
        nodes[0].children.push(h.id);
        nodes[0].size += hiddenBytes;
        nodes[0].children.sort((a, b) => nodes[b].size - nodes[a].size);
      }
    }
    return vol;
  };

  let currentScan = null;
  let dupTimer = null;
  // Saved scans, as the real app keeps them between sessions.
  const saved = new Map([
    ['/', { root_path: '/', scanned_at_ms: Date.now() - 2 * 3600 * 1000, elapsed_ms: 48_000, complete: true, size: 520e9, files: 4_751_801, dirs: 512_000, unreadable: 14, nodes: 0 }],
    ['/Volumes/Backup', { root_path: '/Volumes/Backup', scanned_at_ms: Date.now() - 26 * 3600 * 1000, elapsed_ms: 0, complete: false, size: 0, files: 0, dirs: 0, unreadable: 0, nodes: 0 }],
  ]);

  const VOLUMES = [
    { name: 'Macintosh HD', mount_point: '/', total: 994.66 * GB, available: 431.2 * GB, file_system: 'apfs', removable: false, kind: 'ssd', is_root: true },
    { name: 'Backup', mount_point: '/Volumes/Backup', total: 2000.4 * GB, available: 1213.9 * GB, file_system: 'apfs', removable: true, kind: 'hdd', is_root: false },
    { name: 'Samsung T7', mount_point: '/Volumes/Samsung T7', total: 1000.2 * GB, available: 402.7 * GB, file_system: 'exfat', removable: true, kind: 'ssd', is_root: false },
  ];

  const handlers = {
    app_info: () => ({ version: '0.5.0', os: 'macos', arch: 'aarch64', home: '/Users/joost', preview: true }),
    list_volumes: () => VOLUMES,
    pick_folder: () => '/Users/joost/Developer',
    saved_scans: () =>
      [...saved.values()].sort((a, b) => b.scanned_at_ms - a.scanned_at_ms),
    // Roughly what the real app's saved scans take (LZ4, ~11 bytes per item).
    scan_storage: () => ({
      count: saved.size,
      bytes: [...saved.values()].reduce((sum, s) => sum + (s.files + s.dirs) * 11, 0),
      keep: 12,
    }),
    clear_saved_scans: () => {
      if (currentScan) throw new Error('A scan is running; try again after it finishes.');
      saved.clear();
      return { count: 0, bytes: 0, keep: 12 };
    },
    open_saved: ({ path }) => {
      const record = saved.get(path);
      if (!record || !record.complete) throw new Error('no saved scan');
      const vol = prepare(path);
      return { scan_id: 0, stats: { ...stats(), scanned_at_ms: record.scanned_at_ms }, elapsed_ms: record.elapsed_ms, volume: vol || null, saved: true, previous_ms: path === '/' ? PREVIOUS_MS : null, file_dates: true };
    },
    start_scan: ({ path, full, exclude }) => {
      clearInterval(timer);
      const id = ++scanId;
      const previous = saved.get(path);
      const incremental = !!previous && !full;
      const vol = prepare(path);
      // Excluded folders inside this scan are left out, like the real scanner.
      for (const e of exclude || []) {
        const ex = findPath(e);
        if (ex != null && ex !== rootId) remove(ex);
      }
      const totalFiles = nodes[rootId].items;
      const totalBytes = nodes[rootId].size - hiddenBytes;
      const totalDirs = stats().dirs;
      const started = performance.now();
      // A rescan only reads folders that changed, so it is much quicker.
      const dur = incremental ? 1100 : 2600;
      const scannedAt = Date.now();
      const sample = [];
      descendants(rootId, (n) => n.kind === 'dir' && sample.length < 400 && sample.push(n.id));
      timer = setInterval(() => {
        const el = performance.now() - started;
        const f = Math.min(1, el / dur);
        const e = 1 - Math.pow(1 - f, 2);
        emit('scan-progress', {
          scan_id: id,
          files: Math.round(totalFiles * 1873 * e),
          dirs: Math.round(totalDirs * e),
          bytes: Math.round(totalBytes * e),
          errors: Math.round(14 * e),
          reused: incremental ? Math.round(totalDirs * 0.97 * e) : 0,
          incremental,
          current: pathOf(sample[Math.floor(f * (sample.length - 1))] ?? rootId),
          elapsed_ms: el,
        });
        if (f >= 1) {
          clearInterval(timer);
          currentScan = null;
          const s = { ...stats(), scanned_at_ms: scannedAt };
          saved.set(path, { root_path: path, scanned_at_ms: scannedAt, elapsed_ms: el, complete: true, size: s.size - hiddenBytes, files: s.files, dirs: s.dirs, unreadable: s.unreadable, nodes: nodes.length });
          emit('scan-done', { scan_id: id, stats: s, elapsed_ms: el, volume: vol || null, saved: false, previous_ms: previous?.complete ? previous.scanned_at_ms : null, file_dates: true });
        }
      }, 70);
      currentScan = { path, scannedAt };
      return id;
    },
    cancel_scan: () => {
      clearInterval(timer);
      if (currentScan) {
        // Like the real app: an interrupted scan is kept and continued later.
        saved.set(currentScan.path, { root_path: currentScan.path, scanned_at_ms: currentScan.scannedAt, elapsed_ms: 0, complete: false, size: 0, files: 0, dirs: 0, unreadable: 0, nodes: 0 });
        currentScan = null;
      }
      emit('scan-failed', { scan_id: scanId, cancelled: true, message: 'scan cancelled' });
    },
    get_view: ({ id, depth }) => {
      if (!live(id)) throw new Error('item no longer exists');
      const root = view(id, Math.min(depth, 8), Math.max(1, nodes[id].size * (0.5 / 360)));
      return {
        root,
        crumbs: ancestors(id).map((a) => ({ id: a, name: a === rootId ? rootPath : nodes[a].name })),
        path: pathOf(id),
      };
    },
    list_children: ({ id, limit }) => {
      if (!live(id)) return [];
      const kids = nodes[id].children;
      const out = kids.slice(0, limit).map(leaf);
      if (kids.length > limit) {
        const rest = kids.slice(limit);
        out.push({ id: -1, name: '', size: rest.reduce((s, c) => s + nodes[c].size, 0), items: rest.length, kind: 'group', cat: 7 });
      }
      return out;
    },
    largest_files: ({ id, limit, olderThanDays }) => {
      const files = [];
      const before = olderThanDays ? Date.now() - olderThanDays * 86_400_000 : null;
      descendants(id, (n) => n.kind === 'file' && (!before || (n.mtime && n.mtime < before)) && files.push(n));
      return files.sort((a, b) => b.size - a.size).slice(0, limit).map((n) => hit(id, n));
    },
    search: ({ id, query, limit }) => {
      const q = query.trim().toLowerCase();
      if (!q) return [];
      const found = [];
      descendants(id, (n) => n.kind !== 'hidden' && n.name.toLowerCase().includes(q) && found.push(n));
      return found.sort((a, b) => b.size - a.size).slice(0, limit).map((n) => hit(id, n));
    },
    type_totals: ({ id }) => {
      const n = nodes[id];
      const cats = n.cats ? [...n.cats] : new Array(8).fill(0);
      if (!n.cats && n.kind !== 'hidden') cats[n.cat] = n.size;
      return { cats, hidden: id === rootId ? hiddenBytes : 0, total: n.size };
    },
    node_path: ({ id }) => pathOf(id),
    resolve_items: ({ ids }) => {
      const set = new Set(ids);
      return ids.filter(live).map((id) => {
        const n = nodes[id];
        const covered = ancestors(id).slice(0, -1).some((a) => set.has(a));
        return { id, name: n.name, size: n.size, kind: n.kind, cat: n.cat, path: pathOf(id), covered };
      });
    },
    load_comparison: () => {
      if (rootPath !== '/' || !nodes) return null;
      comparing = true;
      return { previous_ms: PREVIOUS_MS, delta: delta(nodes[rootId]) };
    },
    changes: ({ id, limit }) => {
      if (!comparing || !live(id)) return null;
      const items = nodes[id].children
        .map((c) => nodes[c])
        .filter((n) => !n.removed && n.kind !== 'hidden' && delta(n) !== 0)
        .map((n) => ({ id: n.id, name: n.name, size: n.size, kind: n.kind, cat: n.cat, has_children: n.children.length > 0, delta: delta(n), new: n.prev == null }))
        .sort((a, b) => Math.abs(b.delta) - Math.abs(a.delta))
        .slice(0, limit);
      return { previous_ms: PREVIOUS_MS, delta: delta(nodes[id]), size: nodes[id].size, items };
    },
    find_duplicates: ({ id, minSize }) => {
      clearInterval(dupTimer);
      const job = ++scanId;
      const files = [];
      descendants(id, (n) => n.kind === 'file' && n.dup != null && n.size >= minSize && files.push(n));
      const started = performance.now();
      const dur = 1400;
      dupTimer = setInterval(() => {
        const f = Math.min(1, (performance.now() - started) / dur);
        const phase = f < 0.25 ? 1 : f < 0.45 ? 2 : 3;
        const total = phase === 1 ? files.length * 40 : files.length;
        emit('dup-progress', { job, phase, files_done: Math.round(total * Math.min(1, f * 1.2)), files_total: total, bytes_done: Math.round(f * 18e9), bytes_total: 18e9 });
        if (f < 1) return;
        clearInterval(dupTimer);
        const groups = new Map();
        for (const n of files) groups.set(n.dup, [...(groups.get(n.dup) || []), n]);
        const out = [...groups.values()]
          .filter((g) => g.length > 1)
          .map((g) => {
            const copyName = (n) => (/( copy( \d+)?| \(\d+\)| - copy| kopie)$/i.test(n.name.replace(/\.[^.]+$/, '')) ? 1 : 0);
            g.sort((a, b) => copyName(a) - copyName(b) || pathOf(a.id).split('/').length - pathOf(b.id).split('/').length || pathOf(a.id).localeCompare(pathOf(b.id)));
            return { len: g[0].size, size: g[0].size, wasted: g[0].size * (g.length - 1), files: g.map((n) => ({ id: n.id, name: n.name, path: pathOf(n.id), cat: n.cat, size: n.size })) };
          })
          .sort((a, b) => b.wasted - a.wasted);
        emit('dup-done', { job, root: id, cancelled: false, groups: out, total_groups: out.length, wasted: out.reduce((s, g) => s + g.wasted, 0) });
      }, 90);
      return job;
    },
    cancel_duplicates: () => {
      clearInterval(dupTimer);
      emit('dup-done', { job: scanId, root: 0, cancelled: true, groups: [], total_groups: 0, wasted: 0 });
      return null;
    },
    cleanup_suggestions: () => {
      const home = '/Users/joost';
      const out = [];
      const place = (key, rel, info = false) => {
        const id = findPath(`${home}/${rel}`);
        if (id == null) return;
        const kids = nodes[id].children.filter((c) => !nodes[c].removed);
        out.push({ key, focus: id, path: pathOf(id), size: nodes[id].size, count: kids.length, ids: info ? [] : kids, truncated: false, info });
      };
      place('caches', 'Library/Caches');
      place('xcode_derived', 'Library/Developer/Xcode/DerivedData');
      place('ios_backups', 'Library/Application Support/MobileSync/Backup');
      place('trash', '.Trash', true);
      const nm = [];
      const walk = (id) => {
        for (const c of nodes[id].children) {
          const n = nodes[c];
          if (n.removed || n.kind !== 'dir') continue;
          if (n.name === 'node_modules') nm.push(n);
          else walk(c);
        }
      };
      walk(rootId);
      if (nm.length) out.push({ key: 'node_modules', focus: null, path: null, size: nm.reduce((s, n) => s + n.size, 0), count: nm.length, ids: nm.map((n) => n.id), truncated: false, info: false });
      const dl = findPath(`${home}/Downloads`);
      if (dl != null) {
        const cutoff = Date.now() - 90 * 86_400_000;
        const old = [];
        descendants(dl, (n) => n.kind === 'file' && n.mtime && n.mtime < cutoff && old.push(n));
        old.sort((a, b) => b.size - a.size);
        if (old.length) out.push({ key: 'old_downloads', focus: dl, path: pathOf(dl), size: old.reduce((s, n) => s + n.size, 0), count: old.length, ids: old.map((n) => n.id), truncated: false, info: false });
      }
      return out.filter((x) => x.size >= 20 * MB).sort((a, b) => b.size - a.size);
    },
    export_csv: ({ id, fileName }) => {
      const rows = ['\ufeffPath,Type,Size (bytes),Size,Items,Modified'];
      const q = (v) => (/[",\n]/.test(v) ? `"${v.replace(/"/g, '""')}"` : v);
      const walk = (nid) => {
        const n = nodes[nid];
        if (n.removed) return;
        const date = n.mtime ? new Date(n.mtime).toISOString().slice(0, 10) : '';
        rows.push([q(pathOf(nid)), n.kind === 'dir' ? 'folder' : n.kind, Math.round(n.size), `${(n.size / 1e9).toFixed(1)} GB`, n.items, n.kind === 'dir' ? '' : date].join(','));
        for (const c of n.children) if (nodes[c].kind === 'dir' || nodes[c].size >= 1e6) walk(c);
      };
      walk(id);
      const url = URL.createObjectURL(new Blob([rows.join('\n') + '\n'], { type: 'text/csv' }));
      const a = Object.assign(document.createElement('a'), { href: url, download: fileName });
      document.body.appendChild(a);
      a.click();
      a.remove();
      setTimeout(() => URL.revokeObjectURL(url), 5000);
      return fileName;
    },
    // The browser demo never offers updates.
    check_update: () => null,
    install_update: () => null,
    reveal_node: () => null,
    open_node: () => null,
    open_external: ({ url }) => {
      window.open(url, '_blank');
      return null;
    },
    trash_nodes: ({ ids }) => {
      const set = new Set(ids);
      const removed = [];
      let freed = 0;
      for (const id of ids) {
        if (!live(id) || ancestors(id).slice(0, -1).some((a) => set.has(a))) continue;
        freed += remove(id);
        removed.push(id);
      }
      const s = stats();
      return { removed, failed: [], freed, stats: s };
    },
  };

  return {
    async invoke(cmd, args) {
      const h = handlers[cmd];
      if (!h) throw new Error(`mock: unknown command ${cmd}`);
      await new Promise((r) => setTimeout(r, 8));
      return h(args || {});
    },
    listen(event, cb) {
      if (!listeners.has(event)) listeners.set(event, []);
      listeners.get(event).push(cb);
      return () => listeners.set(event, listeners.get(event).filter((x) => x !== cb));
    },
  };
}
