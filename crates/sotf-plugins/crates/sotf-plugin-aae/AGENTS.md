# sotf-plugin-aae

Active Acoustic Enhancement (AAE) plugin — LARES-inspired multichannel reverb. Takes stereo input and outputs multichannel audio (5.0–9.1.6) with synthesized early reflections and late reverberation distributed across speakers via VBAP.

## Architecture

- `lib.rs` — main plugin struct implementing `Plugin` (variable in/out channel counts).
- `params.rs` — parameter definitions (`AaePluginParams`, `build_parameters`).
- `delay_line.rs` — pre-delay and tap delays.
- `early_reflections.rs` — tapped-delay early-reflection generator with per-tap VBAP routing.
- `fdn.rs` — 8-line Hadamard feedback delay network for late reverb (time-variant).
- `hadamard.rs` — Hadamard mixing matrix.
- `tone_filter.rs` — global tone shaping for the reverb tail.

## Signal Flow

```
Stereo input → mono downmix → pre-delay → input diffusion →
  early reflections (tapped delay, per-tap VBAP)
+ FDN late reverb (8-line, Hadamard, time-variant)
→ multichannel speaker routing → output mixed with dry signal
```

## Features

- `qa` — enables `sotf-host/qa` and the `qa-aae` benchmark binary.

## Testing

```bash
cargo check -p sotf-plugin-aae && cargo clippy -p sotf-plugin-aae
cargo test -p sotf-plugin-aae
cargo run -p sotf-plugin-aae --features qa --bin qa-aae
```

## Important Notes

- Implements `Plugin` (not `ParametricInPlacePlugin`): output channel count differs from input (stereo → 5.0–9.1.6).
- VBAP routing depends on the active speaker layout from `sotf-host::speaker_config`.
- Pre-allocate FDN/delay-line buffers in `build()`; never resize on the audio path.
- AAE is zero-latency through its immediate direct path. Room preset and output
  layout are structural; rebuild rather than mutate them live.
- The LFE effects send is LR4-low-passed at 120 Hz and is not complementary
  bass management. Spatial rows must never route ER/FDN energy into LFE.
