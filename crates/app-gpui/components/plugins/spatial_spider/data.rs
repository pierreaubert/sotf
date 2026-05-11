//! Pure data transformations for the spatial spider visualizer.
//!
//! Two consumers — the 2D disc and the 3D two-plane view — both take the
//! same speaker layout plus a per-channel scalar (SPL in dBTP or correlation
//! r against a reference channel). This module produces the geometry both
//! views need:
//!
//! - [`compute_polygon_2d`] → ordered `Vec<SpeakerVertex>` sorted by
//!   azimuth, ready for `d3rs::shape::radial::radial_area`.
//! - [`compute_polygon_3d`] → `SpiderPolygon` split into the horizontal
//!   plane (y = 0) and a vertical plane (x = 0) containing height speakers
//!   plus the centre anchor.
//!
//! No GPUI / no wgpu — pure structs and arithmetic so the tests can stay in
//! `cargo test -p app-gpui --lib`.

use sotf_plugins::speaker_config::SpeakerConfig;

/// dBTP range mapped onto the unit radius `[0, 1]`. `MIN_DB` → centre,
/// `MAX_DB` → outer ring. Picked to match the level-meter convention used
/// elsewhere in `app-gpui`.
pub const MIN_DB: f64 = -60.0;
pub const MAX_DB: f64 = 0.0;

/// What the spider polygon's radial axis represents.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpiderMode {
    /// Per-channel true-peak (dBTP) → radius. Polygon is always non-negative.
    Spl,
    /// |Pearson r| from the reference channel → radius. Sign of r is exposed
    /// separately via [`SpeakerVertex::signed_value`] so the renderer can
    /// colour anti-phase vertices red without breaking the convex polygon.
    CorrelationFromRef { ref_channel: usize },
}

/// One speaker, with its anchor direction and current scalar value mapped
/// into a unit radius.
#[derive(Debug, Clone, Copy)]
pub struct SpeakerVertex {
    /// Channel index (matches `SpeakerPosition::channel`).
    pub channel: usize,
    /// Azimuth in degrees (carried through unchanged so renderers can label).
    pub azimuth_deg: f32,
    /// Elevation in degrees.
    pub elevation_deg: f32,
    /// Unit-sphere direction (`SpeakerPosition::to_cartesian`).
    pub direction: [f32; 3],
    /// Radial value in `[0.0, 1.0]`. SPL: clamped dBTP → unit; correlation:
    /// `|r|`. Always non-negative so the polygon stays convex.
    pub radius: f32,
    /// Underlying signed scalar before absolute-value / dB-mapping:
    /// - `Spl` → original dBTP value (unclamped).
    /// - `CorrelationFromRef` → signed Pearson r in `[-1, 1]`.
    pub signed_value: f32,
}

/// Geometry for one frame, split into the two reference planes used by the
/// 3D renderer. The 2D renderer only consumes `horizontal`.
#[derive(Debug, Clone)]
pub struct SpiderPolygon {
    /// Speakers on the horizontal plane (elevation ≈ 0°), sorted by azimuth.
    pub horizontal: Vec<SpeakerVertex>,
    /// Height speakers + the centre anchor projected onto the vertical
    /// (x = 0) plane, sorted by signed angle in that plane.
    pub vertical: Vec<SpeakerVertex>,
    /// LFE channels collected separately — they have no direction.
    pub lfe: Vec<SpeakerVertex>,
}

/// Per-channel input scalars. Length must match `config.total_channels`.
/// Callers feed either `LoudnessData::true_peaks_dbtp` (as f32) or one row
/// of `CorrelationData::matrix`.
#[derive(Debug, Clone, Copy)]
pub enum ChannelMetric<'a> {
    /// True-peak in dBTP per channel. Out-of-range `NaN` / `-inf` is mapped
    /// to radius 0.
    Spl(&'a [f64]),
    /// Pearson r row from the reference channel (matrix slice of length
    /// `channels`). The reference channel itself ends up at radius 1.
    Correlation(&'a [f32]),
}

impl ChannelMetric<'_> {
    fn radius_and_signed(self, channel: usize) -> (f32, f32) {
        match self {
            ChannelMetric::Spl(dbtp) => {
                let v = dbtp.get(channel).copied().unwrap_or(MIN_DB);
                let v = if v.is_finite() { v } else { MIN_DB };
                let norm = ((v - MIN_DB) / (MAX_DB - MIN_DB)).clamp(0.0, 1.0) as f32;
                (norm, v as f32)
            }
            ChannelMetric::Correlation(row) => {
                let r = row.get(channel).copied().unwrap_or(0.0);
                let r = if r.is_finite() { r } else { 0.0 };
                let r = r.clamp(-1.0, 1.0);
                (r.abs(), r)
            }
        }
    }
}

/// Threshold (degrees of elevation) above which a speaker is treated as
/// "height" and routed to the vertical plane. The standard layouts in
/// `speaker_config.rs` use either 0° (floor) or 45° (height), so any
/// threshold in `(0, 45)` works; 5° guards against floating-point noise.
const HEIGHT_ELEVATION_THRESHOLD_DEG: f32 = 5.0;

/// Build the 2D horizontal polygon. Output is sorted by azimuth in
/// ascending order (left → front → right → back) so a closed polygon path
/// traces the speaker ring once.
///
/// LFE speakers are excluded (they have no azimuth).
/// Height speakers are excluded — they belong on the vertical plane.
pub fn compute_polygon_2d(config: &SpeakerConfig, metric: ChannelMetric) -> Vec<SpeakerVertex> {
    let mut out: Vec<SpeakerVertex> = config
        .speakers
        .iter()
        .filter(|s| !s.is_lfe && s.elevation.abs() < HEIGHT_ELEVATION_THRESHOLD_DEG)
        .map(|s| {
            let (radius, signed) = metric.radius_and_signed(s.channel);
            SpeakerVertex {
                channel: s.channel,
                azimuth_deg: s.azimuth,
                elevation_deg: s.elevation,
                direction: s.to_cartesian(),
                radius,
                signed_value: signed,
            }
        })
        .collect();
    // Sort by azimuth so the polygon path is well-defined. Use total_cmp so
    // a stray NaN can't crash the sort.
    out.sort_by(|a, b| a.azimuth_deg.total_cmp(&b.azimuth_deg));
    out
}

/// Build the full 3D split. The vertical plane keeps height speakers plus
/// the centre speaker (azimuth 0, elevation 0) as a shared anchor with the
/// horizontal plane, sorted by their "vertical angle"
/// `atan2(direction_z, direction_y)` so the polygon is well-formed.
pub fn compute_polygon_3d(config: &SpeakerConfig, metric: ChannelMetric) -> SpiderPolygon {
    let mut horizontal = Vec::new();
    let mut vertical = Vec::new();
    let mut lfe = Vec::new();

    for sp in config.speakers {
        let (radius, signed) = metric.radius_and_signed(sp.channel);
        let vertex = SpeakerVertex {
            channel: sp.channel,
            azimuth_deg: sp.azimuth,
            elevation_deg: sp.elevation,
            direction: sp.to_cartesian(),
            radius,
            signed_value: signed,
        };
        if sp.is_lfe {
            lfe.push(vertex);
            continue;
        }
        let is_height = sp.elevation.abs() >= HEIGHT_ELEVATION_THRESHOLD_DEG;
        let is_center = sp.azimuth.abs() < 1.0 && !is_height;
        if !is_height {
            horizontal.push(vertex);
        }
        if is_height || is_center {
            vertical.push(vertex);
        }
    }

    horizontal.sort_by(|a, b| a.azimuth_deg.total_cmp(&b.azimuth_deg));
    // Vertical plane: angle measured around the X axis (i.e. in the YZ plane).
    // Use the speaker's direction in that plane, projecting away X.
    vertical.sort_by(|a, b| {
        let ang_a = a.direction[2].atan2(a.direction[1]);
        let ang_b = b.direction[2].atan2(b.direction[1]);
        ang_a.total_cmp(&ang_b)
    });

    SpiderPolygon {
        horizontal,
        vertical,
        lfe,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_plugins::speaker_config::{get_speaker_config, get_speaker_config_by_channels};

    fn make_uniform_dbtp(n: usize, v: f64) -> Vec<f64> {
        vec![v; n]
    }

    #[test]
    fn spl_polygon_5_1_excludes_lfe_and_sorts_by_azimuth() {
        let cfg = get_speaker_config("5.1").unwrap();
        let dbtp = make_uniform_dbtp(cfg.total_channels, -20.0);
        let poly = compute_polygon_2d(cfg, ChannelMetric::Spl(&dbtp));
        // 5.1 has 5 non-LFE speakers, all at elevation 0.
        assert_eq!(poly.len(), 5);
        // Sorted ascending by azimuth.
        for w in poly.windows(2) {
            assert!(w[0].azimuth_deg <= w[1].azimuth_deg);
        }
        // -20 dBTP maps to (-20 - (-60)) / 60 = 0.6666...
        for v in &poly {
            assert!((v.radius - 0.6667).abs() < 1e-3, "got {}", v.radius);
        }
    }

    #[test]
    fn spl_polygon_5_1_4_routes_heights_to_vertical_plane() {
        let cfg = get_speaker_config("5.1.4").unwrap();
        let dbtp = make_uniform_dbtp(cfg.total_channels, -10.0);
        let poly = compute_polygon_3d(cfg, ChannelMetric::Spl(&dbtp));

        // 5 floor speakers (FL/FR/C/SL/SR) on horizontal; centre also shows
        // up on vertical as the shared anchor.
        assert_eq!(poly.horizontal.len(), 5);
        // 4 height speakers + the centre = 5 on the vertical plane.
        assert_eq!(poly.vertical.len(), 5);
        // LFE captured separately.
        assert_eq!(poly.lfe.len(), 1);

        // Centre is on both planes (label = "C").
        let centre_on_vert = poly
            .vertical
            .iter()
            .any(|v| v.azimuth_deg == 0.0 && v.elevation_deg == 0.0);
        assert!(centre_on_vert, "centre should anchor the vertical plane");

        // No height speakers on the horizontal plane.
        assert!(
            poly.horizontal.iter().all(|v| v.elevation_deg.abs() < 1.0),
            "height speaker leaked onto horizontal plane"
        );
    }

    #[test]
    fn spl_clamps_below_min_db_and_at_zero() {
        let cfg = get_speaker_config("2.0").unwrap();
        // Sub-floor dBTP and over-the-roof dBTP both stay in range.
        let dbtp = vec![-200.0, 12.0];
        let poly = compute_polygon_2d(cfg, ChannelMetric::Spl(&dbtp));
        let radii: Vec<f32> = poly.iter().map(|v| v.radius).collect();
        assert!(radii.iter().all(|r| (0.0..=1.0).contains(r)));
        // One vertex at radius 0 (the -200 dBTP one), one at 1.0 (12 dBTP).
        let mut sorted = radii.clone();
        sorted.sort_by(|a, b| a.total_cmp(b));
        assert!((sorted[0] - 0.0).abs() < 1e-6);
        assert!((sorted[1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn spl_handles_neg_infinity_dbtp_as_silence() {
        let cfg = get_speaker_config("2.0").unwrap();
        let dbtp = vec![f64::NEG_INFINITY, -20.0];
        let poly = compute_polygon_2d(cfg, ChannelMetric::Spl(&dbtp));
        let neg_inf_vertex = poly.iter().find(|v| v.channel == 0).unwrap();
        assert_eq!(neg_inf_vertex.radius, 0.0);
    }

    #[test]
    fn correlation_mode_uses_absolute_value_for_radius_and_keeps_sign_separately() {
        let cfg = get_speaker_config("2.0").unwrap();
        // Row from a 2x2 correlation matrix: ref=L, L↔L = 1, L↔R = -0.7.
        let row: [f32; 2] = [1.0, -0.7];
        let poly = compute_polygon_2d(cfg, ChannelMetric::Correlation(&row));
        // 2.0 has only L (ch 0) and R (ch 1).
        let r_vertex = poly.iter().find(|v| v.channel == 1).unwrap();
        assert!((r_vertex.radius - 0.7).abs() < 1e-6);
        assert!((r_vertex.signed_value - (-0.7)).abs() < 1e-6);
        let l_vertex = poly.iter().find(|v| v.channel == 0).unwrap();
        assert!((l_vertex.radius - 1.0).abs() < 1e-6);
        assert_eq!(l_vertex.signed_value, 1.0);
    }

    #[test]
    fn correlation_mode_clamps_out_of_range_and_handles_nan() {
        let cfg = get_speaker_config("2.0").unwrap();
        let row: [f32; 2] = [2.5, f32::NAN];
        let poly = compute_polygon_2d(cfg, ChannelMetric::Correlation(&row));
        let v0 = poly.iter().find(|v| v.channel == 0).unwrap();
        let v1 = poly.iter().find(|v| v.channel == 1).unwrap();
        assert_eq!(v0.radius, 1.0); // 2.5 clamped to 1
        assert_eq!(v0.signed_value, 1.0);
        assert_eq!(v1.radius, 0.0); // NaN → 0
        assert_eq!(v1.signed_value, 0.0);
    }

    #[test]
    fn polygon_7_1_4_total_speakers_match_config_minus_lfe() {
        let cfg = get_speaker_config("7.1.4").unwrap();
        let dbtp = make_uniform_dbtp(cfg.total_channels, -30.0);
        let poly = compute_polygon_3d(cfg, ChannelMetric::Spl(&dbtp));
        // 7.1.4 = 7 floor + 1 LFE + 4 height = 12 channels.
        assert_eq!(poly.horizontal.len(), 7);
        assert_eq!(poly.lfe.len(), 1);
        // 4 height + 1 centre anchor.
        assert_eq!(poly.vertical.len(), 5);
    }

    #[test]
    fn vertex_directions_are_unit_length() {
        let cfg = get_speaker_config_by_channels(12).unwrap();
        let dbtp = make_uniform_dbtp(cfg.total_channels, -20.0);
        let poly = compute_polygon_3d(cfg, ChannelMetric::Spl(&dbtp));
        for v in poly.horizontal.iter().chain(poly.vertical.iter()) {
            let len =
                (v.direction[0].powi(2) + v.direction[1].powi(2) + v.direction[2].powi(2)).sqrt();
            assert!((len - 1.0).abs() < 1e-5, "non-unit direction {:?}", v);
        }
    }
}
