# 0.5.13

## Robustness

- Adopt the host's strict realtime block contract through the parametric
  adapter: exact buffer lengths and finite input are validated before Dither's
  RNG or error-feedback history advances.
- Add a public-adapter regression proving malformed and non-finite blocks are
  transactional and the next valid TPDF block remains byte-identical to a
  fresh instance.

# 0.5.12

## Fixes

- Make the Wannamaker third-order F-weighted error-feedback taps sample-rate
  aware. At and above the 44.1 kHz reference rate, fractional error-history
  delays scale in seconds so the noise-transfer curve remains anchored to
  absolute frequency; lower rates retain the normalized curve because its
  ultrasonic destination band is not representable.
- Add an averaged Hann-windowed FFT regression proving that shaping reduces
  quantization-error power from 200 Hz to 8 kHz and moves it into 16–23 kHz.
- Register Dither in the canonical plugin catalog and facade factory, including
  its parameter schema, channel-preserving contract, and serialized settings.
- Use wrapping arithmetic when deriving per-channel RNG seeds, preventing debug
  overflow panics at the catalog's 4/6/8/12-channel layouts.

# 0.5.11

## Fixes

- Saturate rounded/truncated quantization codes to the signed PCM range
  (`-2^(N-1)` through `2^(N-1)-1`) before converting back to normalized
  floating point. This prevents `+1.0` and noise-shaping overshoot from
  emitting an unrepresentable positive code, and feeds the emitted saturated
  code into the shaping error state.
- Added endpoint, near-full-scale, and shaping-overshoot regression coverage
  at 16-, 20-, and 24-bit depths.

# 0.5.10

## Fixes

- TPDF now uses two independent uniform random values per output sample, removing
  the prior lag-1 anticorrelation and making the first sample genuinely TPDF.
- Noise-shaping feedback stores `quantized - dithered`, so explicit dither is
  excluded from the error-shaper state as documented.
- Negative bit-depth and dither-type choices clamp to the minimum option rather
  than wrapping through `usize` and selecting the maximum.
- Reset now restores deterministic per-channel RNG seeds as well as shaping state.

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
