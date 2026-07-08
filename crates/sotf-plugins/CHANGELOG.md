# 0.7.10

## New

- added a AAE plugin (Active Acoustic Enhancement)
- added a spectral compressor plugin
- added a linear phase eq plugin
- added a dynamic eq plugin
- added a linear phase eq plugin
- added a dither plugin
- added a beamformer plugin

## Bugs

- fixed A/B compare with proper latency compensation
- merge compressor and multiband compressor
- merge expander and multiband expander

# 0.6.6 (unreleased)

## New

### QA / edge-case coverage
- Added `tests/plugin_high_channel_tests.rs` covering 5.1 / 7.1.4 layout transitions, high-channel matrix identity, downmix to stereo, and extreme-parameter/NaN/Inf output sanity for built-in plugins (QA-PLUGIN-003/004, QA-DSP-001/002/003).
- Extended `tests/plugin_high_channel_tests.rs` with block-size variation, silence/denormal input, high-level input, missing-file-path rejection, STFT `context.num_frames` return-value, and latency-reporting tests for spatial/convolution and advanced DSP plugins (QA-DSP-001/002, QA-PLUGIN-003).
- Added explicit routing layout-transition coverage for `band_split`/`band_merge` round-trip at 5.1 / 7.1.4, `mono_to_stereo` 1ch→2ch, and `ab_compare` A/B switching without channel-layout change (QA-DSP-003).
- Hardened external plugin sandbox config parsing: untrusted configs are now rejected for `sandbox_allow_network`, `sandbox_allow_child_processes`, and broad `sandbox_read_paths`/`sandbox_write_paths`; non-existent external plugin paths are rejected at descriptor parse time (QA-PLUGIN-002).

## Bugs

### gate
- Fixed reversed attack/release semantics (attack now controls opening speed, release controls closing speed)
- Fixed linked stereo `is_open` cache always returning true (now checks only envelope[0] in linked mode)

### transient-shaper
- Fixed `Sensitivity` parameter being a no-op (now applies sensitivity only to transient/attack envelope)
- Fixed `reset()` not resetting smoothers
- Fixed output gain applied to wet path only instead of final mixed result

### xtc
- Fixed missing `#[test]` on asymmetric spectral normalization test
- Fixed Brown-Duda `alpha_min` formula
- Fixed room reflection amplitude to use sqrt(energy absorption) for pressure

### downmix
- Fixed WOLA COLA violation (changed from Hann² at 75% overlap to sqrt(Hann) window with 50% overlap)
- Fixed Lt/Rt doc comment sign error

### linear-phase-eq
- Fixed DC magnitude hardcoded to 0 dB (now sums actual biquad responses at 1 Hz)
- Fixed Lowpass/Highpass bands with 0 dB gain being silently skipped in FIR design

### resampler
- Fixed `latency_samples()` to use rubato's `output_delay()` instead of `sinc_len/2` heuristic
- Added `flush()` to prevent last partial chunk being discarded
- Fixed `rebuild_resampler()` to reuse buffers instead of allocating

### stereo-imager
- Fixed unsmoothed crossover frequency changes causing clicks (now uses `LogSmoother` with 50 ms)
- Removed real-time heap allocation from `set_parameter()` (removed `cached_parameters` Vec)

### speech-denoiser
- Fixed dynamic latency on bypass (now constant `RNNOISE_FRAME_SIZE` latency)
- Added 48 kHz sample rate validation
- Added 480-frame block size validation
- Fixed stereo to process mid channel instead of independent per-channel (preserves stereo imaging)

### limiter
- Fixed catastrophic O(lookahead_len × channels) per-sample feed-forward scan → amortized O(1) running max via `lookahead_peaks` Vec
- Fixed 32-channel hard cap → dynamic `ch_peaks` Vec
- Fixed ISP correction decay operating in wrong domain (dB vs linear)

### de-esser
- Fixed split-band crossover order from 1 (6 dB/oct) to 4 (24 dB/oct LR4)

### mono-to-stereo
- Fixed `decor_low_hz`/`decor_high_hz` hardcoded to 300/2000 Hz (now uses parameters)
- Removed dead `enable_comp_eq`/`comp_eq_depth_db` parameters from UI and struct

### pnd
- Fixed circular buffer read bug in `current_drift_estimate()` (reads linear after wrap, now correctly copies in wrap order)

### multiband-compressor
- **CRITICAL**: Fixed real-time heap allocation in `process_in_place` (removed `Vec::resize`, now returns error for blocks > 4096 frames)
- Fixed `num_bands` truncation instead of rounding (now uses `value.round() as usize`)
- Fixed inconsistent `band_levels_db` defaults (`-120.0` everywhere)
- Fixed lookahead clamp inconsistency between constructor and `set_parameter`
- Fixed `reset()` not resetting sidechain tilt biquad states
- Fixed `set_parameter(num_bands)` not calling `rebuild_sidechain_tilt()`

### multiband-expander
- **CRITICAL**: Fixed spectral-mode OLA not COLA (changed from Hann at 75% overlap to Hann at 50% overlap)
- Fixed spectral-mode magnitude normalization ~6 dB offset (now uses actual window DC gain)
- Fixed spectral-mode latency over-reported (now returns `fft_size - hop_size` = 512)
- Fixed spectral-mode OLA ring clear size (clears `fft_size` instead of `hop_size`)
- Fixed `MAX_MB_BANDS` hard-coded array panic for `num_bands > 5` (now uses `Vec`)
- Fixed `initialize()` not calling `reset()`

### dynamic-eq
- **HIGH**: Fixed sidechain detector reading from in-place buffer (Band N saw prior bands' EQ output; now reads from `dry_buf`)
- Removed real-time heap allocation from `process_in_place` (`ensure_dry_buf` replaced with max block size check)

### delay
- Fixed block-constant delay smoother causing zipper/doppler glitches (now advances per-sample with `advance()`)
- Fixed LFO phase wrapping to use `fract()` instead of single subtraction
- Fixed `max_samples` to next power-of-two for fast bitwise masking
- Replaced modulo operations with bitwise AND for delay line indexing

### crossover
- Fixed block-based frequency smoothing causing zipper noise (now updates every 16 samples)
- Fixed frequency can exceed Nyquist after `initialize()` with low sample rates (now clamps to `sample_rate * 0.5 * 0.99`)

### spectral-compressor
- **CRITICAL**: Fixed magnitude normalization off by 6 dB (Hann window coherent gain not compensated; now scales non-DC/non-Nyquist bins by 2.0)
- Fixed attack/release coefficients computed without zero-guard (now clamps to 0.01 ms min)

### saturation
- **CRITICAL**: Fixed block-constant drive/mix/output-gain smoothers (now uses per-sample linear ramps)
- **CRITICAL**: Fixed Tube-mode ADAA using wrong nonlinearity when tone ≠ 1.0 (now falls back to direct tube() when tone differs)

### crossfeed
- **CRITICAL**: Fixed Meier filters never updated for actual sample rate (added to `update_filters()`)
- **CRITICAL**: Fixed asymmetric ITD modeled symmetrically (now computes differential L/R delays from yaw)
- Fixed yaw smoother advancing 1 step per block instead of `next_n(nf)`

### declick (plugins-denoiser transient module)
- Fixed envelope not adapting during suppression (now updates with allowed delta)
- Fixed exact equality check on floating-point envelope (now uses epsilon)
- Fixed `reset()` initializing envelope to 0.0 (now uses 1e-6)

### denoiser
- Fixed harmonic/percussive mode hardcoding 0.5 gain target for transients (now blends toward 1.0 to preserve transients)
- Fixed multi-resolution temporal smoothing applied twice (removed smoothing from small-FFT path; large-FFT path handles it)
