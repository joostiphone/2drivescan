# Changelog

All notable changes to 2DriveScan. The website shows this file on its changelog page;
the Dutch version lives in `CHANGELOG.nl.md`.

## 0.2.1 – 2026-11-03

- Fixed: a finished scan was saved as "unfinished" when some folders could not be read (normal on macOS
  without Full Disk Access), so the app returned to the start screen instead of showing the result. Use
  **Continue scan** once to finish it; the progress you already have is kept.

## 0.2.0 – 2026-11-03

Much faster scans, and you no longer have to start from zero.

- **Cloud files are no longer downloaded.** On macOS, scanning iCloud Drive and other cloud folders made the
  system download everything that was only in the cloud, which could make a scan take half an hour. Those
  folders are now skipped; files that are not downloaded take no disk space anyway.
- **Faster macOS scanner** that reads names, sizes and dates of many files per system call (`getattrlistbulk`).
- **Scans are saved.** The start screen shows when each drive was last scanned; open the result instantly.
- **Quick rescans.** A rescan only reads the folders that changed since the last scan and takes over the rest,
  usually seconds instead of minutes. Hold Alt/Option while clicking Rescan for a full scan.
- **Interrupted scans continue.** Cancel a scan or quit the app, and the next scan picks up where it stopped.

## 0.1.1 – 2026-11-03

- The macOS app is now signed with a Developer ID certificate and notarized by Apple, so it opens
  without security warnings.

## 0.1.0 – 2026-11-03

The first public release.

- **Interactive sunburst** of any drive or folder, with smooth animated zooming.
- **Fast parallel scanner** written in Rust that counts real on-disk sizes and handles hard links.
- **Hidden space**: used space that no file accounts for is shown as its own segment.
- **Side panel** with folder contents, the largest files, a file type breakdown and search.
- **Color by folder or by file type.**
- **Collector**: drag items in and move them to the Trash or Recycle Bin in one go.
- Light and dark mode, English and Dutch.
- Installers for macOS (universal), Windows (x64, ARM64) and Linux (deb, rpm, AppImage for x64 and ARM64).
