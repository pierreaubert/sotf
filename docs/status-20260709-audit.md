# DSP status report audit — 2026-07-09

This audits every finding in `status-20260709.md` against the active code at
`f8ca54fee`. A finding is treated as a bug only when the current product has a
violated behavioral contract; optional quality changes and inactive code are
kept separate.

Result: 13 items exposed active correctness bugs and are fixed below; 17 were
not bugs as stated (including quality/model choices and one stale finding), and
2 referred only to an inactive renderer module.

| Item | Verdict | Evidence / action |
| --- | --- | --- |
| 1.1 DSD anti-aliasing | Not a bug as stated | The 64-sample population count is a first-order CIC/boxcar anti-alias filter, not an unfiltered decimator. Better DSD stop-band rejection is a fidelity improvement without a stated acceptance threshold. |
| 1.2 Offline output rate | Inactive code | `offline_renderer.rs` is not declared by `lib.rs`; neither this renderer nor its tests compile into `sotf-engine`. No dead-code edit was made. |
| 1.3 Offline dither | Inactive code | Same inactive module as 1.2. Dither would also be an output-quality policy, not a file-format correctness requirement. |
| 1.4 Expander Hann² COLA | **Bug — fixed** | RED test measured the overlap product varying from 0.5 to 1.0. Spectral mode now uses 75% overlap and `1 / (1.5 N)` normalization; the window-product sum is constant. The per-hop band snapshot is preallocated, and streamed timing is fixed at one FFT window for every tested host block size. |
| 1.5 Convolution latency | **Bug — fixed** | RED test observed 0 for the 1024-sample UPC path. Signal-level delta tests now match UPC, NUPC, and zero-latency-head reporting. A configured time-domain head reports zero onset latency even before its runtime IR is loaded, keeping the host's build-time latency cache stable; otherwise the NUPC dry path is delayed to the wet path before partial mixing. NUPC/head topology controls are rebuild-only, so runtime setters cannot desynchronize an existing engine from host PDC. |
| 1.6 FIR auto-gain at DC nulls | **Bug — fixed** | Both FIR plugins produced high-pass coefficient peaks around 7.75 million. Auto-gain preserves unity DC when meaningful, falls back to a stable Nyquist passband reference for DC-null filters, and skips normalization when both endpoints are ill-conditioned. Shelf, narrow low-pass, and high-pass tests verify unity reference gain and bounded coefficients. |
| 1.7 Denoiser latency | **Bug, but the report's diagnosis was wrong — fixed** | Streamed RED tests measured delay as `fft_size - host_block_size`, not `fft_size - hop_size`, so the old output schedule was block-size dependent. Explicit startup scheduling now gives the reported one-window delay (`fft_size`) for block sizes 128 through 2048. |
| 1.8 Upmixer latency | Not a bug | Streamed impulse tests show the explicit startup padding produces exactly `fft_size` samples of delay across block sizes 128 through 2048. Changing the report to `fft_size - hop_size` would under-compensate by one hop, so the original one-window report is retained. |
| 2.1 TDF-II interpolation state | Not proven as a bug | A coefficient-varying TDF-II is a valid time-varying realization, although it does not match DF-I state semantics. No response/stability contract in this repository proves the reported warning. |
| 2.2 Runtime direct-form toggle | **Bug — fixed** | RED test showed stale DF-I state reappearing after a TDF-II round trip. Runtime form changes now reset every section and cancel coefficient transitions. |
| 2.3 SVF high-order stages | **Bug — fixed** | SVF is documented as single-stage but accepted high-order bands silently. Switching to SVF with order >2, or raising order while in SVF mode, now returns an error. |
| 2.4 Multiband FIR delays | **Bug — fixed** | RED 3-way reconstruction missed by about 0.09. The plugin now delays early tree branches to the final branch's cumulative group delay; summed bands reconstruct the delayed input. |
| 2.5 Crossover smoother mapping | **Bug — fixed** | RED tests showed sorted frequencies while parameter smoothers retained the old physical indices. Sorting now rebinds smoothers and reinitializes the active crossover. |
| 2.6 Mute/solo ramp endpoint | **Bug — fixed** | RED test showed the last gain at 0.997088 while smoother state was 0.996672. The emitted ramp now advances from `1/n` through `n/n`. |
| 2.7 cpal integer dither | Quality policy | Direct integer conversion is valid. Dither can improve low-level subjective quality but is not required by the cpal/sample-format contract. |
| 2.8 Crossfade sample-rate basis | Not a bug | `input_frames / input_rate` and `output_frames / output_rate` represent the same elapsed block time for a resampler. The reported mismatch does not change the 50 ms duration except for bounded frame rounding. |
| 2.9 Crossfade latency alignment | **Bug — fixed** | Timing-compatible chains crossfade directly. Chains with different latency or output rate now transition old → silence → new with a sample-linear envelope, avoiding both comb filtering and a hard-swap pop; different output frame counts are duration-mapped during the fade, and each host gets a destination sized for its own rate. Compatibility compares output rate and normalized latency, not raw sample counts alone. Bidirectional 48↔96 kHz transition tests bound adjacent-sample jumps. |
| 2.10 Waveform anti-aliasing | Not a bug | The code computes windowed RMS energy for a display envelope; it does not claim to produce a downsampled audio waveform. Pre-filtering would discard high-frequency energy the meter is meant to show. |
| 2.11 Spectrum tone calibration | **Bug — fixed** | A bin-centered full-scale tone measured -6.02 dBFS, not the report's -3 dB. Periodic-Hann coherent-gain correction makes interior tones read 0 dBFS, while Nyquist keeps the real-FFT endpoint scale. The public cache now uses signed maximum dB, so tone and silence peaks report 0 and -100 dBFS rather than +100. |
| 2.12 Symmetric Hann | Not a bug | A periodic Hann is the appropriate DFT-analysis window. Its final sample is near zero and joins the first zero sample periodically; changing to symmetric Hann is not a leakage fix. |
| 3.1 Block-rate dynamics smoothing | Quality/automation tradeoff | Several paths intentionally use one control value or a linear ramp per block. No sample-accurate automation contract is violated; per-sample smoothing would trade CPU for finer control. |
| 3.2 Limiter smoothing domain | Not a bug | Linear-amplitude smoothing is monotonic, bounded, and intentional. Smoothing in dB is a different transition law, not a correctness fix. |
| 3.3 Denoiser reduction formula | Model tuning | The gain tends to unity as SNR rises and is bounded by the configured floor. The parameter's perceptual tuning may be debatable, but the report supplies no violated numeric contract. |
| 3.4 High-order peaking design | Documented design | The EQ crate explicitly documents its Q-scaled cascade. There is no standard unique “high-order peak” response against which this implementation is incorrect. |
| 3.5 Sparse FIR target grid | Stale finding | Current code scales the response grid to at least twice the FIR length and a power of two, rather than using the claimed fixed sparse grid. |
| 3.6 FIR dB composition | Mathematically incorrect finding | Cascaded transfer functions multiply, so their magnitudes multiply and dB magnitudes add exactly; phase does not create cancellation between serial sections. |
| 3.7 Minimum-phase latency | Intentional contract | Existing tests explicitly define minimum-phase FIR latency as zero because no single scalar represents its frequency-dependent group delay. |
| 3.8 Recording rate mismatch | **Bug — fixed** | References are now resampled to the negotiated input rate before analysis, and progress uses that actual capture rate. Offline reference conversion drains the interpolation tail and removes rubato's output delay; impulse alignment and late-signal tests cover content rather than length alone. The engine already resamples playback to the selected output rate. |
| 3.9 Timeline time stretch | **Bug plus quality issue — fixed in scope** | RED test showed ratio 2 reading `0,1,2...` within a block. Rendering now maps every output frame to a continuous fractional source position and interpolates it, including symmetric clamping for fractional reverse playback. Timeline build preallocates bounded source-span scratch so this path does not grow a `Vec` on the audio thread. A higher-order bandlimited interpolator remains an optional quality upgrade. |
| 3.10 Probe crest factor | Quality improvement | Seeded random phase is deterministic and peak-normalized as designed. Schroeder phase would improve excitation energy but no current correctness contract is violated. |
| 3.11 Circular Hilbert envelope | Quality/boundary improvement | FFT Hilbert periodicity is documented behavior in the external `math-dsp` dependency. Padding can reduce edge artifacts but is not a local correctness fix. |
| 3.12 Missing Orfanidis tests | Test gap, not a bug | The finding identifies absent upstream `math-iir-fir` response tests, not incorrect active behavior in this repository. |

## TDD evidence

Before the fixes, focused regression tests failed for every item marked as a
fixed bug above. Representative RED observations were: Hann² range `0.5..1`,
FIR coefficient peak `7752437.5`, shelf DC gain `0.2527`, narrow-low-pass DC
gain `10.3957`, cached spectrum peak `+100 dBFS`, convolution head latency
`1024` versus an observed delta at zero, denoiser delay `1920` versus a
reported `1024` for 128-frame blocks, and timeline ratio-2 output `0,1,2...`
versus `0,2,4...`. Each focused test passed after its corresponding
implementation change.
