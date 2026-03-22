# autoeq-datagen (lib: `autoeq_datagen`)

Generate simulated room measurement data for RoomEQ testing using BEM/FEM acoustic solvers.

## Key Components

- `scenarios.rs` -- Room geometry, speaker/listener positions, crossover filters (17 scenarios)
- `bem_runner.rs` -- BEM solver driver (math-bem)
- `fem_runner.rs` -- FEM solver driver (math-fem)
- `hf_extension.rs` -- Extends 20-500 Hz simulation to 20 kHz with synthetic speaker response
- `csv_export.rs` -- Converts complex pressures to CSV (SPL + phase)
- `roomeq_config_gen.rs` -- Generates `RoomConfig` JSON for roomeq

## Binaries

- `generate-roomeq-data` -- CLI for generating room measurement data

## Dependencies

- `math-bem` / `math-fem` -- Acoustic solvers
- `math-xem-common` -- Shared types
- `autoeq` -- RoomConfig types

## Testing

```bash
cargo test -p autoeq-datagen --lib
cargo test -p autoeq-datagen --test pipeline_test
cargo check -p autoeq-datagen && cargo clippy -p autoeq-datagen
```

## Important Notes

- Subwoofer sources (names starting with "sub") are automatically grouped under "lfe" in config
- Both BEM and FEM solvers return the same `SimulationOutput` format and are interchangeable
- HF extension above 500 Hz is synthetic (not physically simulated)
