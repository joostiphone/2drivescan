# Changelog

All notable changes to 2DriveScan. The website shows this file on its changelog page;
the Dutch version lives in `CHANGELOG.nl.md`.

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
