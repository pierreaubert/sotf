# autoeq-datagen

Generate simulated room measurement data for testing the RoomEQ optimizer.

This crate uses BEM (Boundary Element Method) and FEM (Finite Element Method)
acoustic solvers to compute the sound pressure field in a virtual room, then
exports the results as CSV measurement files and a `config.json` that can be
fed directly into `roomeq`.

## Quick Start

```bash
# Generate data for all 17 scenarios using BEM (default)
cargo run --bin generate-roomeq-data --release -- \
  --output-dir data_tests/roomeq/generated

# Use FEM solver instead
cargo run --bin generate-roomeq-data --release -- \
  --solver fem --output-dir data_tests/roomeq/generated

# Run both solvers (output goes to bem/ and fem/ subdirectories)
cargo run --bin generate-roomeq-data --release -- \
  --solver both --output-dir data_tests/roomeq/generated

# Generate a single scenario
cargo run --bin generate-roomeq-data --release -- \
  --scenario medium_surround_5_1 --output-dir data_tests/roomeq/generated

# Verbose logging
cargo run --bin generate-roomeq-data --release -- -v \
  --output-dir data_tests/roomeq/generated
```

## CLI Options

| Flag | Default | Description |
|------|---------|-------------|
| `-s, --solver` | `bem` | Solver to use: `bem`, `fem`, or `both` |
| `-o, --output-dir` | `data_tests/roomeq/generated` | Output directory |
| `--scenario` | (all) | Run a single scenario by name |
| `-v, --verbose` | off | Enable debug-level logging |

## Scenarios

All scenarios simulate the 20–500 Hz range (100 log-spaced frequency points),
then extend to 20 kHz with a synthetic speaker response shape and smooth random
noise. Boundaries use realistic absorption coefficients (floor 0.3, ceiling
0.05, walls 0.1).

### Stereo & 2.1

| Name | Room | Speakers | Subs | LPs | Description |
|------|------|----------|------|-----|-------------|
| `small_stereo_2_0` | 3x3x2.4 m | L, R | — | 1 | Fullrange stereo |
| `small_stereo_2_1` | 3x3x2.4 m | L, R | 1 | 1 | HP mains + LP sub |
| `small_multi_sub_2` | 3x3x2.4 m | L, R | 2 | 1 | 2 front-corner subs |
| `medium_stereo_2_0` | 5x4x2.5 m | L, R | — | 1 | Fullrange stereo |
| `medium_stereo_2_1` | 5x4x2.5 m | L, R | 1 | 1 | HP mains + LP sub |
| `medium_multi_sub_4` | 5x4x2.5 m | L, R | 4 | 1 | 4 corner subs |
| `medium_multi_seat` | 5x4x2.5 m | L, R | — | 3 | 3 listening positions |
| `large_stereo_2_0` | 7x5.5x2.6 m | L, R | — | 1 | Fullrange stereo |
| `large_stereo_2_1` | 7x5.5x2.6 m | L, R | 1 | 1 | HP mains + LP sub |
| `large_multi_sub_4` | 7x5.5x2.6 m | L, R | 4 | 1 | 4 corner subs |
| `large_multi_seat_2_1` | 7x5.5x2.6 m | L, R | 1 | 3 | 2.1, 3 seats |
| `medium_multi_sub_multi_seat` | 5x4x2.5 m | L, R | 2 | 2 | 2 subs, 2 seats |

### Surround (5.0 / 5.1 / 5.1.4)

Speaker placement follows ITU-R BS.775 and ITU-R BS.2051 guidelines. Surround
speakers are placed at approximately ±110° behind the listener. Height channels
(5.1.4) are near ceiling level at ±45° front and ±135° rear.

| Name | Room | Speakers | Subs | LPs | Description |
|------|------|----------|------|-----|-------------|
| `medium_surround_5_0` | 5x4x2.5 m | L, R, C, SL, SR | — | 1 | Fullrange 5.0 |
| `medium_surround_5_1` | 5x4x2.5 m | L, R, C, SL, SR | 1 | 1 | HP mains + LP sub |
| `medium_surround_5_1_4` | 5x4x2.5 m | L, R, C, SL, SR, TFL, TFR, TRL, TRR | 1 | 1 | Immersive audio |
| `large_surround_5_1` | 7x5.5x2.6 m | L, R, C, SL, SR | 1 | 1 | HP mains + LP sub |
| `large_surround_5_1_4` | 7x5.5x2.6 m | L, R, C, SL, SR, TFL, TFR, TRL, TRR | 1 | 1 | Immersive audio |

Channel abbreviations: L=left, R=right, C=center, SL=surround_left,
SR=surround_right, TFL=top_front_left, TFR=top_front_right,
TRL=top_rear_left, TRR=top_rear_right.

## Output Structure

For each (solver, scenario) pair, the binary creates a directory containing:

```
data_tests/roomeq/generated/
  bem/
    small_stereo_2_0/
      left_lp0.csv          # Left speaker → listening position 0
      right_lp0.csv         # Right speaker → listening position 0
      config.json           # RoomEQ config referencing these CSVs
    medium_surround_5_1/
      left_lp0.csv
      right_lp0.csv
      center_lp0.csv
      surround_left_lp0.csv
      surround_right_lp0.csv
      subwoofer_lp0.csv
      config.json
  fem/
    ...
```

### CSV Format

Each CSV has 200 rows (100 simulated + 100 HF extension) with columns:

```
freq,spl,phase
20.0000,49.9664,-172.6980
20.6610,49.5056,-173.2463
...
20000.0000,27.0280,27.3745
```

- **freq**: Frequency in Hz (20–20000, log-spaced)
- **spl**: Sound Pressure Level in dB (relative to 20 µPa)
- **phase**: Phase in degrees (-180 to +180)

### config.json

A `RoomConfig` JSON that can be passed directly to the `roomeq` binary:

```json
{
  "speakers": {
    "left": { "name": "left", "csv": "left_lp0.csv" },
    "right": { "name": "right", "csv": "right_lp0.csv" },
    "lfe": { "name": "subwoofer", "csv": "subwoofer_lp0.csv" }
  },
  "optimizer": {
    "num_filters": 7,
    "seed": 42
  }
}
```

Subwoofer sources (names starting with "sub") are automatically grouped under
the "lfe" key. Multiple subs produce a `MultiSub` config with per-sub
measurements.

## Architecture

### Pipeline

```
Scenario (room geometry + speaker positions + listener positions)
    │
    ├──▶ BEM solver (math-bem)  ──┐
    │                              ├──▶ SimulationOutput (complex pressure per source/LP/freq)
    └──▶ FEM solver (math-fem)  ──┘
                                        │
                                        ▼
                                   HF Extension (500 Hz → 20 kHz synthetic)
                                        │
                                        ├──▶ CSV export (freq, spl, phase)
                                        │
                                        └──▶ config.json (RoomConfig for roomeq)
```

### Modules

| Module | Purpose |
|--------|---------|
| `scenarios` | Defines room geometry, speaker/listener positions, and crossover filters |
| `bem_runner` | Drives the BEM solver (surface mesh, integral equations) |
| `fem_runner` | Drives the FEM solver (tetrahedral mesh, Helmholtz equation, direct LU) |
| `hf_extension` | Extends 20–500 Hz simulation data to 20 kHz with synthetic speaker response |
| `csv_export` | Converts complex pressures to CSV (SPL + phase) |
| `roomeq_config_gen` | Generates `RoomConfig` JSON from scenario + CSV paths |

### Solvers

**BEM** (Boundary Element Method):
- Solves on the room surface only (no volume mesh)
- Accurate for simple geometries
- Scales with surface area, not volume
- Uses `math-bem` crate

**FEM** (Finite Element Method):
- Solves on a 3D tetrahedral volume mesh
- Uses direct LU factorization (900–3600 DOFs, ~1 ms per frequency solve)
- Source term includes propagation phase `e^{-ikr}` for realistic phase response
- Boundary absorption via impedance conditions
- Uses `math-fem` crate

Both solvers return the same `SimulationOutput` format and are interchangeable.
The FEM solver is faster for small rooms; BEM is more efficient for large rooms.

### HF Extension

Below 500 Hz, room interaction dominates — this is the physically simulated
region. Above 500 Hz, the response is primarily the speaker's direct sound.
The extension module synthesizes the 500–20000 Hz region:

- **Main speakers**: Presence bump at 3 kHz (+1.5 dB), treble rolloff above
  8 kHz (-6 dB at 20 kHz), plus slowly-varying cosine-interpolated random
  noise [-5, +3] dB. Seeded per (source, LP) pair for deterministic output.
- **Subwoofers**: Steep rolloff at -60 dB/octave above the simulation range
  (sub crossover).

### Source Configuration

Sources use crossover filters to model real speaker systems:

| Type | Crossover | Typical use |
|------|-----------|-------------|
| Fullrange | None | Stereo mains without subwoofer |
| Highpass | 80 Hz, 4th-order Butterworth | Mains in systems with subwoofer |
| Lowpass | 80 Hz, 4th-order Butterworth | Subwoofer |

## Development

### Adding a New Scenario

1. Add a function `scenario_XX_name()` in `src/scenarios.rs` returning a
   `Scenario` with room, sources, listening positions, and source names
2. Add it to `all_scenarios()` and update the doc comment count
3. Update the `test_all_scenarios_count` assertion
4. Add config validation tests in `tests/pipeline_test.rs`
5. Run: `cargo test -p autoeq-datagen --lib && cargo test -p autoeq-datagen --test pipeline_test`

Source naming conventions:
- Main speakers: `left`, `right`, `center`, `surround_left`, `surround_right`
- Height channels: `top_front_left`, `top_front_right`, `top_rear_left`, `top_rear_right`
- Subwoofers: any name starting with `sub` (e.g., `subwoofer`, `sub1`, `sub2`)
  — these are automatically grouped under `lfe` in the config

### Testing

```bash
# Library unit tests (scenarios, HF extension)
cargo test -p autoeq-datagen --lib

# Integration tests (full BEM pipeline + config validation)
cargo test -p autoeq-datagen --test pipeline_test

# Lint
cargo check -p autoeq-datagen && cargo clippy -p autoeq-datagen
```

### Key Types

```rust
/// Output from a BEM or FEM simulation.
/// Pressures indexed as [source_idx][lp_idx][freq_idx].
pub struct SimulationOutput {
    pub frequencies: Vec<f64>,
    pub pressures: Vec<Vec<Vec<Complex64>>>,
    pub source_names: Vec<String>,
}
```

### Dependencies

- `math-bem` / `math-fem`: Acoustic solvers
- `math-xem-common`: Shared types (room geometry, source config, etc.)
- `autoeq`: RoomConfig types for config generation
- `rayon`: Parallel frequency solving
- `rand`: Seeded RNG for HF extension noise
