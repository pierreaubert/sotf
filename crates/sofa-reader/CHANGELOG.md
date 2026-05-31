# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.6] - 2026-05-31

### Added
- Add typed `SofaFile::try_load`, `try_load_sqlite`, and `strict_load` APIs.

### Fixed
- Follow the HDF5 v1 B-tree right-edge key layout for group and chunk indexes.
- Fall back to creation-order dense link/attribute indexes when name indexes are absent.
- Borrow contiguous and unfiltered chunk data instead of cloning it unnecessarily.
- Copy chunk payloads row-wise instead of per element.
- Write explicit NaN fill-value messages for float datasets.
- Warn when dataset storage contains bytes beyond the declared shape.

## [0.1.5] - 2026-05-30

### Fixed
- Materialize declared HDF5 fill values for unallocated contiguous/chunked datasets instead of forcing zero-filled output.
- Return a clear unsupported error for compound dataset types in numeric read paths.
- Validate numeric dataset byte counts against declared shape and element size, trimming only explicit trailing bytes.
- Use dataset element size as shuffle fallback when filter pipeline metadata omits/zeros the shuffle element size.
- Preserve radius differences in nearest-neighbor HRTF lookup and replace full-sort nearest-3 selection with a single-pass tracker.

### Changed
- Prefer `CLASS=DIMENSION_SCALE` when detecting dimensions, with SOFA-name fallback for legacy files.
- `write_simple_free_field_hrtf` now writes `EmitterPosition` as `[C, I]` instead of `[E, C, I]` for single-emitter files.

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
