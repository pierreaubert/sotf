# 0.5.9

## Fixes

- **`random_f32` comment corrected** (`src/lib.rs`): the code comment claimed
  the output range was `[-0.5, 0.5)` (half-open), but because `u32::MAX as f32`
  rounds up to `2^32` in IEEE 754, the actual range is the closed interval
  `[-0.5, 0.5]`.  The implementation is acoustically correct (TPDF dither works
  with a closed interval); only the comment was wrong.  Added
  `test_random_f32_boundary_precision` to document and pin this behaviour.

## Deferred / acknowledged (code-review items)

- **Item 1 — TPDF amplitude at 24-bit** (Low): no fix needed; reviewer confirmed
  the implementation is correct.
- **Item 2 — noise shaping feedback includes dither** (Low): the current
  non-subtractive Wannamaker formulation is mathematically valid.  Switching to
  subtractive dither or computing feedback as `quantized - shaped` would be a
  cross-architecture change; deferred.
- **Item 4 — "None" mode still quantizes** (None / expected behaviour): the
  parameter name is accurate.  Renaming or adding a passthrough mode is
  deferred as a feature request.
- **Item 5 — two RNG calls per sample** (Low): 96 k xorshift64 calls/s at
  stereo 48 kHz is negligible on modern CPUs.  Block-dither optimisation
  deferred.
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
