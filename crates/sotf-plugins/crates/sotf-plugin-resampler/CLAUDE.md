# sotf-plugin-resampler

Sample rate conversion plugin using high-quality sinc interpolation via rubato.

## Architecture

```
src/
  lib.rs -- ResamplerPlugin (Plugin), ResamplerQuality
```

Single-file crate. Data flow: Interleaved input -> deinterleave to planar -> rubato async resampler (sinc interpolation) -> interleave back to output.

**Key types:**

- `ResamplerPlugin` -- Main plugin implementing `Plugin` (output frame count differs from input based on ratio). Uses rubato `Async<f32>` resampler internally.
- `ResamplerQuality` -- Preset enum: `Fast` (64-tap), `Medium` (128-tap), `High` (256-tap sinc filter).

**Buffer management:** Input is accumulated in planar buffers until a full `chunk_size` is available, then processed through rubato. Output frames are buffered and drained incrementally to match the caller's frame count.

## Key Public API

- `ResamplerPlugin::new(channels, input_rate, output_rate) -> Self` (`lib.rs`)
- `ResamplerPlugin::with_quality(channels, input_rate, output_rate, quality) -> Self` (`lib.rs`)
- Implements `Plugin` trait (output frame count varies based on resampling ratio)

**Parameters:** `quality` (choice: fast/medium/high), `input_sample_rate`, `output_sample_rate`.

## Testing

```bash
cargo test -p sotf-plugin-resampler
```

## Important Notes

- Output buffer size differs from input: for 44.1kHz to 48kHz, output has ~8.8% more frames. Callers must allocate sufficient output buffer space.
- Uses `SincInterpolationParameters` with `BlackmanHarris2` window and `Cubic` interpolation type.
- The resampler is recreated on `initialize()` with the actual sample rates. Construction uses placeholder values.
- Rubato operates on planar (non-interleaved) buffers internally. The plugin handles interleaved-to-planar conversion.
- When input and output rates match, the plugin passes through without resampling.
- Chunk size is fixed by rubato at creation time. Input accumulation handles the mismatch between caller frame sizes and rubato's expected chunk size.
