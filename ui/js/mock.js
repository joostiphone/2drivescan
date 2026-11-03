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
  const add = (name, parent, kind, size = 0, cat = CAT.other) => {
    const n = { id: nodes.length, name, parent, kind, size, items: kind === 'dir' ? 0 : 1, cat, children: [], removed: false };
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
          ['Application Support', 19, [['Steam', 9.5, { app: 2, other: 3 }], ['Slack', 2.4, { other: 1, image: 1 }], ['Code', 1.8, { code: 1 }], ['MobileSync', 5.1, { other: 1 }]]],
          ['Containers', 5, { other: 2, document: 1 }],
          ['Mail', 2, { document: 2, other: 1 }],
        ]],
        ['Developer', 32, projects],
        ['Downloads', 36, { archive: 4, document: 2, video: 1, image: 1, app: 1 }],
        ['Documents', 21, [['Work', 9, { document: 4, image: 1 }], ['Scans', 3.5, { document: 1 }], ['Archive', 6.5, { archive: 1, document: 1 }]]],
        ['Music', 16, [['Music', 11, many('Album ', 12, 11, { audio: 1 })], ['GarageBand', 5, { audio: 2 }]]],
        ['Desktop', 5.5, { image: 3, document: 2, video: 1 }],
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

  const leaf = (id) => {
    const n = nodes[id];
    return {
      id: n.id,
      name: id === rootId ? rootPath : n.name,
      size: n.size,
      items: n.items,
      kind: kindOf(n),
      cat: n.cat,
      has_children: n.children.length > 0,
    };
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
  const hit = (base, n) => ({ id: n.id, name: n.name, size: n.size, kind: n.kind, cat: n.cat, rel_path: relPath(base, n.id) });

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

  const VOLUMES = [
    { name: 'Macintosh HD', mount_point: '/', total: 994.66 * GB, available: 431.2 * GB, file_system: 'apfs', removable: false, kind: 'ssd', is_root: true },
    { name: 'Backup', mount_point: '/Volumes/Backup', total: 2000.4 * GB, available: 1213.9 * GB, file_system: 'apfs', removable: true, kind: 'hdd', is_root: false },
    { name: 'Samsung T7', mount_point: '/Volumes/Samsung T7', total: 1000.2 * GB, available: 402.7 * GB, file_system: 'exfat', removable: true, kind: 'ssd', is_root: false },
  ];

  const handlers = {
    app_info: () => ({ version: '0.1.1', os: 'macos', arch: 'aarch64', home: '/Users/joost', preview: true }),
    list_volumes: () => VOLUMES,
    pick_folder: () => '/Users/joost/Developer',
    start_scan: ({ path }) => {
      clearInterval(timer);
      const id = ++scanId;
      const full = makeTree(7);
      nodes = full;
      rootPath = path;
      hiddenBytes = 0;
      rootId = 0;
      // Re-root on a sub folder when the path matches one.
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
      const totalFiles = nodes[rootId].items;
      const totalBytes = nodes[rootId].size - hiddenBytes;
      const started = performance.now();
      const dur = 2600;
      const sample = [];
      descendants(rootId, (n) => n.kind === 'dir' && sample.length < 400 && sample.push(n.id));
      timer = setInterval(() => {
        const el = performance.now() - started;
        const f = Math.min(1, el / dur);
        const e = 1 - Math.pow(1 - f, 2);
        emit('scan-progress', {
          scan_id: id,
          files: Math.round(totalFiles * 1873 * e),
          dirs: Math.round(totalFiles * 211 * e),
          bytes: Math.round(totalBytes * e),
          errors: Math.round(14 * e),
          current: pathOf(sample[Math.floor(f * (sample.length - 1))] ?? rootId),
          elapsed_ms: el,
        });
        if (f >= 1) {
          clearInterval(timer);
          const s = stats();
          emit('scan-done', { scan_id: id, stats: s, elapsed_ms: el, volume: vol || null });
        }
      }, 70);
      return id;
    },
    cancel_scan: () => {
      clearInterval(timer);
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
    largest_files: ({ id, limit }) => {
      const files = [];
      descendants(id, (n) => n.kind === 'file' && files.push(n));
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
