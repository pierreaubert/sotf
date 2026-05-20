# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.4] - 2025-05-17

### Changed
- Edition bumped to 2024.

## [0.1.2] - 2025-05-13

### Fixed
- Various bug fixes.

## [0.1.1] - 2025-05-13

### Added
- Added the higher-level HRTF API previously hosted in `sotf-host`: `SofaFile`,
  `SourcePosition`, `HrtfData`, coordinate conversion, nearest-position lookup,
  and `.hrtfdb`/SQLite loading.

### Changed
- `sotf-host` can now re-export SOFA/HRTF types from `sofa-reader`, allowing
  crates such as `autoeq` to depend on the focused reader crate instead of the
  full plugin host.

## [0.1.0] - 2025-05-13

### Added
- Initial release of `sofa-reader`: in-house pure-Rust SOFA (HDF5/NetCDF4) file
  reader and writer for HRTF data. This is not an upstream fork; it is a
  purpose-built reader that extracts only the subset of HDF5/NetCDF4 needed for
  SOFA HRTF files.
- `deflate` (default) feature wires zlib decompression via `flate2` so
  compressed SOFA files load without C dependencies.
- Extended Apple-platform build gates so the crate compiles on tvOS / watchOS /
  visionOS alongside macOS / iOS.
