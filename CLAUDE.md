# 2DriveScan – notes for Claude

Free, open source DaisyDisk alternative. Tauri 2 app: Rust backend, plain HTML/CSS/JS frontend (no bundler).
The owner communicates in Dutch; the app UI and repo docs are English (UI also has a Dutch translation).

## Layout
- `crates/scan-core` – parallel scanner (rayon) + arena tree (`Tree`, stable `NodeId`s). Pure Rust, unit tests.
- `src-tauri/src/main.rs` – Tauri commands (scan, view, list, search, trash, reveal…); `volumes.rs` – drive list
  and per-OS scan rules (stay on one volume, macOS firmlinks, Linux pseudo filesystems).
- `ui/js/main.js` – app controller; `sunburst.js` – canvas chart with camera-based zoom animation;
  `mock.js` – demo backend used when `window.__TAURI__` is missing (browser preview / website demo);
  `i18n.js` – en + nl strings. Keep both languages in sync when adding strings.

## Conventions
- Never delete files permanently: removal always goes through the OS Trash (`trash` crate).
- New Tauri commands must be registered in `generate_handler!` and mirrored in `ui/js/mock.js`.
- Before pushing: `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace`.
- UI check without a desktop: `npm run preview`, then screenshot with Playwright.

## Releasing
1. `node scripts/bump-version.mjs X.Y.Z`, run `cargo update --workspace` (updates Cargo.lock), commit.
2. `git tag vX.Y.Z && git push origin main vX.Y.Z` – `.github/workflows/release.yml` builds macOS (universal dmg),
   Windows (x64 msi/nsis, arm64 nsis), Linux (x64 + arm64 deb/rpm/AppImage) and publishes the release.
3. Website (2drivescan.app) must be updated for every release too – hosting/SSH details still to come from the owner.
