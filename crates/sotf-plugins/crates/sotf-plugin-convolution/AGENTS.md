# sotf-plugin-convolution

FFT-based convolution plugin for impulse response processing, supporting uniform and non-uniform partitioned convolution (NUPC).

## Architecture

```
src/
  lib/convolution_plugin.rs -- host contract, loading, UPC processing, transitions
  lib/types.rs              -- serialized, active, completion, and retirement state
  params.rs                 -- centralized parameter specs and generated UI layout

plugins-spatial/src/nupc/ -- shared immutable NUPC kernels plus per-channel streaming histories
```

Data flow: IR file loaded (WAV/FLAC via Symphonia, resampled via rubato if needed) -> partitioned into FFT blocks -> input accumulated in `PARTITION_SIZE` (1024) chunks -> forward FFT -> complex multiply-accumulate with IR partitions (FDL ring buffer) -> inverse FFT -> overlap-add -> mix/gain application.

**Key types:**

- `ConvolutionPlugin` -- Main plugin implementing `ParametricInPlacePlugin`. Uses `ArcSwap<Option<ConvolutionState>>` for lock-free IR swapping.
- `ConvolutionState` -- Holds pre-computed frequency-domain IR partitions: `partitions[channel][partition][bin]`.
- `ConvolutionPluginParams` -- Serde config: `ir_file`, `mix`, `gain_db`, `use_nupc`, `zero_latency_head`, `head_taps`.

**Constants:** `PARTITION_SIZE = 1024`, `FFT_SIZE = 2048` (zero-padded for linear convolution).

## Key Public API

- `ConvolutionPlugin::new(channels, ir_file, mix, gain_db) -> Self` (`lib.rs`)
- `ConvolutionPlugin::from_params(channels, params) -> Self` (`lib.rs`)
- Implements `ParametricInPlacePlugin` trait

**Parameters:** `ir_file` (path string), `mix` (0-1), `gain_db`, `use_nupc` (bool, default true), `zero_latency_head` (bool), `head_taps` (default 128).

## Testing

```bash
cargo test -p sotf-plugin-convolution
```

## Important Notes

- IR loading uses Symphonia for decoding (FLAC, WAV, PCM) and rubato for sample rate conversion to match the plugin's sample rate.
- The FDL (Frequency Domain Line) uses a flat ring buffer with `fdl_head` pointer to avoid `rotate_right` overhead.
- NUPC (`nupc.rs`) uses non-uniform partition sizes for long IRs: small partitions for low latency on early reflections, larger partitions for efficiency on late reverb. Enabled by default.
- `zero_latency_head` mode processes the first `head_taps` samples via direct time-domain convolution (no latency), with the rest using partitioned FFT. Useful for preserving transient attacks.
- UPC accumulation stays on the callback thread; it never dispatches through Rayon's global pool.
- Complex multiply-accumulate uses SIMD (`complex_mul_add_simd` from sotf-host).
- Lock-free IR swapping via `ArcSwap` allows changing IRs without blocking the audio thread.
- Channel mapping: if IR has fewer channels than input, channels are mapped cyclically.
- Never drop a replaced backend or error string on the callback; use the retirement queues.
- Preserve the configured latency in inactive/loading/failed/cleared states.
