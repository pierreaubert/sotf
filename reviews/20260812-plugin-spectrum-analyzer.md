# Spectrum Analyzer plugin review

Date: 2026-08-12
Scope: `SpectrumAnalyzerPlugin` DSP and analyzer cache, factory/catalog/parameter metadata, engine publication, GPUI presentation, AU/FFI and VST3/CLAP bridges, tests, QA, fuzzing, and benchmarks. Review only; no production code was changed.

Final verification (2026-08-12): every P0-P2 finding below is closed with a focused regression. The follow-up audit corrected two flaws in the initial remediation: sample-wise maximum-channel selection was nonlinear, and cloned nested cache buffers could allocate on the first FFT. Channel aggregation is now maximum per-channel FFT-line power, cache payloads are independently preallocated, reset has a third preallocated generation, band power is Hann-ENBW normalized, and the normal GPUI path shares `Arc<[f32]>` storage without copying. The analyzer intentionally performs at most one latest-window FFT per callback; this is the tested bounded-work/freshness contract rather than an overlap analyzer.

## Findings

### P0 — [Fixed] The advertised VST3/CLAP Spectrum Analyzer cannot instantiate

The NIH crate exports `SotfSpectrumAnalyzer` for both CLAP and VST3 (`crates/sotf-plugins/crates/plugins-nih/src/lib.rs:317-321`). Its initializer calls `plugins_bridge::create_plugin("SpectrumAnalyzer", ...)` and returns `false` on any factory error (`crates/sotf-plugins/crates/plugins-nih/src/wrapper.rs:105-131`). The bridge factory has no Spectrum Analyzer match arm and falls through to `Unknown plugin type` (`crates/sotf-plugins/crates/plugins-bridge/src/factory.rs:321-328`); it is absent from the advertised bridge catalog too (`crates/sotf-plugins/crates/plugins-bridge/src/factory.rs:331-374`). The FFI/AU creation path delegates to this same bridge (`crates/sotf-plugins/crates/plugins-ffi/src/plugin_factory.rs:8-17`). Consequently, the standalone plugin binaries may build and be discoverable but cannot initialize in a host.

Fix: add one canonical Spectrum Analyzer bridge registration, including aliases and initialization, and derive every exported-format catalog from that registry. Add a bridge test that creates every NIH/FFI-exported type, plus an actual CLAP/VST3/AU initialization smoke test. Specifically assert that `SpectrumAnalyzer` and `spectrum_analyzer` construct, initialize, process 4096 frames, and publish `SpectrumData`.

### P1 — [Fixed] Persisted tilt controls are complete no-ops

Engine settings persist `tilt_correction` and `tilt_reference` (`crates/sotf-engine/src/plugins/plugin_settings.rs:897-910`) and serialize both into factory JSON (`crates/sotf-engine/src/plugins/plugin_config_converter/effects.rs:195-220`). Static metadata advertises them as supported setup parameters (`crates/sotf-plugins/crates/sotf-host/src/param_specs/param_spec.rs:523-564`), and the GPUI exposes working selectors (`crates/app-gpui/components/plugins/ui_spectrum.rs:245-355`). But `SpectrumConfig` contains only bins, frequency bounds, and smoothing (`crates/sotf-plugins/crates/sotf-host/src/analyzer_spectrum.rs:30-35`); serde silently ignores the extra JSON. Runtime parameters likewise expose only those four values (`crates/sotf-plugins/crates/sotf-host/src/analyzer_spectrum.rs:168-180,239-280`). Finally, rendering passes only magnitudes, range, and smoothing to `SpectrumElement` (`crates/app-gpui/components/plugins/ui_spectrum.rs:133-142`), whose API and paint path contain no tilt operation (`gpui-toolkit/crates/gpui-audio-kit/src/spectrum/spectrum_element.rs:9-69,122-207`). Changing either control rebuilds the chain but cannot change the display.

Fix: choose one owner for display correction. Prefer publishing uncorrected dBFS and applying `slope_db_per_octave * log2(f/reference)` in the view, so stored analyzer data remains physically meaningful. Remove tilt from DSP/factory metadata if it is view-only. Add screenshot/numeric tests for None, 3 dB/oct, 6 dB/oct, Pink, and every reference; assert a predictable low-to-high delta and that None is unchanged.

### P1 — [Fixed] Arithmetic channel averaging can report silence for loud audio

Stereo is reduced to `(L + R) / 2`, and all other layouts use the arithmetic mean (`crates/sotf-plugins/crates/sotf-host/src/analyzer_spectrum.rs:307-327`). A full-scale antiphase stereo tone therefore produces exact zero and a -100 dB display. Surround channels with different phase/content are attenuated according to layout and channel count, while LFE is weighted identically to full-range channels. This is not a safe default for a level-oriented analyzer.

Fix: make aggregation explicit and documented: per-channel traces, max channel power, or an energy-preserving RMS/power sum. If mono sum remains an option, label it and use layout-aware weights. Add tests for antiphase stereo, tone in one of N channels, uncorrelated multichannel noise, and disjoint tones on separate channels; results must not disappear or vary merely because unused channels were added.

### P1 — [Fixed] Public construction accepts malformed configurations that can create invalid data or unbounded allocation

`with_config` immediately computes logarithms and allocates `num_bins`-sized vectors without validation (`crates/sotf-plugins/crates/sotf-host/src/analyzer_spectrum.rs:99-165,183-188`). The main factory deserializes arbitrary JSON directly into this path (`crates/sotf-plugins/src/factory/create.rs:365-374`). Zero/huge `num_bins`, zero channels, non-positive/non-finite frequencies, and `max_freq <= min_freq` are accepted. Outcomes include NaN frequency centers/mappings, meaningless zero-channel processing, and attacker/user-controlled large allocations or OOM. `initialize(0)` is also accepted (`crates/sotf-plugins/crates/sotf-host/src/analyzer_spectrum.rs:283-287`). Runtime clamps do not protect construction-time JSON.

Fix: centralize `SpectrumConfig::validate(sample_rate, channels)` and reject non-finite/order-invalid bounds, bins outside the declared range, zero channels/rate, and max above Nyquist (or clamp only where the API explicitly promises it). Add direct and factory tests for every boundary, NaN/Inf, zero, reversed bounds, extremely large bins, and sample rates 8/32/44.1/48/96 kHz. No malformed input should panic, allocate proportionally before validation, or publish NaNs.

### P1 — [Fixed] The FIFO processes at most one FFT per callback, causing lag and sample loss

The ring holds four FFTs, all incoming frames are pushed, but only one 4096-sample chunk is consumed per `process` call (`crates/sotf-plugins/crates/sotf-host/src/analyzer_spectrum.rs:122-123,307-355`). An 8192-frame callback leaves a complete old frame queued; larger or repeated callbacks overflow the ring and drop samples (`crates/sotf-plugins/crates/sotf-host/src/analyzer_spectrum.rs:328-334`). The visualization can therefore lag by several windows, skip discontinuous time spans, and produce smoothing dependent on host callback size. Returning success hides the loss except for a rate-limited log.

Fix: define a bounded freshness policy. For a display analyzer, retain the latest 4096 samples and discard stale complete hops before FFT, or process all complete hops up to a documented CPU budget and expose dropped-hop counters. Avoid a producer/consumer FIFO if both ends live in the same plugin thread. Add 4095/4096/8192/16384/20000-frame and repeated-large-block tests that verify latest-tone freshness, bounded work, no out-of-bounds access, and explicit overflow diagnostics.

### P1 — [Fixed] Structural parameter updates allocate in the real-time processing path

Every setter first calls validation through the allocative `parameters()` clone (`crates/sotf-plugins/crates/sotf-host/src/analyzer_spectrum.rs:236-240`). Bins/range changes call `rebuild_config_dependent`, which allocates frequency and mapping vectors, may resize work buffers, clones frequencies, replaces magnitude storage, and rebuilds the parameter vector (`crates/sotf-plugins/crates/sotf-host/src/analyzer_spectrum.rs:191-213,248-271`). External wrappers make this worse: NIH synchronizes every parameter on every audio callback (`crates/sotf-plugins/crates/plugins-nih/src/wrapper.rs:145-150`), iterating a `HashMap`, cloning every ID, and calling every setter (`crates/sotf-plugins/crates/plugins-nih/src/params.rs:106-115`). Once the bridge is fixed, unchanged structural values still allocate through validation and cached-parameter reconstruction.

Fix: distinguish setup/structural configuration from RT automation, perform it off-thread, and atomically hand over a fully prepared state. Make unchanged setters return before rebuilding metadata; provide static/no-allocation validation; make wrappers synchronize only changed values/events. Add allocation-counter tests for unchanged parameter sync, smoothing automation, each structural change, and contention while a reader holds analyzer data.

### P1 — [Fixed] Cache shape changes can reintroduce allocation on the audio thread

`rebuild_config_dependent` attempts to replace cached arrays inside `RealTimeCache::update` (`crates/sotf-plugins/crates/sotf-host/src/analyzer_spectrum.rs:208-213`). `RealTimeCache` deliberately skips an update when its spare `Arc` is held (`crates/sotf-plugins/crates/sotf-host/src/analyzer.rs:38-65`). If that skip occurs while `num_bins` changes, the next successful process update calls `SpectrumData::update_magnitudes`; a length mismatch allocates a new vector (`crates/sotf-plugins/crates/sotf-host/src/analyzer.rs:357-367`) inside the audio callback (`crates/sotf-plugins/crates/sotf-host/src/analyzer_spectrum.rs:399-403`). The steady-state allocation test never changes shape and therefore cannot detect this.

Fix: build both cache buffers at the new shape off-thread and swap the complete cache/state, or defer activation until both are ready. Add a regression that holds each alternating cache `Arc`, changes 30→100 bins, processes frames under an allocation counter, and verifies consistent frequency/magnitude lengths with zero callback allocations.

### P2 — [Fixed] Reset leaves queued pre-reset audio in the analyzer

`reset` clears smoothed values and tries to clear the published cache, but does not drain/recreate the ring or clear pending FFT input (`crates/sotf-plugins/crates/sotf-host/src/analyzer_spectrum.rs:289-297`). A transport reset after 4095 loud samples leaves them queued; one silent sample completes that old window and republishes the pre-reset signal. If cache contention prevents the reset closure, even the displayed old values remain until a later FFT.

Fix: drain/reinitialize pending input and clear all analysis state under the reset contract; arrange an unconditional cache reset or generation switch. Test 4095 tone samples → reset → silence, reset while holding both cache generations, and repeated reset/process cycles.

### P2 — [Fixed] Display “bands” are maxima of FFT lines, not band level or PSD

Each logarithmic display bin takes the maximum dB value of any constituent FFT line (`crates/sotf-plugins/crates/sotf-host/src/analyzer_spectrum.rs:369-382`). This is reasonable only if the API explicitly promises a peak-line spectrum. It does not represent energy in a band: wide high-frequency bands and narrow low-frequency bands are incomparable; broadband noise depends on FFT resolution and extreme-value statistics; off-bin tones lose level; bins containing no FFT line remain at -100 dB. The metadata calls them “frequency bands” (`crates/sotf-plugins/crates/sotf-host/src/param_specs/param_spec.rs:527-530`).

Fix: specify the quantity. For band levels, sum linear power with correct one-sided/Hann ENBW normalization, then convert once to dB; for PSD, normalize by Hz; for peak-line mode, rename/document it. Add bin-centered and half-bin tones, white/pink noise, sample-rate/FFT-size invariance, single-line and empty-band tests against a reference implementation.

### P2 — [Fixed with bounded-window policy] Smoothing has no time-domain meaning and is duplicated by the UI

DSP smoothing is a per-FFT dB interpolation (`crates/sotf-plugins/crates/sotf-host/src/analyzer_spectrum.rs:385-391`). Its decay time changes with sample rate, callback size, backlog, and dropped frames. With non-overlapping 4096-sample windows, the nominal update rate is only 11.7 Hz at 48 kHz. GPUI then passes the same setting to `SpectrumElement` (`crates/app-gpui/components/plugins/ui_spectrum.rs:136-140`), but the element applies smoothing only when `previous_magnitudes` is supplied; this call does not supply it (`gpui-toolkit/crates/gpui-audio-kit/src/spectrum/spectrum_element.rs:42-50,143-155`). Thus the UI setting misleadingly suggests display smoothing while only the cadence-dependent DSP filter is active.

Fix: the normalized control now represents a 0–1000 ms time constant and uses the exact elapsed samples between bounded latest-window analyses. Hop remainder is retained across callbacks. The DSP is the sole smoothing owner; the latest-window policy deliberately avoids overlap and caps work at one FFT per channel per callback. A regression compares equal 1.024-second decay at 32/48/96 kHz with 64- and 8192-frame callbacks.

### P2 — [Fixed] Frequency bounds are not tied to Nyquist, and DC/empty-bin policy is implicit

The runtime maximum is clamped to a fixed 22050 Hz while `initialize` accepts arbitrary rates (`crates/sotf-plugins/crates/sotf-host/src/analyzer_spectrum.rs:262-287`). At 32 kHz, labels can extend to 20 kHz although no data exists above 16 kHz. The FFT loop always skips DC (`crates/sotf-plugins/crates/sotf-host/src/analyzer_spectrum.rs:371`), and low logarithmic display bins can contain no line because resolution is 11.72 Hz at 48 kHz; they remain indistinguishable from measured -100 dB silence.

Fix: validate/clamp the active range against `sample_rate/2`, publish the effective range, and explicitly represent unavailable/empty bins (for example NaN/validity mask rather than signal floor). Document DC handling. Test low sample rates, min frequency below the first non-DC line, max exactly/above Nyquist, and configurations with more display bins than mapped FFT lines.

### P2 — [Fixed] The process API panics on malformed buffers instead of returning an error

`output.copy_from_slice(input)` panics when lengths differ, then indexing trusts `context.num_frames * num_channels` (`crates/sotf-plugins/crates/sotf-host/src/analyzer_spectrum.rs:299-323`). The public `Plugin::process` contract returns `Result`, so a bad host/bridge call should not abort the process. Zero channels compounds this by making frame/layout validation impossible.

Fix: validate exact input/output sizes and nonzero channel count before copying/indexing, returning a descriptive error. Add undersized/oversized input, mismatched output, inconsistent context, and zero-channel property/fuzz tests.

### P2 — [Fixed] The GPUI copies every magnitude array on every render

The analyzer originally published magnitudes as an `Arc<Vec<f32>>`, but both plugin and phone views copied them into a new `Arc<[f32]>` before constructing the GPU element (`crates/app-gpui/components/plugins/ui_spectrum.rs:133-136,400-407`). At typical sizes this was small, but it was needless per-render allocation and obscured cache ownership/contention behavior.

Fix: `SpectrumData` now publishes `Arc<[f32]>`; the default uncorrected GPUI path returns the identical Arc, verified with `Arc::ptr_eq`. Tilt-corrected views allocate only when an actual correction must produce different values. The cold-allocation regression holds two published UI generations while resetting the analyzer.

## DSP and implementation strengths

- The periodic Hann is precomputed and appropriate for block FFT analysis (`analyzer_spectrum.rs:93-97`). Interior and DC/Nyquist amplitude scaling correctly accounts for one-sided real FFT output and Hann coherent gain (`analyzer_spectrum.rs:363-380`). Existing full-scale bin-centered and Nyquist tests validate this calibration.
- FFT plans, output, windowed input, window coefficients, display mapping, and magnitude workspaces are preallocated. Window multiplication uses the shared SIMD helper. The normal fixed-shape steady-state path passed the allocation test.
- The analyzer is bit-transparent, returns the host frame count, supports the compiled analyzer-tap path, rate-limits overflow/FFT errors, and uses a nonblocking cache design rather than locks on the processing thread.
- `fast_log10` is used only after a `1e-10` power floor. Its documented `fast_log2` error (~0.001, approximately 0.003 dB after `10*log10`) is adequate for a display. Non-finite input is sanitized before the FFT and a regression asserts that published data never contains NaN.

## Focused verification

Final focused commands passed:

- `cargo test -p sotf-host analyzer_spectrum --lib` — 17 passed.
- `cargo test -p sotf-host --test test_analyzer_plugins` — 16 passed.
- `cargo test -p sotf-plugins --test realtime_allocation_tests test_spectrum_analyzer` — 2 passed, including cold eight-channel FFT, allocation-free live smoothing, and reset while two cache generations are held.
- `cargo test -p plugins-bridge spectrum_analyzer_aliases_initialize_process_and_publish_data` — passed for canonical and snake-case aliases.
- `cargo run -p sotf-host --features qa --bin qa-host` — all QA passed; Spectrum Analyzer measured approximately 0.20% CPU for stereo.

The GPUI tilt/zero-copy test is source-complete but its package-level run is presently blocked by unrelated concurrent `sotf-player` initializers missing new AB Compare and Binaural fields; the compiler reached those unrelated files before the GPUI test binary. The broad advertised-factory matrix is likewise blocked by the unrelated newly advertised `dither` type not yet being added to that matrix; direct bridge construction and processing pass for both Spectrum Analyzer aliases.

## Coverage reviewed

Read the complete Spectrum Analyzer host implementation and inline tests; analyzer cache/data implementation and tests; host analyzer integration tests, QA/fuzzer, all-plugin DSP/channel/high-channel/parameter/factory matrices, allocation tests and benchmarks; main factory/catalog and param specs; engine settings/defaults/conversion/accessors, chain wiring, analyzer cache publication and FFT-chain test; GPUI plugin/fullscreen rendering, editing flow and e2e scenario; `gpui-audio-kit` SpectrumElement implementation/tests; FFI factory and parameter fallback; NIH wrapper, dynamic parameters, feature export; bridge factory/catalog/tests; and the underlying `fast_log10`, real-FFT/window/scaling path. No plugin source segment was intentionally skipped.
