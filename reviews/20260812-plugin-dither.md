# Dither plugin code review — 2026-08-12

## Remediation status

Follow-up in 0.5.12 closes the remaining review findings:

- F-weighted error-feedback tap delays now scale from their 44.1 kHz reference
  in seconds at higher sample rates, preserving the absolute-frequency
  noise-transfer curve. Rates below 44.1 kHz retain the normalized curve because
  the reference ultrasonic destination band is above Nyquist. A response
  regression compares 44.1 and 88.2 kHz at five fixed frequencies.
- The canonical catalog and facade factory now expose and construct Dither from
  serialized parameters with an explicit channel-preserving, zero-latency
  contract. A factory/catalog regression covers schema, identity, channel count,
  sample rate initialization, and parameter restoration. Catalog-wide channel
  probing additionally exposed and fixed debug overflow in per-channel RNG seed
  derivation for widths above stereo.
- The former total-error smoke check is supplemented by a deterministic,
  averaged Hann-windowed FFT/PSD regression. It requires a measured reduction
  from 200 Hz–8 kHz and a measured increase from 16–23 kHz.

Remediated in 0.5.11: independent two-uniform TPDF generation, quantizer-input
residual feedback that excludes explicit dither, signed-domain clamping for both
choice parameters, deterministic reset of stochastic state, and signed-PCM
positive-endpoint saturation before scaling. Regression tests measure TPDF
mean/variance and lag-1 correlation, pin the feedback algebra, cover negative and
positive choice bounds, verify reset restarts the RNG sequence, and cover endpoint,
near-full-scale, and shaper-overshoot behavior at all supported bit depths.

The repository-wide non-finite/buffer-length policy remains a cross-plugin
follow-up; it is not one of the Dither-specific shaping/factory defects above.

## Findings

### P1 — The claimed one-call TPDF sequence is temporally correlated and the first sample after reset is not TPDF

`next_tpdf` forms `R[n] - R[n-1]` and saves `R[n]` for the next output (`crates/sotf-plugins/crates/sotf-plugin-dither/src/lib/dither_plugin.rs:145-152`). Each output therefore shares one random variable with its neighbour: adjacent dither samples have negative covariance, so the dither is high-pass coloured rather than the independent TPDF described by the README. Immediately after construction/reset, `prev_random` is zero (`dither_plugin.rs:249-252`), so the first dither sample is only uniform RPDF. The unit test at `src/lib/tests.rs:246-290` locks in the implementation but does not test the PDF or autocorrelation.

This can change idle noise spectrum and the statistical conditions under which quantization error is decorrelated. It also composes unintentionally with the separately enabled F-weighted shaper. Use two independent uniforms per output, or a proven one-word TPDF generator whose two components do not overlap across samples. Add a long-run histogram/mean/variance test, lag-1 autocorrelation bound, quantization-error correlation test for low-level DC/sines, and a post-reset first-sample test.

### P1 — “Exclude explicit dither” stores the opposite residual

The processing path computes `dithered = shaped + dither`, quantizes it, then stores `quantized - shaped` (`dither_plugin.rs:284-305`). Algebraically that stored value includes the explicit dither contribution. The residual which excludes explicit dither is `quantized - dithered`. Both the comment and `test_noise_shaping_feedback_excludes_dither_term` at `src/lib/tests.rs:294-334` assert the reversed interpretation, and the changelog repeats it.

This feeds dither energy back through the error shaper and changes the intended NTF, especially on silence and low-level material. First specify whether this is subtractive-dither error feedback or conventional non-subtractive dither plus error feedback, then implement that topology explicitly. Validate it against an offline reference using PSD bands and input/error cross-correlation; do not use the present algebra-preserving regression as proof of perceptual shaping.

### P1 — Negative choice values select the maximum option instead of clamping to the minimum

`apply_values` casts signed values to `usize` before applying `.min(...)` for both `bit_depth` and `dither_type` (`dither_plugin.rs:194-213`). Thus `-1` becomes a huge unsigned value and is mapped to 24-bit or Truncate, respectively. This contradicts the method's stated clamp semantics (`dither_plugin.rs:225-230`) and makes malformed automation/preset values choose the opposite endpoint.

Clamp in the signed domain (`v.clamp(0, max as i32) as usize`) or use the host schema validator consistently. Extend `out_of_range_ints_are_clamped_not_rejected` (`tests/integration.rs:105-119`) with negative values for both choice parameters.

### P2 — Positive full scale is emitted as an unrepresentable signed-PCM code (fixed in 0.5.11)

The quantizer uses a scale of `2^(bits-1)` (`dither_plugin.rs:48-58`, `103-106`) and writes the rounded value without bounding it (`dither_plugin.rs:290-309`). At every supported depth, `+1.0` is therefore a possible output although signed N-bit PCM tops out at `1.0 - 1/2^(N-1)`. Dither/noise-shaping overshoot can also exceed the nominal range. A later float-to-integer conversion must then saturate, changing the quantizer result and its feedback error outside this plugin.

Remediated by clamping the rounded/truncated integer code to the signed PCM
range before scaling back to normalized float. The shaping residual is computed
from that emitted saturated value. The
`signed_pcm_endpoints_are_saturated_before_error_feedback` regression covers
`-1`, `+1`, near-full-scale rounding, and shaping overshoot at 16-, 20-, and
24-bit depths.

Define the float/PCM endpoint contract. If this plugin represents final integer quantization, clamp the integer code to `[-2^(N-1), 2^(N-1)-1]` before scaling back and feed the actually emitted code into the shaper. Test `-1`, `+1`, near-full-scale tones, and shaper overshoot at all depths.

### P2 — The “F-weighted” response is fixed while every sample rate is accepted (fixed in 0.5.12)

The feedback coefficients are hard-coded (`src/lib/misc.rs:1-7`), while `initialize` accepts and stores arbitrary sample rates without adapting or rejecting them (`dither_plugin.rs:233-246`). The perceptual frequency placement of a discrete-time NTF changes with sample rate, so a curve described as pushing energy above about 15 kHz at 48 kHz does not preserve that property at 44.1, 88.2, 96, or 192 kHz.

Document a supported rate if the coefficient set is rate-specific, or provide validated coefficient sets/design per rate and atomically reset state on a topology change. Add pole/NTF magnitude checks and band-energy tests at every supported sample rate.

### P2 — The facade exposes a Dither UI/schema but the canonical factory catalog cannot construct it (fixed in 0.5.12)

The facade exports the dither parameter schema and plugin (`crates/sotf-plugins/src/lib.rs:172-174`, `246`, `308`), and render-plan tests generate a Dither UI, but `PLUGIN_CATALOG` has no `dither` entry (`crates/sotf-plugins/src/factory/catalog.rs:419-1307`). Consequently catalog-driven pickers and `create_plugin` do not expose a built-in that the crate otherwise presents as supported.

Add a Dither catalog row and factory construction arm with channel, zero-latency, parameter, preset, and allocation evidence, or explicitly mark the crate as intentionally non-factory and remove misleading generic UI exposure. Add a catalog/factory coverage test derived from exported built-ins so this omission cannot recur.

### P3 — The principal noise-shaping test does not measure the property named in the test (fixed in 0.5.12)

`test_noise_shaping_reduces_audible_noise` says it checks a low-frequency band, but computes only total time-domain squared error and merely asserts both totals are finite/non-zero (`src/lib/tests.rs:64-137`). It can pass if shaping is disabled, reversed, unstable-but-finite, or spectrally worse.

Replace it with deterministic FFT/PSD comparisons against an offline NTF reference: assert reduced in-band power, bounded ultrasonic/total power, finite peak level, and consistent results across block boundaries and rates. Retain the existing basic finiteness check under a name that matches what it proves.

### P3 — Reset does not reset the complete stochastic state

`reset` clears `prev_random` and the shaping history but leaves `rng_state` advanced (`dither_plugin.rs:249-256`). This produces a discontinuous first-sample distribution after reset and prevents deterministic restart even though construction uses deterministic seeds. The existing reset test disables both dither and shaping, so it cannot observe either state (`tests/integration.rs:208-235`).

Define reset semantics. For deterministic offline renders/tests, restore the per-channel seeds as well as the previous sample; if continued randomness is intended, preserve both RNG and `prev_random` so reset does not splice in an RPDF sample. Test the chosen streaming contract.

## Algorithm and realtime assessment

The core process loop is bounded O(frames × channels), uses preallocated per-channel state, performs no locking or logging, returns `context.num_frames`, and does not allocate in the audio path (`dither_plugin.rs:258-315`). Parameter/schema operations do allocate (`dither_plugin.rs:109-126`, `173-191`, `221`) but are outside normal sample processing; the host must keep them off the realtime thread. `flush_denormals_inplace` after integer-grid quantization is redundant work, and calling `enable_ftz_daz` every block is avoidable if the host establishes that thread mode, but neither is a leading cost relative to correctness above.

Channel state is independent and preallocated. The plugin has zero algorithmic latency and preserves interleaved channel count. There is no explicit bypass parameter. Buffer length is assumed to be at least `num_frames * channels`, consistent with the host trait contract but not defended locally. Non-finite inputs remain non-finite; the repository-wide robustness policy should decide whether the host sanitizes them or each nonlinear plugin must do so.

## Scope reviewed

Read all plugin-owned documentation and code: `AGENTS.md`, `README.md`, `CHANGELOG.md`, `Cargo.toml`, `src/lib.rs`, every file under `src/lib/`, `src/params.rs`, `tests/integration.rs`, and `bin/qa_dither.rs`. Also checked facade exports, parameter/layout snapshots, factory/catalog exposure, host `ParametricInPlacePlugin` expectations surfaced by the repository checklist, and TokenSave test-risk/panic-site results. No source-code changes were made.

## Existing strengths

- The implementation centralizes parameter defaults/UI metadata and wires cached parameters, setters, getters/current values, and serde defaults.
- The hot loop is allocation-free with per-channel state prepared during construction/initialization.
- Tests cover parameter round trips, grid quantization, channel independence, reset plumbing, supported bit depths, and error paths; the weaknesses above are mainly in the statistical/audio assertions.
- The plugin correctly reports zero latency and returns the requested frame count.

## Suggested verification after fixes

```bash
cargo test -p sotf-plugin-dither
cargo test -p sotf-plugins --test all_plugins_dsp_matrix
cargo clippy -p sotf-plugin-dither -- -W warnings
cargo check -p sotf-plugins
```
