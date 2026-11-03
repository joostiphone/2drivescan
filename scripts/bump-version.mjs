#!/usr/bin/env node
// Sets the app version everywhere it is defined.
//   node scripts/bump-version.mjs 0.2.0
// Then commit, and tag the release:  git tag v0.2.0 && git push origin main v0.2.0
import { readFileSync, writeFileSync } from 'node:fs';
import { join } from 'node:path';
import { fileURLToPath } from 'node:url';

const version = process.argv[2];
if (!/^\d+\.\d+\.\d+(-[0-9A-Za-z.]+)?$/.test(version || '')) {
  console.error('usage: node scripts/bump-version.mjs <major.minor.patch>');
  process.exit(1);
}

const root = join(fileURLToPath(new URL('.', import.meta.url)), '..');

function edit(rel, fn) {
  const file = join(root, rel);
  const before = readFileSync(file, 'utf8');
  const after = fn(before);
  if (before === after) {
    if (!before.includes(version)) throw new Error(`no version found in ${rel}`);
    console.log(`${rel} already at ${version}`);
    return;
  }
  writeFileSync(file, after);
  console.log(`updated ${rel}`);
}

const cargoVersion = (s) => s.replace(/^version = "[^"]+"/m, `version = "${version}"`);
edit('src-tauri/Cargo.toml', cargoVersion);
edit('crates/scan-core/Cargo.toml', cargoVersion);
edit('package.json', (s) => s.replace(/"version": "[^"]+"/, `"version": "${version}"`));
edit('package-lock.json', (s) => {
  const lock = JSON.parse(s);
  lock.version = version;
  if (lock.packages?.['']) lock.packages[''].version = version;
  return `${JSON.stringify(lock, null, 2)}\n`;
});
edit('ui/js/mock.js', (s) => s.replace(/version: '[^']+'/, `version: '${version}'`));
console.log(`\nVersion set to ${version}. Run \`cargo update --workspace\` to refresh Cargo.lock, then commit and tag v${version}.`);
