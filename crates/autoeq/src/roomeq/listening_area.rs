//! Spatial interpolation of audio measurements over a continuous listening area.
//!
//! Pairs with `math_audio_optimisation::continuous_area` to provide a
//! continuous-prior alternative to the discrete `MultiSeatMeasurements`.
//!
//! # Inputs
//!
//! - `K` calibration positions `p_k ∈ R^D` (typically D=2 for an MLP rectangle,
//!   D=1 for a couch line, D=3 for a head-volume sweep).
//! - One [`Curve`] per (subwoofer, position) pair. Each curve must carry phase.
//!
//! # Output
//!
//! At any query point `p` inside the listening area's bounding box, return one
//! [`Curve`] per sub representing the *spatially interpolated* response at `p`.
//!
//! # Method
//!
//! Inverse-distance weighting (IDW) on log-magnitude (dB) and on unwrapped
//! phase (with shortest-arc deltas across calibration points). IDW is
//! parameter-light, basis-free, and well-conditioned for K=4..16 scattered
//! calibration points — the realistic regime for room measurements. For
//! tighter fits with smooth response fields, swap in RBF / kriging here.
//!
//! Phase interpolation uses unwrapped per-bin shortest-arc deltas so that
//! sharp ±180° wraps don't smear into nonsense averages.

use crate::Curve;
use crate::error::{AutoeqError, Result};
use ndarray::Array1;

/// Configuration for the spatial interpolator.
#[derive(Debug, Clone)]
pub struct ListeningAreaInterpolatorConfig {
    /// IDW power exponent. Higher values concentrate weight on the nearest
    /// calibration points; default 2.0 gives a smooth fall-off in 2D rooms.
    pub idw_power: f64,
    /// Distance offset added to the IDW denominator to avoid division by zero
    /// when a query point lands exactly on a calibration point. Has units of
    /// position. Default `1e-9`.
    pub epsilon: f64,
}

impl Default for ListeningAreaInterpolatorConfig {
    fn default() -> Self {
        Self {
            idw_power: 2.0,
            epsilon: 1e-9,
        }
    }
}

/// Calibration grid of measurements at K positions in R^D.
///
/// Use [`ListeningArea::interpolate_at`] to obtain virtual measurements at
/// any query point inside the bounding box of the calibration set.
#[derive(Debug, Clone)]
pub struct ListeningArea<const D: usize> {
    /// Calibration positions in R^D, indexed `[k]`.
    positions: Vec<[f64; D]>,
    /// Per-(sub, position) measurements, indexed `[sub][k]`.
    measurements: Vec<Vec<Curve>>,
    /// Number of subwoofers / drivers.
    num_subs: usize,
    /// Number of calibration positions K.
    num_positions: usize,
    /// Interpolator configuration.
    config: ListeningAreaInterpolatorConfig,
}

impl<const D: usize> ListeningArea<D> {
    /// Construct from a list of calibration positions and per-(sub, position)
    /// measurements.
    ///
    /// `measurements[sub_idx][pos_idx]` is the curve recorded by sub
    /// `sub_idx` at position `positions[pos_idx]`. All curves must:
    /// - share the same frequency grid,
    /// - carry phase data,
    /// - be aligned with the position list (same length per sub).
    pub fn new(
        positions: Vec<[f64; D]>,
        measurements: Vec<Vec<Curve>>,
        config: ListeningAreaInterpolatorConfig,
    ) -> Result<Self> {
        if measurements.is_empty() {
            return Err(AutoeqError::InvalidConfiguration {
                message: "ListeningArea requires at least one subwoofer".into(),
            });
        }
        let num_subs = measurements.len();
        let num_positions = measurements[0].len();
        if num_positions == 0 {
            return Err(AutoeqError::InvalidConfiguration {
                message: "ListeningArea requires at least one calibration position".into(),
            });
        }
        if positions.len() != num_positions {
            return Err(AutoeqError::InvalidConfiguration {
                message: format!(
                    "ListeningArea: {} positions but {} measurements per sub",
                    positions.len(),
                    num_positions
                ),
            });
        }

        // Validate all subs have the same number of positions and that all
        // curves carry phase data and share a frequency grid.
        let reference_freq = &measurements[0][0].freq;
        for (sub_idx, sub) in measurements.iter().enumerate() {
            if sub.len() != num_positions {
                return Err(AutoeqError::InvalidConfiguration {
                    message: format!(
                        "ListeningArea: sub {} has {} positions, expected {}",
                        sub_idx,
                        sub.len(),
                        num_positions
                    ),
                });
            }
            for (pos_idx, curve) in sub.iter().enumerate() {
                if curve.spl.len() != curve.freq.len() {
                    return Err(AutoeqError::InvalidMeasurement {
                        message: format!(
                            "ListeningArea: sub {} pos {} freq/spl length mismatch",
                            sub_idx, pos_idx
                        ),
                    });
                }
                if curve.phase.is_none() {
                    return Err(AutoeqError::InvalidMeasurement {
                        message: format!(
                            "ListeningArea: sub {} pos {} is missing phase data; \
                             complex spatial interpolation requires phase",
                            sub_idx, pos_idx
                        ),
                    });
                }
                if !super::frequency_grid::same_frequency_grid(reference_freq, &curve.freq) {
                    return Err(AutoeqError::InvalidMeasurement {
                        message: format!(
                            "ListeningArea: sub {} pos {} has a different frequency grid \
                             from the reference (sub 0, pos 0)",
                            sub_idx, pos_idx
                        ),
                    });
                }
            }
        }

        Ok(Self {
            positions,
            measurements,
            num_subs,
            num_positions,
            config,
        })
    }

    /// Number of subwoofers / drivers.
    pub fn num_subs(&self) -> usize {
        self.num_subs
    }

    /// Number of calibration positions K.
    pub fn num_positions(&self) -> usize {
        self.num_positions
    }

    /// Calibration positions, indexed `[k]`.
    pub fn positions(&self) -> &[[f64; D]] {
        &self.positions
    }

    /// Axis-aligned bounding box `(lo, hi)` per axis derived from the
    /// calibration positions. Useful as a default for prior bounds.
    pub fn bounding_box(&self) -> [(f64, f64); D] {
        let mut bounds = [(f64::INFINITY, f64::NEG_INFINITY); D];
        for p in &self.positions {
            for i in 0..D {
                if p[i] < bounds[i].0 {
                    bounds[i].0 = p[i];
                }
                if p[i] > bounds[i].1 {
                    bounds[i].1 = p[i];
                }
            }
        }
        bounds
    }

    /// Interpolate per-sub curves at an arbitrary query position `p`.
    ///
    /// Uses inverse-distance weighting on log-magnitude (dB SPL is already
    /// log-magnitude) and on unwrapped phase. Returns one [`Curve`] per sub.
    pub fn interpolate_at(&self, p: [f64; D]) -> Vec<Curve> {
        let weights = self.idw_weights(p);

        let reference_freq = self.measurements[0][0].freq.clone();
        let num_bins = reference_freq.len();

        let mut out: Vec<Curve> = Vec::with_capacity(self.num_subs);
        for sub_idx in 0..self.num_subs {
            let mut spl = Array1::<f64>::zeros(num_bins);
            let mut phase = Array1::<f64>::zeros(num_bins);

            for bin in 0..num_bins {
                // SPL: weighted mean in dB.
                let mut spl_acc = 0.0_f64;
                for (k, &w) in weights.iter().enumerate() {
                    spl_acc += w * self.measurements[sub_idx][k].spl[bin];
                }
                spl[bin] = spl_acc;

                // Phase: unwrap each calibration sample relative to position 0
                // for this bin, weighted-average, then re-wrap to (-180, 180].
                let phase_ref = self.measurements[sub_idx][0]
                    .phase
                    .as_ref()
                    .expect("phase presence validated in ::new")[bin];
                let mut phase_acc = 0.0_f64;
                for (k, &w) in weights.iter().enumerate() {
                    let phase_k = self.measurements[sub_idx][k]
                        .phase
                        .as_ref()
                        .expect("phase presence validated in ::new")[bin];
                    let mut delta = phase_k - phase_ref;
                    delta -= 360.0 * (delta / 360.0).round();
                    phase_acc += w * delta;
                }
                let mut phase_out = phase_ref + phase_acc;
                phase_out -= 360.0 * (phase_out / 360.0).round();
                phase[bin] = phase_out;
            }

            out.push(Curve {
                freq: reference_freq.clone(),
                spl,
                phase: Some(phase),
                ..Default::default()
            });
        }

        out
    }

    fn idw_weights(&self, p: [f64; D]) -> Vec<f64> {
        let eps = self.config.epsilon.max(0.0);
        let power = self.config.idw_power;
        let mut weights: Vec<f64> = Vec::with_capacity(self.num_positions);

        // If the query point is on a calibration point (within eps), collapse
        // to that point exactly to avoid floating-point noise.
        for pk in &self.positions {
            let mut d2 = 0.0_f64;
            for i in 0..D {
                let dx = pk[i] - p[i];
                d2 += dx * dx;
            }
            let d = d2.sqrt();
            if d <= eps {
                let mut w = vec![0.0_f64; self.num_positions];
                let idx = self
                    .positions
                    .iter()
                    .position(|q| q == pk)
                    .expect("pk is one of self.positions");
                w[idx] = 1.0;
                return w;
            }
            weights.push(1.0 / (d + eps).powf(power));
        }

        let total: f64 = weights.iter().sum();
        if total <= 0.0 {
            // Defensive: fall back to uniform weights if something pathological.
            return vec![1.0 / self.num_positions as f64; self.num_positions];
        }
        for w in weights.iter_mut() {
            *w /= total;
        }
        weights
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_curve(freq: Vec<f64>, spl: Vec<f64>, phase: Vec<f64>) -> Curve {
        Curve {
            freq: Array1::from_vec(freq),
            spl: Array1::from_vec(spl),
            phase: Some(Array1::from_vec(phase)),
            ..Default::default()
        }
    }

    #[test]
    fn interpolate_at_calibration_point_returns_calibration_curve() {
        let positions = vec![[0.0, 0.0], [1.0, 0.0], [0.0, 1.0], [1.0, 1.0]];
        let curves: Vec<Curve> = (0..4)
            .map(|k| {
                make_curve(
                    vec![100.0, 1000.0],
                    vec![80.0 + k as f64, 85.0 + k as f64],
                    vec![10.0 * k as f64, 20.0 * k as f64],
                )
            })
            .collect();
        let area: ListeningArea<2> = ListeningArea::new(
            positions.clone(),
            vec![curves.clone()],
            ListeningAreaInterpolatorConfig::default(),
        )
        .expect("constructible");

        for (k, p) in positions.iter().enumerate() {
            let interp = area.interpolate_at(*p);
            assert_eq!(interp.len(), 1);
            assert!(
                (interp[0].spl[0] - curves[k].spl[0]).abs() < 1e-6,
                "at {:?}: spl[0] expected {}, got {}",
                p,
                curves[k].spl[0],
                interp[0].spl[0]
            );
            assert!(
                (interp[0].spl[1] - curves[k].spl[1]).abs() < 1e-6,
                "at {:?}: spl[1] expected {}, got {}",
                p,
                curves[k].spl[1],
                interp[0].spl[1]
            );
        }
    }

    #[test]
    fn interpolate_midpoint_brackets_calibration_values() {
        // Two calibration points with SPL 70 and 90 at the same freq;
        // midpoint should give a value strictly between them under IDW.
        let positions = vec![[0.0], [1.0]];
        let curves = vec![
            make_curve(vec![100.0], vec![70.0], vec![0.0]),
            make_curve(vec![100.0], vec![90.0], vec![0.0]),
        ];
        let area: ListeningArea<1> = ListeningArea::new(
            positions,
            vec![curves],
            ListeningAreaInterpolatorConfig::default(),
        )
        .expect("ok");
        let mid = area.interpolate_at([0.5]);
        assert!(mid[0].spl[0] > 70.0 && mid[0].spl[0] < 90.0);
        // With IDW power=2 and equal distance, midpoint is the arithmetic mean.
        assert!((mid[0].spl[0] - 80.0).abs() < 1e-6);
    }

    #[test]
    fn rejects_curves_without_phase() {
        let positions = vec![[0.0]];
        let no_phase = Curve {
            freq: Array1::from_vec(vec![100.0]),
            spl: Array1::from_vec(vec![80.0]),
            phase: None,
            ..Default::default()
        };
        let err = ListeningArea::<1>::new(
            positions,
            vec![vec![no_phase]],
            ListeningAreaInterpolatorConfig::default(),
        )
        .unwrap_err();
        assert!(format!("{err}").contains("phase"));
    }

    #[test]
    fn rejects_mismatched_position_count() {
        let positions = vec![[0.0], [1.0]];
        let curves = vec![make_curve(vec![100.0], vec![80.0], vec![0.0])];
        let err = ListeningArea::<1>::new(
            positions,
            vec![curves],
            ListeningAreaInterpolatorConfig::default(),
        )
        .unwrap_err();
        assert!(format!("{err}").contains("positions"));
    }

    #[test]
    fn bounding_box_matches_extremes() {
        let positions = vec![[-1.0, 2.0], [3.0, -4.0], [0.0, 0.0]];
        let curves: Vec<Curve> = (0..3)
            .map(|_| make_curve(vec![100.0], vec![80.0], vec![0.0]))
            .collect();
        let area: ListeningArea<2> = ListeningArea::new(
            positions,
            vec![curves],
            ListeningAreaInterpolatorConfig::default(),
        )
        .expect("ok");
        let bb = area.bounding_box();
        assert_eq!(bb[0], (-1.0, 3.0));
        assert_eq!(bb[1], (-4.0, 2.0));
    }

    #[test]
    fn phase_interpolation_handles_wraparound() {
        // Two cal points with phases 170° and -170° at the same freq.
        // The shortest arc midpoint should be 180° (or -180°), not 0°.
        let positions = vec![[0.0], [1.0]];
        let curves = vec![
            make_curve(vec![100.0], vec![80.0], vec![170.0]),
            make_curve(vec![100.0], vec![80.0], vec![-170.0]),
        ];
        let area: ListeningArea<1> = ListeningArea::new(
            positions,
            vec![curves],
            ListeningAreaInterpolatorConfig::default(),
        )
        .expect("ok");
        let mid = area.interpolate_at([0.5]);
        let p = mid[0].phase.as_ref().unwrap()[0];
        // Should be near ±180°, definitely not near 0°.
        assert!(p.abs() > 170.0, "expected near ±180°, got {}", p);
    }
}
