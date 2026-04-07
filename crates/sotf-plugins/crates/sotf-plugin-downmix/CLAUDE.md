# sotf-plugin-downmix

Phase-coherent multichannel to stereo downmix plugin with ITU-R BS.775 support and optional Lt/Rt matrix encoding.

## Architecture

```
src/
  lib.rs    -- DownmixPlugin (Plugin), DownmixPluginParams
  params.rs -- Centralized parameter specs
```

Data flow: Multichannel input -> per-channel gain (center, surround, height, LFE) -> optional phase-coherent frequency-domain processing -> optional Lt/Rt matrix encoding (surround channels phase-shifted) -> stereo output.

**Key types:**

- `DownmixPlugin` -- Main plugin implementing `Plugin`. N input channels -> 2 output channels. Uses FFT-based phase-coherent downmixing when enabled.
- `DownmixPluginParams` -- Serde config: input_channels, per-group gain (center, surround, height, LFE), phase_coherence, ITU mode, Lt/Rt mode.

**Phase-coherent mode:** Uses FFT to analyze phase relationships between channels and blend between simple summing (low frequencies) and phase-aligned summing (high frequencies), controlled by `phase_blend_low_hz` and `phase_blend_high_hz`.

## Key Public API

- `DownmixPlugin::new(input_channels) -> Self` (`lib.rs`)
- `DownmixPlugin::from_params(params) -> Self` (`lib.rs`)
- Implements `Plugin` trait: N input channels -> 2 output channels

**Parameters:** `center_gain_db`, `surround_gain_db`, `height_gain_db`, `lfe_gain_db`, `phase_coherence` (bool), `phase_blend_low_hz`, `phase_blend_high_hz`, `itu_mode` (bool, uses ITU-R BS.775 coefficients), `matrix_ltrt` (bool, Dolby Lt/Rt encoding).

## Testing

```bash
cargo test -p sotf-plugin-downmix
```

## Important Notes

- Speaker configuration is auto-detected from input channel count using `get_speaker_config_by_channels` from sotf-host.
- ITU mode uses standard ITU-R BS.775 downmix coefficients for 5.1 to stereo: center at -3dB, surround at -3dB.
- Lt/Rt (matrix_ltrt) applies 90-degree phase shift to surround channels before mixing, compatible with Dolby Pro Logic decoders.
- Phase-coherent processing adds latency (FFT-based) but preserves spatial cues better than simple coefficient mixing.
- Uses fast math (`fast_atan2`, `fast_cos`, `fast_sin`) from `math-dsp` and biquad filters from `math-iir-fir`.
- The plugin uses `realfft` for efficient real-only FFT operations.
