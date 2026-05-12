# 0.5.5

Bug fixes from code review (2026-05-11):

- **Fixed (1.4 major):** First processed 480-sample frame is now discarded to remove the fade-in artifact documented by `nnnoiseless`. This adds a fixed one-time 480-sample startup delay; subsequent frames are unaffected.
- **Fixed (3.1 medium):** Per-channel `input_buf` / `output_buf` scratch arrays are now pre-allocated in `initialize()` and stored in `scratch_input` / `scratch_output`. The audio callback no longer pushes 3.8 KB onto the stack on every 480-sample block.
- `initialize()` now returns `Result<(), String>` and rejects sample rates other than 48000 Hz with a descriptive error message.

Deferred / noted:
- **2.1 (medium — ring buffer pointer overflow):** Absolute write/read positions remain monotonically increasing `usize`; practical overflow would require ~years of continuous audio on a 64-bit system. Full modulo wrap-on-advance deferred.
- **3.2 (major — alloc in reset):** `nnnoiseless::DenoiseState` has no `clear()` method, so `reset()` must allocate. Documented in code; callers must not invoke `reset()` per-callback.
- **1.5 (major — stereo image):** Independent per-channel RNNoise processing is a known limitation. Mid/side or gain-application-only approach deferred.

# 0.5.4

- Initial release. Shared DSP building blocks extracted from `sotf-plugin-denoiser` so that the new dedicated declick, hiss reducer, and speech denoiser plugins can reuse the same primitives.
- Modules: `transient` (TransientSuppressor for click repair), `hiss` (HissReducer for stationary high-frequency noise), `rnnoise` (RnnoiseBackend wrapping `nnnoiseless`).
