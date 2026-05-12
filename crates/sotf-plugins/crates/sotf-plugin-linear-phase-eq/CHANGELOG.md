# 0.5.3

## Fixes

- **Bug #1 (🔴 critical): DC magnitude was hardcoded to 0 dB** (`src/lib.rs:348`).
  The `rebuild_fir` DC point was `magnitudes_db.push(0.0)` regardless of filter
  shape. Lowshelf cuts and highpass filters now correctly attenuate the DC region
  because the DC gain is computed by summing `band.biquad.log_result(1.0)` over
  all active bands.

- **Bug #2 (🔴 critical): Lowpass/Highpass bands were silently skipped** (`src/lib.rs:361`).
  The `band.gain_db.abs() > 1e-6` guard was applied to every filter type.
  Since lowpass/highpass always have `gain_db == 0`, every LP/HP band was treated
  as flat and omitted from the FIR design, producing an all-pass FIR. Fixed by
  matching on `BiquadFilterType::Lowpass | BiquadFilterType::Highpass` to always
  include those types; the gain guard is retained only for Peak, Shelf, etc.

- **Bug #3 (🟠 high): Insufficient frequency sampling for long FIR lengths** (`src/lib.rs:342`).
  `num_points` was a fixed 4096 regardless of FIR tap count. For an 8192-tap FIR
  this left high-Q narrow peaks undersampled. `num_points` now scales as
  `MAG_RESPONSE_POINTS.max(fir_length * 2).next_power_of_two()`.

## Tests added

- `test_highpass_attenuates_below_cutoff` — regression for bug #2.
- `test_lowshelf_cut_attenuates_low_frequencies` — regression for bug #1.

## Deferred

- **#4 (🟠): Overlap-add buffer sized to `fft_size` instead of `fir_len - 1`.**
  The current logic is correct (no out-of-bounds access was observed) but complex.
  Refactoring requires careful regression testing of the overlap-add path and is
  deferred to avoid scope creep. Ticket recommended before next major release.

- **#5 (🟡): Auto-gain normalizes by DC gain, not perceived loudness.**
  This is documented behavior: auto-gain targets DC unity. A treble boost with
  auto-gain enabled will reduce DC gain to 0 dB, which may partially counteract
  the boost at high frequencies. Documented in code comments; the trade-off is
  intentional (predictable loudness reference point).

- **#6 (🟡): FFT per channel without SIMD batching.**
  Acceptable for stereo; deferred for multi-channel optimization work.

- **#7 (🟡): `rebuild_fir` allocates `freqs`/`magnitudes_db` vectors.**
  Only called on parameter change, not in the audio thread. Deferred.

---

# 0.5.2

## Fixes

- Process blocks larger than the FFT size by chunking them through the overlap-add path.
- Avoid silently passing oversized blocks through dry while still reporting FIR latency.
- Add regression coverage that verifies large blocks are processed.
