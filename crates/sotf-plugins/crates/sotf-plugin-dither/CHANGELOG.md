# 0.5.9

## Fixes

- **`random_f32` comment corrected** (`src/lib.rs`): the code comment claimed
  the output range was `[-0.5, 0.5)` (half-open), but because `u32::MAX as f32`
  rounds up to `2^32` in IEEE 754, the actual range is the closed interval
  `[-0.5, 0.5]`.  The implementation is acoustically correct (TPDF dither works
  with a closed interval); only the comment was wrong.  Added
  `test_random_f32_boundary_precision` to document and pin this behaviour.

- **Dither mode options and TPDF RNG cost reduced** (`src/lib.rs`): added a third
  `dither_type` mode, **`Truncate`** (index 2), implemented as zero-crossing
  quantization with `trunc()` to avoid rounding distortion when you explicitly want
  passthrough-style quantization.  Also switched TPDF generation from two RNG calls
  per sample (`r1 - r2`) to a one-call form `R[n] - R[n-1]` by tracking per-channel
  previous random state, which halves the PRNG calls in the hot path.
  Added regression coverage:
  `test_tpdf_dither_uses_single_rng_sample` and
  `test_truncate_mode_quantizes_without_rounding`.

- **Noise-shaping feedback now excludes explicit dither term** (`src/lib.rs`):
  when `noise_shaping` is enabled, feedback now stores `quantized - shaped`
  instead of `quantized - dithered`, preventing the added dither from dominating
  the shaper and preserving shaping effect at higher bit depths. Added
  regression coverage: `test_noise_shaping_feedback_excludes_dither_term`.

## Deferred / acknowledged (code-review items)

- **Item 1 — TPDF amplitude at 24-bit** (Low): no fix needed; reviewer confirmed
  the implementation is correct.
- **Item 2 — noise shaping feedback includes dither** (Low): addressed by
  excluding explicit dither from feedback (`quantized - shaped`) and added
  coverage.
- **Item 4 — "None" mode still quantizes** (None / expected behaviour):
  previously index 1 behaved as round-only quantization; index 2 now exposes
  an explicit `Truncate` mode. The existing round-only option remains available
  as **`None (round)`**.
- **Item 5 — two RNG calls per sample** (Low): 96 k xorshift64 calls/s at
  stereo 48 kHz is negligible on modern CPUs; fixed with one-call TPDF generation.
- **Item 6 — `flush_denormals_inplace` after quantization** (None): quantized
  output is always a multiple of `inv_scale`, well above the f32 denormal range.
  The call is redundant but harmless; removing it is a cosmetic change out of
  scope for this patch.

# 0.5.8

## New

- Added missing qa_*.rs files for some plugins
- Added a dithering plugin (TPDF dither + F-weighted noise shaping)

## Changes

- Next iteration on UI and testing for plugins this time with native look&feel
