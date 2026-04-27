Roomeq Fix

# Phase 1: Correctness Blockers

Create one canonical post-DSP response update path

## Add a helper that applies any inserted DSP plugin to both channel_chains and channel_results.final_curve.
- It must update magnitude and phase where relevant: gain/EQ/all-pass/delay/polarity.
- All later stages must read from this canonical final curve, never from stale pre-stage curves.
- Fix stale reporting for time alignment

## When auto time-alignment delay plugins are inserted, update final curve phase immediately.
- Recompute downstream GD input, EPA, aggregate scores, and IR generation from the updated curve.
- Fix stale reporting for spectral alignment

After spectral alignment appends EQ/gain plugins, apply those changes into final curves.
Ensure VoG, ICD, GD, EPA, and QA all see the aligned response.
Fix stale reporting for Voice-of-God correction

## Apply VoG EQ/gain plugins into final curves.
Add QA asserting exported VoG DSP and reported curves match.
Fix stale reporting for ICD correction

## Move ICD correction before final IR/EPA/metadata generation, or recompute those artifacts after ICD correction.
Ensure combined_post_score, epa_per_channel, final IRs, and metadata all reflect inserted matching filters.

# Phase 2: Phase-Critical Safety

Require valid phase for DBA

## Stop substituting 0° phase in DBA complex summation.
If phase is missing, skip DBA with a clear advisory.
Preserve phase in the computed DBA combined curve where possible so downstream phase/GD checks can evaluate it.
Require valid phase and shared grids for cardioid subs

## Reject or skip cardioid synthesis when front/rear phase is missing.
Validate or resample front/rear curves onto a common frequency grid before complex summation.
Add tests for missing phase, grid mismatch, and successful measured-phase summation.
Standardize frequency-grid validation/resampling

## Add shared utility for “same grid by value” validation and optional log-frequency resampling.
Use it in spectral alignment, ICD, DBA, cardioid, MSO, GD, and any complex-sum code.
Prefer rejecting phase-critical mismatches unless safe interpolation is explicitly implemented.
Fix spectral alignment and ICD index assumptions

Validate or resample all channel curves before computing masks, gains, deviations, and correction filters.
Add regression tests using same-length but shifted frequency grids.

# Phase 3: Perceptual Objective Improvements

## Upgrade MSO objective
Replace variance-only or flatness-only objectives with a weighted objective: average target fit, seat-to-seat variance, primary-seat priority, null avoidance, extension, and headroom.
Keep delay/gain/polarity normalized so the optimizer does not add arbitrary latency or lose output unnecessarily.

## Add QA fixtures for “variance improves but bass target collapses” to prevent perceptually bad wins.
Make channel matching role-aware
Match L/R tightly.
Match LCR with center/dialog priority.
Match surrounds and heights within bandwidth-appropriate limits.

## Avoid forcing all channels toward one global spectral profile.
Improve GD/AP production readiness
Add real multi-sweep/bootstrap ingestion so AP filters can be enabled safely.
Keep AP disabled without bootstrap evidence.
Add perceptual weighting around crossover and modal bands instead of optimizing raw GD uniformly.
Improve correction scoring
Add phase-aware and seat-aware metrics alongside magnitude scores.
Report separate perceptual metrics for bass consistency, dialog band smoothness, channel matching, headroom, and timing confidence.
Avoid using a single “looks flat” score as the main success indicator.

# Phase 4: Home-Cinema Feature Gaps

## Add bass-management optimization
- Model main high-pass, sub low-pass, crossover slope/order, redirected bass, and LFE channel summation.
- Optimize crossover phase and delay per speaker group.
- Include LFE +10 dB calibration and sub trim/headroom limits.

## Add speaker-role target curves
- Separate targets for LCR, center/dialog, surrounds, heights, subs, and LFE.
- Support cinema-style tilt/X-curve behavior and room-size/listening-distance compensation.
- Make target choice explicit in metadata and QA.

## Add home-cinema layout model
- Support common layouts: 5.1, 7.1, 5.1.2, 7.1.4, 9.1.6.
- Track roles, groups, bandwidth limits, crossover eligibility, and matching groups.
- Use layout roles to drive optimization constraints.

## Add multi-seat correction beyond subs
- Add weighted seats and primary-seat constraints for all channels.
- Use spatial variance masks to avoid overcorrecting seat-specific nulls.
- Report per-seat predicted response and per-seat pass/fail metrics.

## Add timing/localization diagnostics
- Report acoustic distance/delay consistency.
- Flag surround/height precedence risks.
- Add LCR imaging and center-dialog timing checks.
- Add home-cinema QA targets

## QA

- qa-roomeq-dsp-consistency: exported DSP must match final curves/IRs/metadata.
- qa-roomeq-phase-critical: missing phase/grid mismatch must skip DBA/cardioid/MSO/GD safely.
- qa-roomeq-home-cinema: bass management, LFE calibration, role targets, and layout constraints.
- qa-roomeq-perceptual: verifies objective improvements do not trade away bass target, headroom, or dialog-band quality.

# Assumptions

Fixing stale DSP/metric consistency is the first priority because it affects trust in every later metric.
Phase-critical features should skip rather than invent phase.
Perceptual quality takes precedence over cosmetically flat curves.
Home-cinema work should be role-aware from the start rather than treating all channels as interchangeable.
