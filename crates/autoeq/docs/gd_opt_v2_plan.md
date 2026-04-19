# Group Delay Optimisation v2 — Design Plan

**Status:** future work. The v1 implementation (`crates/autoeq/src/roomeq/group_delay.rs`) is being removed in the 2.0 simplification pass. This document captures what v2 needs to do differently and the measurement changes required to make it actually work.

**Author context:** v1 was removed because (a) per-speaker pairwise GD matching does not express what the listener hears at the sum, and (b) the phase data arriving at the optimiser was too noisy in the bass to derive a meaningful target.

---

## 1. Why v1 did not work

| Issue | Location | Effect |
|---|---|---|
| Raw forward-difference GD on log-spaced bins | `group_delay.rs::calculate_group_delay` | At 30 Hz, bin spacing ≈ 5 Hz and dφ jitter of 5° maps to ~45 ms of GD noise. |
| Phase defaulted to 0° when missing | `group_delay.rs::curve_to_complex` | Silent "successful" matching of zero-phase ghosts. |
| Search band hard-coded to `[min_freq, 200]` Hz | `optimize.rs:1716` | Wrong for DBA/cardioid (XO ≈ 40 Hz) and bookshelves (XO ≈ 120 Hz). |
| Greedy per-filter, never jointly refined | `optimize_ap_filters_n` | Freezes early mistakes. |
| Per-sub→per-main pairwise matching | `find_sub_main_pairings` | Ignores mutual cancellation in the sum. The listener hears `L + R + Sub`, not the pair. |
| No output metric | caller side | User cannot tell if GD-Opt helped. `best_error` logged at `debug!` only. |
| `target_ms` field read nowhere | `types/config.rs:1331` | Dead configuration knob. |

---

## 2. Recording changes (prerequisite)

GD-Opt v2 is blocked on getting trustworthy phase below the Schroeder frequency. These changes live in `crates/sotf-engine/src/signal_recorder.rs` and the recording wizard under `crates/app-gpui/components/room_eq/`.

### 2.1 Sweep parameters

Today the sweep is a generic log sine (`gen_log_sweep`) parameterised by `start_freq`, `end_freq`, `amp`, `duration`. Bass precision wants:

- **`start_freq` as low as the system can play cleanly.** Default: 10 Hz (not 20). The phase of the first measured bin drives the unwrap for everything above; starting at 20 Hz already inside many rooms' first modal region poisons the unwrap.
- **Duration auto-derived from octave range, not fixed seconds.** At least **3 seconds per octave below 100 Hz** to let modal energy settle before the sweep leaves the band. A 10 Hz→20 kHz sweep then needs ~15 s of bass content; today a 10 s sweep gives ~1 s below 100 Hz, which is why bass SNR collapses.
- **Pre- and post-silence windows.** Pre-roll silence long enough for HVAC/electronics noise estimation (2 s); post-roll long enough for the longest room decay (RT60 + 1 s). Both written to WAV so the decoder can estimate noise floor deterministically.
- **Deterministic level.** The current `amp` is open-ended. Cap bass energy to a target SPL (e.g. 85 dB @ 1 m equivalent) to avoid subwoofer driver excursion producing harmonic distortion that contaminates phase.

**Schema addition** (not an API change — new optional fields in `RecordingConfiguration`):

```rust
pub struct RecordingConfiguration {
    // ...
    pub bass_octave_duration_s: Option<f32>,   // default 3.0
    pub pre_silence_s: Option<f32>,            // default 2.0
    pub post_silence_s: Option<f32>,           // default = schroeder_rt60 + 1.0
    pub sweep_level_db_spl: Option<f32>,       // target SPL at listening position
}
```

### 2.2 Multi-sweep averaging below Schroeder

Single-sweep phase below 100 Hz is dominated by HVAC and electronic mains harmonics. Use **coherence averaging** over N ≥ 4 sweeps:

1. Record N consecutive sweeps back-to-back with the post-silence window in between.
2. Deconvolve each sweep independently to obtain N impulse responses `h_i(t)`.
3. Compute the per-bin complex average `H̄(f) = (1/N) Σ H_i(f)` and the magnitude-squared coherence `γ²(f) = |H̄|² / ⟨|H|²⟩`.
4. Export `coherence` alongside `spl` and `phase` in the CSV.

**Coherence is the confidence metric.** Below γ² = 0.9 the phase is untrustworthy and GD-Opt must refuse to correct.

### 2.3 Signal type: log sweep, not MLS

Log sine sweep is the chosen excitation. MLS was considered and rejected:

| Property | Log sweep | MLS |
|---|---|---|
| Bass energy per driver-excursion limit | Wins — slow sweep concentrates in-band. | Loses — flat power spectrum spreads energy. |
| Harmonic distortion handling | Wins — HD lands at negative times in deconvolved IR, gated off. | Loses — HD smears across the IR as correlated noise that corrupts phase. |
| Coherence estimation | Needs N sweeps (already doing this). | Native from a single recording. |
| Robustness to non-stationarity (HVAC) | Only the in-band frequencies at the moment of the glitch are lost. | Whole sequence ruined. |

Bass SNR is driver-excursion-limited, not mic-limited. The sweep's bass energy advantage dominates; multi-sweep averaging (§2.2) closes the coherence gap.

### 2.4 Min-phase / excess-phase decomposition at measurement time

Rather than ad-hoc time windowing, decompose each channel's complex response via Hilbert transform on log-magnitude: `H = H_min · H_excess · e^{-jωτ}`. Reuses the existing [`mixed_phase::decompose_phase`](../src/roomeq/mixed_phase.rs). Export both components alongside `spl`/`phase`/`coherence`.

- **H_min** — realisable by any IIR (including AP biquads). This is what the optimiser manipulates.
- **H_excess · e^{-jωτ}** — pure delay + modal non-minimum-phase zeros. Not correctable by IIR; lives in the objective as an uncontrollable constant.

Below ~100 Hz the direct sound and first reflections are indistinguishable at metre-scale wavelengths, so `H_excess ≈ 1` and min-phase ≈ measured — no gate tuning required, no "gated LF is fictional" artefact.

### 2.5 Probe step integration

The existing Probe step (tone-burst delay detection) captures arrival times. Extend it to capture a **short bass tone burst** (20 Hz, 5 cycles) per channel and fit its envelope phase. This gives a per-channel anchor for the sweep-derived phase that survives even if the sweep SNR is marginal. Feed the anchor into the sweep unwrap as a hard constraint on the first bin.

### 2.6 Calibrated microphone phase

USB measurement mics (UMIK-1 etc.) typically ship with magnitude calibration only. Below 50 Hz the mic's own phase can drift ±30°. Add:

- `mic_calibration_phase_path: Option<PathBuf>` in `RecordingConfiguration`.
- A secondary calibration routine (sub-bass comparison against a known reference, e.g. a measurement-grade electret) that produces a 4-column CSV (`freq, mag_db, phase_deg, coherence`).

Without mic phase calibration, bass phase from cheap mics is a rounding error compared to the room. Document this clearly — the UI should surface "Bass phase untrusted: mic phase not calibrated" as an `Advisory`.

---

## 3. Algorithm design

### 3.1 Global, not pairwise

Goal: find **one delay + one set of AP filters per channel** that minimises the GD deviation of the **summed response at the listening position** inside the bass band.

```
objective(delays, ap_params) =
    ∫ w(f) · |GD_sum(f) − GD_target(f)|² df
over f ∈ [min_freq .. crossover + 1 oct]

GD_sum(f) = -d/dω arg Σ_channels  H_ch(f) · e^(-jωτ_ch) · H_ap_ch(f)
```

Key properties:
- Sums complex transfer functions first, takes phase derivative after. This captures what the listener hears when two sources mutually cancel at a modal null.
- `w(f)` is `coherence²(f)` — unreliable bins contribute nothing to the loss.
- `GD_target(f)` is the flattest common GD achievable; default to a constant `τ_ref` chosen as the median GD across channels in the gated direct-sound window.

### 3.2 Parameter space

Per channel:
- 1 delay `τ` in ms (range `[0, max_delay_ms]`, coarse LS → fine Nelder-Mead).
- 0–2 AP filters `(f_ap, Q_ap)` — **adaptive budget**, not fixed. Each additional filter is accepted only if it passes the bootstrap improvement test in §3.3.
- 1 polarity bit (optional, reuse `PhaseAlignmentConfig::optimize_polarity`).

The search is small enough that `math-de` does it in <5 s per run. Keep it in DE with a local refinement; do **not** write another greedy one-at-a-time search.

### 3.3 Adaptive AP filter budget

v1 capped at 3 AP filters with a crude 10 % improvement heuristic that consistently overfit noisy bass data. v2 uses a bootstrap test driven by the N realisations already recorded for coherence (§2.2):

```
for k in 1..=2:
    fit (τ, ap_1..ap_k) over the full measurement
    for each of the N per-sweep realisations i:
        compute sum_gd_rms_i with and without ap_k
    σ = stddev over i of (rms_with − rms_without)
    keep ap_k iff (mean improvement) / σ > 3
```

The null case is "noise-driven improvement"; the 3σ threshold rejects filters that correlate with sweep-to-sweep jitter rather than a true acoustic feature. Budget hard-capped at 2 regardless.

### 3.4 Band derivation

No hard-coded 200 Hz. Compute:

```rust
let band_lo = max(min_freq, crossover_freq * 0.25);   // one octave below XO, clamped
let band_hi = crossover_freq * 2.0;                   // one octave above XO
```

Reads `crossover_freq` from the matching entry in `RoomConfig.crossovers` or from the detected XO in the measurement (via `math-dsp` bandpass slope fit — already available).

### 3.5 Confidence gate

Before running the optimiser, compute:

```rust
fn bass_phase_confidence(curves: &[Curve], band: (f64, f64)) -> BassPhaseConfidence
```

returning `Trustworthy { mean_coherence: f64 }` or `Degraded { reason: &'static str }` where `reason` is one of:
- `"no_phase_data"`
- `"coherence_below_threshold"` (mean γ² < 0.8 in band)
- `"mic_phase_uncalibrated"`
- `"insufficient_bass_duration"` (duration-per-octave < 2 s)
- `"snr_below_10db"` (when noise floor estimate from pre-silence exceeds signal-in-band − 10 dB)

If degraded, GD-Opt v2 returns `Err` early with the reason, the `RoomEqReport` surfaces it as an `Advisory::GdOptDegradedPhase { reason }`, and nothing is written to the plugin chain.

### 3.6 Output

```rust
pub struct GroupDelayOptResult {
    pub band: (f64, f64),
    pub per_channel: HashMap<String, ChannelGdResult>,
    pub sum_gd_pre_rms_ms: f64,
    pub sum_gd_post_rms_ms: f64,
    pub mean_coherence: f64,
    pub improvement_db: f64,  // 20 log10 (pre_rms / post_rms)
}

pub struct ChannelGdResult {
    pub delay_ms: f64,
    pub polarity_inverted: bool,
    pub ap_filters: Vec<Biquad>,  // ≤ 2
    pub channel_gd_pre_rms_ms: f64,
    pub channel_gd_post_rms_ms: f64,
}
```

This result attaches to `RoomEqReport.group_delay` (see main review) and ships in the JSON output so the UI can draw the pre/post GD curves.

### 3.7 Per-mode dispatch

GD-Opt v2 behaves differently depending on `ProcessingMode`. Everything upstream — decomposition (§2.4), confidence gate (§3.5), band derivation (§3.4), `GroupDelayOptResult` struct (§3.6) — is shared. The output path is the variation point.

| Mode | Phase-shaping mechanism | v2 output |
|---|---|---|
| `LowLatency` | IIR biquads only. | Delays + ≤2 min-phase AP biquads per channel. Primary use case. |
| `Hybrid` | IIR below XO, FIR above. | Same as `LowLatency` but **asserts `band_hi ≤ mixed_config.crossover_freq`** so AP filters never straddle the FIR takeover. |
| `MixedPhase` | IIR (mag + min-phase) + short per-channel excess-phase FIR. | **Runs after** [`mixed_phase::decompose_phase`](../src/roomeq/mixed_phase.rs). Each channel's excess phase has already been flattened individually; v2 handles only inter-channel alignment on the residual — typically 1 delay per channel, occasionally 1 AP. |
| `WarpedIir`, `KautzModal` | IIR (different basis). | Same as `LowLatency`; AP biquads compose normally. |
| `PhaseLinear` | FIR only. | **No AP filters emitted.** See below. |

**`PhaseLinear` is structurally different.** A linear-phase FIR cannot alter the measurement's phase curve — inter-channel phase mismatches survive a pure magnitude correction untouched. Two mutually-exclusive answers:

1. Accept the mismatch (current behaviour). Then GD-Opt has nothing to do; skip it in `PhaseLinear`.
2. Fold the GD target into the FIR design via **Kirkeby mixed-phase inversion** (already implemented in [`fir::generate_fir_correction`](../src/roomeq/fir.rs) behind `correct_excess_phase: true`). The FIR then inverts magnitude *and* min-phase up to the excess-phase bound set by `min_spatial_depth` / `pre_ringing_threshold_db`.

v2 takes path (2): the FIR designer gains an optional input

```rust
pub struct GdAlignmentTarget {
    pub per_channel_delay_ms: HashMap<String, f64>,
    pub sum_gd_reference: Array1<f64>,
}

fn generate_fir_correction(
    curve: &Curve,
    config: &OptimizerConfig,
    target_curve: Option<&TargetCurveConfig>,
    sample_rate: f64,
    gd_target: Option<&GdAlignmentTarget>,   // ← new
) -> Result<Vec<f64>>
```

The confidence gate runs upstream exactly as for IIR modes. When it passes, `GdAlignmentTarget` is built from the same min-phase decomposition (§2.4); when it fails, `gd_target = None` and the FIR designer falls back to pure magnitude correction. `GroupDelayOptResult` still ships — with empty `ap_filters` and a non-empty `per_channel_delay_ms` — so the report plumbing (Phase GD-4) is mode-agnostic.

---

## 4. Implementation phases (v2)

### Phase GD-1 — Recording upgrades

Files: `crates/sotf-engine/src/signal_recorder.rs`, `crates/autoeq/src/roomeq/types/config.rs` (`RecordingConfiguration` extension), recording wizard UI under `crates/app-gpui/components/room_eq/`.

Deliverables:
- Configurable per-octave duration, multi-sweep averaging, coherence export.
- Min-phase / excess-phase decomposition at measurement export time (reuses `mixed_phase::decompose_phase`).
- Updated probe step with 20 Hz tone-burst anchor.
- Mic phase calibration loader.

### Phase GD-2 — Bass phase confidence

Files: `crates/autoeq/src/roomeq/phase_utils.rs` (extend), new `bass_phase_confidence.rs`.

Deliverables:
- `BassPhaseConfidence` API.
- Tests on known synthetic cases (clean sweep, degraded sweep, missing coherence).

### Phase GD-3a — IIR-path optimiser (AP filters + delays)

Files: new `crates/autoeq/src/roomeq/gd_opt.rs`, integration points in `optimize.rs`.

Deliverables:
- Complex-sum GD objective on `H_min`, with `H_excess` as an uncontrollable constant (see §2.4).
- DE + local refinement over `(τ, f_ap, Q_ap, polarity)`.
- Adaptive AP budget with bootstrap gating (§3.3).
- Honours `crossovers` config for band derivation.
- Dispatches `LowLatency`, `Hybrid` (with XO assert), `MixedPhase` (after per-channel excess-phase FIR), `WarpedIir`, `KautzModal`.

### Phase GD-3b — FIR-path integration (PhaseLinear only)

Files: `crates/autoeq/src/roomeq/fir.rs`, `crates/autoeq/src/roomeq/gd_opt.rs` (target construction), integration points in `optimize.rs`.

Deliverables:
- `GdAlignmentTarget` struct and new optional parameter on `generate_fir_correction`.
- Kirkeby pipeline consumes the target when present; falls back to pure magnitude when absent.
- Emits a `GroupDelayOptResult` with empty `ap_filters` and populated `per_channel_delay_ms` so Phase GD-4 plumbing is mode-agnostic.

### Phase GD-4 — Report plumbing

Attach `GroupDelayOptResult` to `RoomEqReport`; emit `Advisory` entries for all confidence-gate outcomes.

### Phase GD-5 — QA

- Golden-ratio QA case in `roomeq_qa_quality`: synthetic L/R/Sub with known delay mismatch + known AP distortion; assert improvement ≥ 6 dB and delay recovery within 0.1 ms.
- Real-world smoke test against a reference recording set (`data_tests/roomeq/gd_case/`).

---

## 5. Non-goals (explicitly deferred)

- **Multi-seat GD.** v2 targets the single primary seat. Multi-seat phase averaging is ill-posed below the Schroeder frequency and requires its own plan.
- **Replacing `phase_alignment.rs`.** GD-Opt and phase-alignment solve different problems (GD-Opt: bass coherence in the sum; phase-alignment: polarity+delay at the crossover null). v2 composes with phase-alignment, not replaces it.

---

## 6. Success criteria

GD-Opt v2 is done when, on a reference 2.1 recording:

- On clean bass phase (γ² ≥ 0.9): sum-GD RMS drops by ≥ 6 dB inside `[band_lo, band_hi]`.
- On degraded bass phase: the optimiser refuses to run, the report explains why, and the audio chain is left untouched.
- UI shows pre/post GD curves, detected XO, confidence per bin, and the advisory list.
- No regression on non-2.1 configurations.
