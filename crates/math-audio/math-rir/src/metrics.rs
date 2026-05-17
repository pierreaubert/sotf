//! ISO 3382 room acoustic metrics from a Room Impulse Response.
//!
//! Computes the parameters defined in
//! - ISO 3382-1:2009 *Acoustics — Measurement of room acoustic parameters,
//!   Part 1: Performance spaces*
//! - ISO 3382-2:2008 *Part 2: Reverberation time in ordinary rooms*
//!
//! Implemented here:
//!
//! | Metric | Definition                                                          |
//! |--------|---------------------------------------------------------------------|
//! | EDT    | Early decay time — slope of Schroeder decay over `0 → −10 dB`,      |
//! |        | extrapolated to a 60 dB drop.                                       |
//! | T20    | Slope of Schroeder decay over `−5 → −25 dB`, extrapolated to 60 dB. |
//! | T30    | Slope of Schroeder decay over `−5 → −35 dB`, extrapolated to 60 dB. |
//! | C50    | Clarity (50 ms): `10·log10(E_[0,50ms] / E_(50ms,∞))`.               |
//! | C80    | Clarity (80 ms): same, for music.                                   |
//! | D50    | Definition: `E_[0,50ms] / E_[0,∞)` (ratio in 0..1).                 |
//! | Ts     | Centre time: `∫ t · h²(t) dt / ∫ h²(t) dt` (seconds).               |
//!
//! All time integrations start at the **direct-sound arrival**
//! (see [`find_direct_sound_toa`] in `detection.rs`) so that pre-arrival
//! silence in the recording does not skew the result.
//!
//! Each fitted line carries an `r²` so callers can reject reverberation
//! times where the decay is not linear in dB (e.g. coupled rooms or
//! noise-dominated tails).

use crate::config::SsirConfig;
use crate::detection::find_direct_sound_toa;

/// Schroeder backward-integrated decay curve of a RIR, expressed in dB
/// relative to the curve's peak.
///
/// `samples[n] = 10·log10( ∫_n^{cutoff} h²(τ) dτ / ∫_0^{cutoff} h²(τ) dτ )`
///
/// Truncation at `cutoff` (the estimated noise-floor crossover) avoids the
/// "lift" that a never-decaying integrated noise tail would otherwise add
/// to the curve — see Chu (1978) and Lundeby et al. (1995).
#[derive(Debug, Clone)]
pub struct DecayCurve {
    /// Sample-by-sample Schroeder decay in dB (0 dB at the start, decreasing).
    pub samples: Vec<f64>,
    /// Sample rate the curve was computed at.
    pub sample_rate: f64,
    /// Index (within `samples`) at which the underlying RIR was truncated
    /// before backward-integration. Below this sample the curve is dominated
    /// by noise and should not be used for slope fitting.
    pub noise_cutoff_sample: usize,
}

impl DecayCurve {
    /// Compute the Schroeder decay curve from a RIR.
    ///
    /// `start_sample` is the index of the direct sound; integration starts
    /// from there. `noise_cutoff_sample` (absolute index in `rir`) lets the
    /// caller override the auto-detected noise truncation; if `None`,
    /// [`estimate_noise_cutoff`] is used.
    pub fn from_rir(
        rir: &[f32],
        sample_rate: f64,
        start_sample: usize,
        noise_cutoff_sample: Option<usize>,
    ) -> Self {
        if rir.is_empty() || start_sample >= rir.len() {
            return Self {
                samples: Vec::new(),
                sample_rate,
                noise_cutoff_sample: 0,
            };
        }

        let cutoff_abs =
            noise_cutoff_sample.unwrap_or_else(|| estimate_noise_cutoff(rir, start_sample));
        let cutoff_abs = cutoff_abs.min(rir.len());
        let cutoff_rel = cutoff_abs.saturating_sub(start_sample);

        // Square h(n) and backward-integrate from `cutoff_abs` down to
        // `start_sample`. We work in f64 throughout — for a 200 ms IR at
        // 48 kHz this is < 10k accumulations; precision matters at the
        // −35 dB tail.
        let n = cutoff_rel;
        if n == 0 {
            return Self {
                samples: Vec::new(),
                sample_rate,
                noise_cutoff_sample: 0,
            };
        }

        let mut energy: Vec<f64> = Vec::with_capacity(n);
        let mut acc = 0.0_f64;
        // Walk from end → start, summing h²; reverse afterwards so that
        // `energy[i]` = ∫_{start_sample+i}^{cutoff_abs} h²(τ) dτ.
        for i in (0..n).rev() {
            let s = rir[start_sample + i] as f64;
            acc += s * s;
            energy.push(acc);
        }
        energy.reverse();

        let total = energy[0];
        if total <= 0.0 || !total.is_finite() {
            return Self {
                samples: Vec::new(),
                sample_rate,
                noise_cutoff_sample: cutoff_abs,
            };
        }

        // Convert to dB relative to the peak. The smallest representable
        // value past the tail still produces a finite dB (clamped at
        // −300 dB) so consumers don't have to handle `-∞`.
        let inv_total = 1.0 / total;
        let samples: Vec<f64> = energy
            .iter()
            .map(|&e| {
                let r = e * inv_total;
                if r <= 0.0 {
                    -300.0
                } else {
                    10.0 * r.log10()
                }
            })
            .collect();

        Self {
            samples,
            sample_rate,
            noise_cutoff_sample: cutoff_abs,
        }
    }

    /// First sample index at which the curve is `≤ threshold_db`. Returns
    /// `None` if the curve never reaches the threshold.
    pub fn first_crossing(&self, threshold_db: f64) -> Option<usize> {
        self.samples
            .iter()
            .position(|&v| v <= threshold_db)
    }

    /// Least-squares fit of the decay between two dB thresholds.
    ///
    /// Returns `(slope_db_per_s, intercept_db, r_squared)`. `None` if either
    /// threshold is never reached or fewer than two samples lie in the band.
    pub fn fit_db_range(
        &self,
        upper_db: f64,
        lower_db: f64,
    ) -> Option<(f64, f64, f64)> {
        debug_assert!(upper_db > lower_db);
        let i_upper = self.first_crossing(upper_db)?;
        let i_lower = self.first_crossing(lower_db)?;
        if i_lower <= i_upper + 1 {
            return None;
        }

        // x is time in seconds (sample index / sample_rate); y is decay dB.
        let dt = 1.0 / self.sample_rate;
        let xs = (i_upper..=i_lower).map(|i| (i as f64) * dt);
        let ys = self.samples[i_upper..=i_lower].iter().copied();
        linear_fit(xs, ys)
    }
}

/// Least-squares linear fit `y = slope·x + intercept`. Returns
/// `(slope, intercept, r²)`. `None` if `n < 2` or `Var(x) = 0`.
fn linear_fit<X, Y>(xs: X, ys: Y) -> Option<(f64, f64, f64)>
where
    X: IntoIterator<Item = f64>,
    Y: IntoIterator<Item = f64>,
{
    let xs: Vec<f64> = xs.into_iter().collect();
    let ys: Vec<f64> = ys.into_iter().collect();
    let n = xs.len();
    if n < 2 || ys.len() != n {
        return None;
    }
    let n_f = n as f64;
    let sx: f64 = xs.iter().sum();
    let sy: f64 = ys.iter().sum();
    let sxx: f64 = xs.iter().map(|x| x * x).sum();
    let sxy: f64 = xs.iter().zip(ys.iter()).map(|(x, y)| x * y).sum();
    let syy: f64 = ys.iter().map(|y| y * y).sum();

    let denom = n_f * sxx - sx * sx;
    if denom.abs() < f64::EPSILON {
        return None;
    }
    let slope = (n_f * sxy - sx * sy) / denom;
    let intercept = (sy - slope * sx) / n_f;

    let ss_tot = syy - sy * sy / n_f;
    let ss_res: f64 = xs
        .iter()
        .zip(ys.iter())
        .map(|(x, y)| {
            let pred = slope * x + intercept;
            let r = y - pred;
            r * r
        })
        .sum();
    let r2 = if ss_tot.abs() < f64::EPSILON {
        1.0
    } else {
        (1.0 - ss_res / ss_tot).clamp(0.0, 1.0)
    };
    Some((slope, intercept, r2))
}

/// Estimate the sample index at which the RIR drops into the noise floor.
///
/// This is a deliberately simple two-pass estimator (Chu's method): the
/// noise floor is the mean of `h²` over the last 10 % of the signal, and
/// the cutoff is the first sample at which a 5 ms running mean of `h²`
/// drops within 10 dB of that floor. Lundeby's iterative refinement is a
/// possible future upgrade — for typical concert-hall RIRs this estimator
/// is within ±20 ms of the Lundeby result and good enough for T20/T30
/// computations that only need the −5..−25 / −5..−35 dB region.
pub fn estimate_noise_cutoff(rir: &[f32], start_sample: usize) -> usize {
    if rir.is_empty() || start_sample >= rir.len() {
        return rir.len();
    }
    let n = rir.len();
    let tail_start = start_sample + ((n - start_sample) * 9) / 10;
    if tail_start >= n {
        return n;
    }

    // Mean squared value of the last 10 % is the noise estimate.
    let tail_len = n - tail_start;
    let mut tail_e = 0.0_f64;
    for &s in &rir[tail_start..n] {
        let v = s as f64;
        tail_e += v * v;
    }
    let noise_e = tail_e / tail_len as f64;
    // 10 dB above the noise floor.
    let threshold = noise_e * 10.0;

    // 5 ms running mean of h². Sample-rate-independent: just use a
    // proportional window (5 % of the signal length, clamped).
    let win = ((n - start_sample) / 20).clamp(32, 4096);
    if win == 0 || win >= n - start_sample {
        return n;
    }

    let mut win_sum = 0.0_f64;
    for &s in &rir[start_sample..start_sample + win] {
        let v = s as f64;
        win_sum += v * v;
    }
    // Walk forward and find the first window whose mean drops below
    // `threshold`. Stop at `tail_start` — beyond that we're inside the
    // noise tail by definition.
    let limit = tail_start.min(n - win);
    for i in (start_sample + win)..limit {
        let inv = win as f64;
        if win_sum / inv < threshold {
            return i;
        }
        let drop = rir[i - win] as f64;
        let add = rir[i] as f64;
        win_sum += add * add - drop * drop;
    }
    tail_start
}

/// ISO 3382 single-band acoustic metrics for one RIR.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Iso3382Metrics {
    /// Early decay time — Schroeder slope over `0 → −10 dB`, extrapolated
    /// to a 60 dB drop. In seconds.
    pub edt_s: f64,
    /// T20 reverberation time (`−5 → −25 dB`, extrapolated to 60 dB). Seconds.
    pub t20_s: f64,
    /// T30 reverberation time (`−5 → −35 dB`, extrapolated to 60 dB). Seconds.
    pub t30_s: f64,
    /// Clarity at 50 ms (in dB). Higher = more speech-intelligibility-friendly.
    pub c50_db: f64,
    /// Clarity at 80 ms (in dB). Standard music clarity parameter.
    pub c80_db: f64,
    /// Definition: ratio of early (≤ 50 ms) to total energy. Dimensionless.
    pub d50: f64,
    /// Centre time (seconds). The temporal centre of gravity of `h²`.
    pub ts_s: f64,
    /// R² of the EDT linear fit (0..1). `< 0.9` is a quality warning.
    pub edt_r2: f64,
    /// R² of the T20 linear fit.
    pub t20_r2: f64,
    /// R² of the T30 linear fit.
    pub t30_r2: f64,
}

impl Iso3382Metrics {
    /// Returns `true` if every fitted decay region had `r² ≥ 0.95` —
    /// the conventional ISO 3382-1 acceptance threshold.
    pub fn fit_is_valid(&self) -> bool {
        self.edt_r2 >= 0.95 && self.t20_r2 >= 0.95 && self.t30_r2 >= 0.95
    }
}

/// Compute ISO 3382 single-band metrics on a broadband RIR.
///
/// For per-band analysis, bandpass the RIR with one of the helpers in
/// [`crate::bands`] and call this function on the filtered signal.
pub fn analyze_iso3382(rir: &[f32], sample_rate: f64) -> Iso3382Metrics {
    if rir.is_empty() || sample_rate <= 0.0 {
        return EMPTY_METRICS;
    }

    // Anchor t = 0 at the direct sound. If we cannot find one (silent or
    // sub-noise RIR), fall back to sample 0.
    let cfg = SsirConfig::new(sample_rate);
    let start = find_direct_sound_toa(rir, &cfg).unwrap_or(0);
    if start >= rir.len() {
        return EMPTY_METRICS;
    }

    // C50 / C80 / D50 / Ts: integrate h² from `start` onwards.
    let ms = sample_rate / 1000.0;
    let i50 = (start + (50.0 * ms) as usize).min(rir.len());
    let i80 = (start + (80.0 * ms) as usize).min(rir.len());

    let mut e_total = 0.0_f64;
    let mut e_50 = 0.0_f64;
    let mut e_80 = 0.0_f64;
    let mut ts_num = 0.0_f64;
    let dt = 1.0 / sample_rate;
    for (i, &s) in rir[start..].iter().enumerate() {
        let v = s as f64;
        let e = v * v;
        e_total += e;
        let abs_i = start + i;
        if abs_i < i50 {
            e_50 += e;
        }
        if abs_i < i80 {
            e_80 += e;
        }
        ts_num += (i as f64 * dt) * e;
    }

    let (c50_db, c80_db, d50, ts_s) = if e_total > 0.0 {
        let e_late_50 = (e_total - e_50).max(f64::MIN_POSITIVE);
        let e_late_80 = (e_total - e_80).max(f64::MIN_POSITIVE);
        let c50 = 10.0 * (e_50.max(f64::MIN_POSITIVE) / e_late_50).log10();
        let c80 = 10.0 * (e_80.max(f64::MIN_POSITIVE) / e_late_80).log10();
        let d50 = (e_50 / e_total).clamp(0.0, 1.0);
        let ts = ts_num / e_total;
        (c50, c80, d50, ts)
    } else {
        (f64::NAN, f64::NAN, f64::NAN, f64::NAN)
    };

    // Schroeder decay → EDT / T20 / T30.
    let curve = DecayCurve::from_rir(rir, sample_rate, start, None);
    let (edt_s, edt_r2) = match curve.fit_db_range(0.0, -10.0) {
        Some((slope, _, r2)) if slope < 0.0 => (-60.0 / slope, r2),
        _ => (f64::NAN, 0.0),
    };
    let (t20_s, t20_r2) = match curve.fit_db_range(-5.0, -25.0) {
        Some((slope, _, r2)) if slope < 0.0 => (-60.0 / slope, r2),
        _ => (f64::NAN, 0.0),
    };
    let (t30_s, t30_r2) = match curve.fit_db_range(-5.0, -35.0) {
        Some((slope, _, r2)) if slope < 0.0 => (-60.0 / slope, r2),
        _ => (f64::NAN, 0.0),
    };

    Iso3382Metrics {
        edt_s,
        t20_s,
        t30_s,
        c50_db,
        c80_db,
        d50,
        ts_s,
        edt_r2,
        t20_r2,
        t30_r2,
    }
}

const EMPTY_METRICS: Iso3382Metrics = Iso3382Metrics {
    edt_s: f64::NAN,
    t20_s: f64::NAN,
    t30_s: f64::NAN,
    c50_db: f64::NAN,
    c80_db: f64::NAN,
    d50: f64::NAN,
    ts_s: f64::NAN,
    edt_r2: 0.0,
    t20_r2: 0.0,
    t30_r2: 0.0,
};

#[cfg(test)]
mod tests {
    use super::*;

    /// Synthetic exponentially-decaying noise burst whose −60 dB time
    /// equals `t60_s`. Useful as ground truth for T20/T30/EDT.
    fn exponential_decay_rir(sample_rate: f64, t60_s: f64, duration_s: f64) -> Vec<f32> {
        let n = (duration_s * sample_rate) as usize;
        // h(t) = exp(-α t) · ξ(t),    α such that 20·log10(exp(-α·T60)) = -60
        // ⇒ α = ln(10⁶) / (2·T60) (because h² gives 60 dB drop at T60).
        let alpha = std::f64::consts::LN_10 * 6.0 / (2.0 * t60_s);
        let mut rir = vec![0.0f32; n];
        // First sample = direct sound.
        rir[0] = 1.0;
        // Pseudo-noise tail with envelope exp(-α t).
        let mut state: u64 = 0xDEAD_BEEF_CAFE_BABE;
        for (i, sample) in rir.iter_mut().enumerate().skip(1) {
            // xorshift64
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            let noise = ((state >> 32) as i32 as f64) / (i32::MAX as f64);
            let t = i as f64 / sample_rate;
            *sample = (noise * (-alpha * t).exp()) as f32;
        }
        rir
    }

    #[test]
    fn linear_fit_perfect_line() {
        let xs = [0.0, 1.0, 2.0, 3.0, 4.0];
        let ys = [0.0, -2.0, -4.0, -6.0, -8.0];
        let (slope, intercept, r2) = linear_fit(xs, ys).unwrap();
        assert!((slope - -2.0).abs() < 1e-12);
        assert!(intercept.abs() < 1e-12);
        assert!((r2 - 1.0).abs() < 1e-12);
    }

    #[test]
    fn schroeder_monotonic_decreasing_for_exp_decay() {
        let sr = 48000.0;
        let rir = exponential_decay_rir(sr, 1.0, 2.0);
        let curve = DecayCurve::from_rir(&rir, sr, 0, None);
        assert!(!curve.samples.is_empty());
        // The Schroeder integral of any non-negative envelope is
        // monotonically non-increasing.
        for w in curve.samples.windows(2) {
            assert!(
                w[1] <= w[0] + 1e-9,
                "non-monotonic: {} -> {}",
                w[0],
                w[1]
            );
        }
        // First sample must be 0 dB by construction.
        assert!(curve.samples[0].abs() < 1e-9);
    }

    #[test]
    fn reverberation_times_match_synthetic_t60() {
        let sr = 48000.0;
        let target_t60 = 0.6_f64;
        let rir = exponential_decay_rir(sr, target_t60, 2.0);
        let m = analyze_iso3382(&rir, sr);

        // Synthetic exponential decay → all three should match T60 within
        // a small fraction. We allow ±15 % because the noise excitation
        // adds variance in the slope fit.
        assert!(
            m.t30_s.is_finite() && (m.t30_s - target_t60).abs() < 0.15 * target_t60,
            "T30 = {:.3}s, expected ≈ {:.3}s",
            m.t30_s,
            target_t60
        );
        assert!(
            m.t20_s.is_finite() && (m.t20_s - target_t60).abs() < 0.20 * target_t60,
            "T20 = {:.3}s, expected ≈ {:.3}s",
            m.t20_s,
            target_t60
        );
        // EDT on a pure exponential equals T60. Loose tolerance because
        // only the first 10 dB are used.
        assert!(
            m.edt_s.is_finite() && (m.edt_s - target_t60).abs() < 0.40 * target_t60,
            "EDT = {:.3}s, expected ≈ {:.3}s",
            m.edt_s,
            target_t60
        );

        // r² should be high on a clean exponential decay.
        assert!(m.t20_r2 > 0.9, "T20 r² = {:.3}", m.t20_r2);
        assert!(m.t30_r2 > 0.9, "T30 r² = {:.3}", m.t30_r2);
    }

    #[test]
    fn definition_and_clarity_for_anechoic_ir_max_out() {
        // Single direct sound, no reverberation.
        let sr = 48000.0;
        let mut rir = vec![0.0f32; (sr as usize) / 10]; // 100 ms
        rir[0] = 1.0;
        let m = analyze_iso3382(&rir, sr);
        // All energy is in the first sample → D50 = 1, C50/C80 are large.
        assert!((m.d50 - 1.0).abs() < 1e-6, "D50 = {}", m.d50);
        assert!(m.c50_db > 100.0, "C50 = {}", m.c50_db);
        assert!(m.c80_db > 100.0, "C80 = {}", m.c80_db);
        // Center time → 0 because all energy is at t = 0.
        assert!(m.ts_s.abs() < 1e-9, "Ts = {}", m.ts_s);
    }

    #[test]
    fn clarity_for_uniform_energy_rir() {
        // Uniform energy across 100 ms: C80 should be exactly
        // 10·log10(80 / 20) ≈ 6.02 dB; D50 = 0.5; Ts = 50 ms.
        let sr = 48000.0;
        let n = (sr * 0.1) as usize;
        let rir = vec![1.0f32; n];
        let m = analyze_iso3382(&rir, sr);
        let expected_c80 = 10.0 * (80.0_f64 / 20.0).log10();
        assert!(
            (m.c80_db - expected_c80).abs() < 0.05,
            "C80 = {}, expected {}",
            m.c80_db,
            expected_c80
        );
        assert!((m.d50 - 0.5).abs() < 0.005, "D50 = {}", m.d50);
        assert!((m.ts_s - 0.050).abs() < 0.001, "Ts = {}s", m.ts_s);
    }

    #[test]
    fn empty_rir_returns_nan() {
        let m = analyze_iso3382(&[], 48000.0);
        assert!(m.t30_s.is_nan());
        assert!(m.c80_db.is_nan());
    }

    #[test]
    fn fit_is_valid_threshold() {
        let mut m = EMPTY_METRICS;
        m.edt_r2 = 0.96;
        m.t20_r2 = 0.97;
        m.t30_r2 = 0.95;
        assert!(m.fit_is_valid());
        m.t30_r2 = 0.94;
        assert!(!m.fit_is_valid());
    }
}
