# 0.5.11

## Fixes (2026-08-12 review closure)

- Keep the RNNoise model warm during bypass and crossfade over 480 samples on a
  latency-aligned dry path, preventing stale-state re-entry and hard switches.
- Restrict the supported layout contract to mono/stereo and enforce it in both
  factories, the catalog, and the Audio Unit format negotiation path.
- Clamp finite input to the RNNoise model domain, remove unused reduction-meter
  work, and update cached parameter metadata without callback-time allocation.
- Move reusable FFT, pitch, analysis, synthesis, and RNN workspaces from the
  per-frame stack into initialized state while preserving reference output.
- Add cold-callback allocation, 64 KiB stack, bypass continuity, non-finite
  recovery, strict schema, invalid-layout, variable-block, and deadline tests.

# 0.5.10

## Fixes (2026-08-12 review follow-up)

- Update the all-plugin block-size conformance matrix to exercise the new
  arbitrary-callback FIFO contract instead of treating non-480-frame blocks
  as unsupported.

# 0.5.9

Linked-stereo RNNoise gain now transitions between detector decisions over
model frames. This bounds cancellation-prone stereo amplitude modulation while
retaining one common gain and preserving the stereo image.

# 0.5.8

Fix linked stereo cancellation: anti-phase, hard-panned, and unequal-level
signals no longer collapse in the mono detector or acquire unstable channel
gain differences. Regression coverage now verifies image and level-ratio
preservation.

# 0.5.7

Bug fixes from code review (2026-08-12):

- Accept arbitrary host callback sizes by framing input internally and always
  returning the requested frame count with a constant 480-sample startup delay.
- Preserve the first processed frame instead of deleting it at startup; bypass
  is now timeline-aligned with enabled processing.
- Sanitize non-finite input and reject buffer-size multiplication overflow.
- Prepare RNNoise shared FFT resources during initialization and use a stable
  frame-level linked-stereo gain rather than sample-wise cancellation-prone
  division.

# 0.5.6

Bug fixes from code review (2026-05-16):

- Delegated to `plugins-denoiser`: RNNoise reduction metering now averages input/output power
  across all processed channels instead of reporting channel 0 only.

# 0.5.5

Bug fixes from code review (2026-05-11):

- **Fixed (1.1 critical):** `latency_samples()` now always returns 480 regardless of the `enabled` flag. Previously it returned 0 when disabled, causing phase cancellation and comb-filtering in parallel-processing chains.
- **Fixed (1.3 critical):** `initialize()` now returns `Err` for any sample rate other than 48000 Hz. RNNoise band edges and FFT sizes are hard-coded for 48 kHz; running at other rates silently corrupts frequency response.
- **Fixed (1.2 critical):** `process_in_place()` now returns `Err` if `num_frames` is not a multiple of 480. Previously, tail samples of every buffer were zeroed when the block size was not a multiple of 480, producing periodic audible dropouts.
- **Fixed (2.3 medium):** `process_in_place()` now validates that `buffer.len() >= num_frames * channels` before accessing any index, preventing a panic on malformed host calls.

Delegated to `plugins-denoiser` 0.5.5 (same release):
- First-frame fade-in discard (1.4 major).
- Pre-allocated scratch buffers instead of stack arrays in the hot loop (3.1 medium).

Deferred / noted:
- **1.5 (major — stereo image):** Independent per-channel processing is a known limitation of the RNNoise mono model. A proper fix (downmix-to-mono → process → apply gains) is a cross-crate design change deferred to a future release.
- **2.1 (medium — ring buffer pointer overflow):** Pointers now use modulo indexing on every read/write; absolute counters remain `usize` (no 64-bit overflow risk in practice). Full wrap-on-advance can be done when the ring size is fixed at compile time.
- **3.2 (major — alloc in reset):** `nnnoiseless::DenoiseState` does not expose a `clear()` method. `reset()` still heap-allocates fresh states. This is acceptable for transport-stop/start events but must not be called per-callback.
- **2.2 (major — max_in_place_frames misleading):** `max_in_place_frames()` removed from the public path; block-size validation now uses the correct `% 480 == 0` check.

# 0.5.4

- Initial release. Split out of `sotf-plugin-denoiser` into a dedicated voice denoiser using the RNNoise (`nnnoiseless`) backend via the shared `plugins-denoiser` crate.
- Single parameter: `enabled`.
- Reports its actual `latency_samples()` so the host can compensate.
