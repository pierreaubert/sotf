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
