# sotf-plugin-crossover

Linkwitz-Riley 4th-order (LR4) crossover plugin supporting 2-way, 3-way, and 4-way frequency band splitting.

## Architecture

```
src/
  lib.rs -- CrossoverPlugin (Plugin), CrossoverPluginParams, CrossoverMode
```

Single-file crate. Data flow: Input -> LR4 crossover filter(s) -> output bands based on mode (lowpass only, highpass only, or all bands interleaved).

**Key types:**

- `CrossoverPlugin` -- Main plugin implementing `Plugin` (variable output channels). Uses `Lr4Crossover` (2-way) or `MultibandLr4Crossover` (3+way) from sotf-host.
- `CrossoverPluginParams` -- Serde config: `crossover_type`, `frequency`, `output` mode, `extra_frequencies` for multi-way.
- `CrossoverMode` -- Output selection: `Lowpass`, `Highpass`, `Both`.

**Channel mapping in Both mode:** Output is `num_channels * num_bands` channels, interleaved as `[band0_ch0, band0_ch1, ..., band1_ch0, band1_ch1, ...]`.

## Key Public API

- `CrossoverPlugin::new(channels, type, frequency, output) -> Result<Self, String>` -- 2-way (`lib.rs`)
- `CrossoverPlugin::new_multiway(channels, type, frequency, output, extra_frequencies) -> Result<Self, String>` -- 3/4-way (`lib.rs`)
- `CrossoverPlugin::from_params(channels, params) -> Result<Self, String>` (`lib.rs`)
- Implements `Plugin` trait (input_channels != output_channels in Both mode)

**Parameters:** `frequency` (20-20000 Hz, primary crossover point), `mode` (lowpass/highpass/both), `frequency_2`, `frequency_3` (extra crossover points for multi-way).

## Testing

```bash
cargo test -p sotf-plugin-crossover
```

## Important Notes

- LR4 crossover sums to flat magnitude (all-pass) when bands are recombined, but introduces group delay. Per-sample comparison with undelayed input is invalid; use RMS energy comparison.
- Maximum 4 bands (3 crossover points). The `band_flat` buffer and `band_slices` array use fixed-size `[&mut [f32]; 4]` to avoid per-frame heap allocation.
- Frequencies are automatically sorted and deduplicated. Extra frequencies provided via `extra_frequencies` are combined with the primary `frequency`.
- Frequency smoothing uses `LogSmoother` (logarithmic interpolation, 20ms) for click-free crossover point changes.
- In Lowpass/Highpass modes with multi-way crossover, only the lowest/highest band is output respectively. Output channel count equals input channel count.
- DC passes through the lowest band; this is a reliable test for correct crossover operation.
