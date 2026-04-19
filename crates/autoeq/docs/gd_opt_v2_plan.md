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

## 2. Recording changes (prerequisite) — design locked

This section describes the full set of changes to the recording pipeline
that GD-Opt v2 is blocked on. Everything here is committed as the design
of record; implementation phases are enumerated in §2.10.

### 2.1 North star and acceptance criteria

When a user completes the recording wizard on a 2.1 system with a
modestly noisy room and a calibrated UMIK-1:

1. Exported per-channel curves carry a `coherence` column with γ² ≥ 0.9
   across `[band_lo, band_hi]` on ≥ 80 % of recordings.
2. The `BassPhaseConfidence` gate (§3.5) classifies the recording as
   `Trustworthy` deterministically.
3. Every WAV file needed to recompute the curves offline is in the
   session directory, with pre- and post-silence windows intact.
4. The bass-anchor step resolves the first measured bin's phase
   without the current 2π wraparound ambiguity.

Failure is **graceful**: if any of (1)–(4) fail, the wizard still
completes, the optimiser runs magnitude-only, and the report emits
`Advisory::GdOptDegradedPhase { reason }`.

### 2.2 Data contract — `RecordingConfiguration` + `SplCalibration`

All new fields are optional (`Option<_>`) so that intermediate-phase
session files load via serde defaults. Session files written before
GD-1a are *not* migrated — see §2.11 Q6.

```rust
pub struct RecordingConfiguration {
    // ... existing fields unchanged ...

    // --- A: sweep shape ---
    pub bass_octave_duration_s: Option<f32>,   // default 3.0; clamp [1.0 .. 10.0]
    pub pre_silence_s: Option<f32>,            // default 2.0
    pub post_silence_s: Option<f32>,           // default = schroeder_rt60 + 1.0
    pub sweep_level_db_spl: Option<f32>,       // target SPL @ listening position

    // --- B: multi-sweep ---
    pub num_sweeps: Option<u8>,                // default 4; clamp [1 .. 8]
    pub coherence_threshold: Option<f32>,      // default 0.9; user-exposed, clamp [0.5 .. 0.99]

    // --- D: bass anchor ---
    pub bass_probe_freq_hz: Option<f32>,       // default 20.0 (or 1.25 × min_freq, whichever higher)
    pub bass_probe_cycles: Option<u16>,        // default 5

    // --- E: mic phase calibration ---
    pub mic_phase_calibration_path: Option<String>,
    pub mic_phase_calibration_paths: Option<Vec<Option<String>>>,

    // --- Q4: SPL calibration anchor ---
    pub spl_calibration: Option<SplCalibration>,
}

/// SPL calibration anchor captured from a pre-sweep reference-tone read.
/// Maps a peak-sample-value on the recording ADC to a dBSPL reading at
/// the listening position, so `sweep_level_db_spl` can be targeted
/// deterministically.
pub struct SplCalibration {
    /// User-reported dBSPL at the listening position during calibration.
    pub reported_db_spl: f32,
    /// Frequency of the calibration tone in Hz (default 1000).
    pub reference_freq_hz: f32,
    /// Peak sample value observed on the recording ADC during the tone.
    pub peak_sample_level: f32,
    /// Offset: `dbspl_at_mic = 20*log10(peak_sample_value) + spl_offset_db`.
    pub spl_offset_db: f32,
}
```

### 2.3 Data contract — `Curve` extensions

```rust
pub struct Curve {
    pub freq: Array1<f64>,
    pub spl: Array1<f64>,
    pub phase: Option<Array1<f64>>,
    // --- NEW (all Option<_>; default None for legacy CSVs) ---
    pub coherence: Option<Array1<f64>>,
    pub noise_floor_db: Option<Array1<f64>>,
    // Min-phase / excess-phase are computed at LOAD time (Q3), not
    // persisted to the CSV. Left as cached fields on the struct so
    // callers can lazily request them.
    pub min_phase: Option<Array1<f64>>,
    pub excess_phase: Option<Array1<f64>>,
    pub excess_delay_ms: Option<f64>,
}
```

### 2.4 CSV column layout

Legacy CSVs (`freq, spl, phase`) keep loading unchanged. New CSVs append
two columns:

| Column | Source | Consumed by |
|---|---|---|
| `coherence` | §2.5, computed during multi-sweep averaging | GD confidence gate; optimiser weight |
| `noise_floor_db` | Pre-silence window RMS per 1/6-octave band | SNR gate in §3.5; Evaluating UI |

`min_phase_deg`, `excess_phase_deg`, and `excess_delay_ms` are **not**
persisted to CSV — they are recomputed at load time from the raw sweep
WAVs per the decision in §2.11 Q3. Keeping them out of CSV means the
decomposition algorithm can evolve without requiring recording
re-export.

CSV readers key off column-name (not position), so additional columns
in either direction are tolerated.

### 2.5 Session directory layout

```
<recording_dir>/
  config.json                          # RecordingConfiguration
  probe_wideband.wav                   # existing narrowband delay probe
  probe_bass.wav                       # NEW: per-channel bass tone burst
  ch00_left/
    sweep_01.wav                       # NEW: per-sweep raw capture
    sweep_02.wav
    sweep_03.wav
    sweep_04.wav
    fr.csv                             # extended column set (see §2.4)
  ch01_right/...
```

Per-sweep WAVs are **always** kept (§2.11 Q2). The coherence-averaged
IR is *not* persisted; it is recomputed on load. Storage cost: ~150 MB
for a 5-channel session at the defaults — acceptable.

### 2.6 Wizard flow — 7 steps

```
Config
  └─ SplCalibration       [NEW, Q4 required]
Capture
Probe                     [existing wideband narrow-band delay probe]
  └─ BassAnchor           [NEW, Q1 dedicated sub-step]
Evaluating
Saving
```

- **Config** gains: per-channel mic-phase picker (E); `num_sweeps`
  slider (B); `bass_octave_duration_s` slider (A); `coherence_threshold`
  numeric in Advanced (Q5).
- **SplCalibration** is a blocking gate: user plays a 1 kHz tone at
  a fixed digital level, types the dBSPL their handheld meter reads
  at the listening position, and `spl_offset_db` is persisted.
  `sweep_level_db_spl` becomes non-optional on write (though the
  field on disk stays `Option<_>` for wire-format stability).
- **Capture** shows per-sweep progress ("Sweep 2 / 4") and a
  live coherence preview after each sweep completes.
- **Probe** is unchanged.
- **BassAnchor** plays a 20 Hz × 5-cycle tone burst per channel and
  fits envelope phase. Total run: ~20 s for a 5-channel system.
- **Evaluating** shows a coherence strip below the FR plot
  (green ≥ 0.9, amber 0.7–0.9, red < 0.7) and the per-channel
  `excess_delay_ms` number (computed on load in §2.3).
- **Saving** writes the extended CSVs + `config.json` (+ all raw
  sweep WAVs).

### 2.7 Defaults & budgets

| Parameter | Default | Rationale |
|---|---|---|
| `bass_octave_duration_s` | 3.0 | Lets modal energy settle below 100 Hz |
| `pre_silence_s` | 2.0 | HVAC / mains-harmonic noise-floor estimation |
| `post_silence_s` | `rt60 + 1.0` | Captures full room decay; RT60 from room volume if absent |
| `num_sweeps` | 4 | Bootstrap budget in §3.3 needs N ≥ 4 for the 3σ test |
| `coherence_threshold` | 0.9 | Matches optimiser refusal threshold in §3.5 |
| `bass_probe_freq_hz` | 20.0 | Below every plausible bass XO |
| `bass_probe_cycles` | 5 | Enough for envelope phase fit; short enough to avoid modal ringing |

Total recording time at defaults, 5-channel system, 10 Hz → 20 kHz:
~14 min (was ~1 min). GD-Opt v2 requires this budget; surface an
early-warning "estimated capture time" in the Config step.

### 2.8 Failure modes

All failures surface through `RoomEqReport::Advisory`; the recording
pipeline itself never aborts.

| Trigger | Advisory reason | Effect on GD-Opt |
|---|---|---|
| `num_sweeps == 1` OR coherence column missing | `"no_coherence_data"` | Skip GD-Opt |
| Mean coherence in `[band_lo, band_hi]` < 0.9 | `"coherence_below_threshold"` | Skip GD-Opt |
| `noise_floor_db` within 10 dB of in-band signal | `"snr_below_10db"` | Skip GD-Opt |
| Mic phase cal absent on known unflat-phase mics | `"mic_phase_uncalibrated"` | Warn, proceed |
| Bass probe envelope-phase fit residual > 20° | `"bass_anchor_unreliable"` | Proceed without anchor |
| `bass_octave_duration_s < 2.0` OR `num_sweeps < 4` | `"insufficient_bass_duration"` | Skip GD-Opt |
| `spl_calibration` absent | `"no_spl_calibration"` | Warn (Q4 says require at wizard level, but the gate stays in case of config-file-only paths) |

### 2.9 Cross-cutting

- **Determinism:** extend the existing probe seed pattern to the sweep
  generator; expose as optional `seed: Option<u64>` in
  `RecordingConfiguration`. QA sets; UI hides.
- **Sample rate:** all upgrades are sample-rate-agnostic. 20 Hz × 5
  cycles = 250 ms = 12 000 samples @ 48 kHz.
- **Storage:** ~150 MB/session. Session-size shown in Saving step
  for visibility, no opt-out.
- **Format compat:** extended CSV starts with the legacy three
  columns in the same units, so external tools (REW, ARTA,
  HolmImpulse) keep working.

### 2.10 Implementation phases

Each phase is independently mergeable. Phases touching
`crates/sotf-engine` get a dedicated PR per the engine/plugins rule.

| Phase | Scope | Files touched |
|---|---|---|
| **GD-1a** ✅ | Config types only — `RecordingConfiguration` extensions, `SplCalibration`, schema regen | autoeq (config types + schema), tests |
| **GD-1a.1** | Delete `migrate_legacy_recording`; introduce `AutoeqError::UnsupportedRecordingFormat` | sotf-engine (engine PR) |
| **GD-1a.2** ✅ | `Curve` extensions (`coherence`, `noise_floor_db`, `min_phase`, `excess_phase`, `excess_delay_ms`) + `impl Default` + `..Default::default()` spread across ~72 call sites; CSV reader header-driven tolerance for the new columns | autoeq + sotf-player + app-gpui + app-tui + gpui-toolkit demos |
| **GD-1b** | Sweep shape (A): octave-scaled generator + pre/post silence | math-dsp, sotf-engine, Config UI |
| **GD-1c** | Multi-sweep (B): `record_multi_sweep`, `compute_coherence`, session-directory layout, Capture UI | sotf-engine (engine PR), Capture UI |
| **GD-1d** | Load-time min-phase decomposition (Q3): `Curve::decompose_into_cache()` called by CSV loader and by `Curve::from_measurement` | autoeq |
| **GD-1e** | BassAnchor wizard step (Q1-b) | sotf-engine, app-gpui wizard |
| **GD-1e.5** | SplCalibration wizard step (Q4) | sotf-engine (tone playback), app-gpui wizard |
| **GD-1f** | Mic phase calibration loader (E) | math-dsp, Config UI |
| **GD-1g** | `BassPhaseConfidence` gate stub reading from `Curve` | autoeq (prep for GD-2) |

Estimated effort: ~13 working days.

### 2.11 Decision log (resolves §9 of the review)

| # | Question | Decision | Rationale |
|---|---|---|---|
| 1 | Bass tone burst location | **(b)** dedicated wizard sub-step (BassAnchor) | Clean separation; reuses probe persistence; engine code is a trimmed variant of `probe_channel_delays_core` |
| 2 | Raw sweep persistence | **Persist all**, no user toggle | Required to re-compute coherence / min-phase offline after algorithm changes |
| 3 | Min-phase decomposition time | **Load time**, not save time | Decoupling: decomposition algorithm can evolve without CSV re-export |
| 4 | SPL cap | **Require** with a calibration routine | Determinism; avoids overdriving subwoofers at bass sweep levels |
| 5 | Coherence threshold UI | **Exposed** as a numeric in Advanced | User-visible quality gate |
| 6 | `migrate_legacy_recording` | **Drop** entirely | "No back-compat" direction; legacy recordings fail with a typed error |

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
