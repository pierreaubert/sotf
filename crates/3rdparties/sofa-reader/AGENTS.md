# sofa-reader

Pure-Rust SOFA (HDF5/NetCDF4) file reader and writer for HRTF data.

## Overview

Vendored in-house library. The standard HDF5 library is hard to build portably across platforms; this crate extracts only the subset needed for SOFA HRTF files.

## Features

- `deflate` (default) -- Deflate compression support via `flate2`

## Testing

```bash
cargo test -p sofa-reader --lib
cargo check -p sofa-reader && cargo clippy -p sofa-reader
```

## Important Notes

- This is NOT an upstream fork -- it's a purpose-built SOFA reader
- Only supports the HDF5/NetCDF4 features needed for SOFA files
- Used by `sotf-host` for loading HRTF data for the binaural plugin
