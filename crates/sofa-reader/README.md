# sofa-reader

Pure-Rust SOFA (HDF5/NetCDF4) file reader and writer for HRTF data.

## Overview

Provides reading and writing of Spatially Oriented Format for Acoustics (SOFA) files, commonly used for storing Head-Related Transfer Function (HRTF) measurements. Built entirely in Rust with no C dependencies.

## Features

- **Pure Rust**: No dependency on system HDF5/NetCDF4 libraries
- **Read & Write**: Load existing SOFA files and create new ones
- **HRTF API**: High-level types like `SofaFile`, `HrtfData`, and `SourcePosition`
- **Coordinate Conversion**: Spherical/cartesian coordinate system support
- **Nearest-Position Lookup**: Find closest HRTF measurement for a given source direction
- **SQLite Integration**: Load `.hrtfdb` SQLite databases
- **Deflate Support**: Optional zlib decompression via `flate2` (enabled by default)

## Usage

```rust
use sofa_reader::SofaReader;

let reader = SofaReader::open("hrtf.sofa")?;
let dims = reader.dimension("M")?;
let data = reader.read_f32("Data.IR")?;
```

### HRTF API

```rust
use sofa_reader::{SofaFile, SourcePosition};

let sofa = SofaFile::open("hrtf.sofa")?;
let hrtf = sofa.hrtf_data()?;

// Find nearest HRTF for a direction
let pos = SourcePosition::spherical(30.0, 0.0, 1.0); // azimuth, elevation, distance
let (left, right) = hrtf.get_hrtf_nearest(&pos)?;
```

## Module Layout

- `hdf5` — Low-level HDF5/NetCDF4 file parser
- `hrtf` — High-level HRTF data structures and queries
- `error` — Error types (`SofaError`, `Result`)

## Features

| Feature  | Default | Description                            |
|----------|---------|----------------------------------------|
| `deflate`| Yes     | Enables zlib decompression via `flate2`|

## Testing

```bash
cargo test -p sofa-reader --lib
cargo check -p sofa-reader && cargo clippy -p sofa-reader
```

## License

See the root workspace `LICENSE` file.
