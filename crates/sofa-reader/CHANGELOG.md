# 0.1.2-0.1.4

- Bug fixes

# 0.1.1

## New

- Added the higher-level HRTF API previously hosted in `sotf-host`: `SofaFile`,
  `SourcePosition`, `HrtfData`, coordinate conversion, nearest-position lookup,
  and `.hrtfdb`/SQLite loading.

## Changes

- `sotf-host` can now re-export SOFA/HRTF types from `sofa-reader`, allowing
  crates such as `autoeq` to depend on the focused reader crate instead of the
  full plugin host.

# 0.1.0

In-house pure-Rust SOFA (HDF5/NetCDF4) file reader and writer for HRTF data. This is not an upstream fork; it is a purpose-built reader that extracts only the subset of HDF5/NetCDF4 needed for SOFA HRTF files.

## New

- Initial release of `sofa-reader`: removes the workspace-wide dependency on `hdf5/netcdf` by reimplementing the narrow slice of the format actually consumed by `sotf-host` for binaural HRTF loading.
- `deflate` (default) feature wires zlib decompression via `flate2` so compressed SOFA files load without C dependencies.

## Changes

- Extended Apple-platform build gates so the crate compiles on tvOS / watchOS / visionOS alongside macOS / iOS.
