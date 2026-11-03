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

## Website (`website/`, 2drivescan.app)
- Laravel 13, no database, no build step. Texts in `website/lang/{en,nl}/site.php` (keep both in sync).
- The site shows downloads live from GitHub Releases, embeds `ui/` as a live demo, and renders
  `CHANGELOG.md` / `CHANGELOG.nl.md`. The server pulls `main` every 5 minutes (`website/deploy/update.sh`).
- **Whenever the app changes, update the website in the same change:** feature texts in both languages,
  `docs/screenshots/` (re-render with Playwright), and both changelogs. Pushing to `main` deploys it.
- Tests: `cd website && php artisan test && ./vendor/bin/pint --test`.
- Server: self-hosted at the owner's home network; it is not reachable from cloud sessions. Server work
  goes through the owner's Mac via Remote Control.

## Releasing
1. `node scripts/bump-version.mjs X.Y.Z`, run `cargo update --workspace` (updates Cargo.lock), commit.
2. Push `main`, then start the release: `git tag vX.Y.Z && git push origin vX.Y.Z`, or (cloud sessions cannot
   push tags) `gh workflow run release.yml --ref main`, which creates the tag on publish.
   `.github/workflows/release.yml` builds macOS (universal dmg, signed + notarized),
   Windows (x64 msi/nsis, arm64 nsis), Linux (x64 + arm64 deb/rpm/AppImage) and publishes the release.
3. Add the release to `CHANGELOG.md` and `CHANGELOG.nl.md` before tagging; the website picks up the new
   installers automatically.
