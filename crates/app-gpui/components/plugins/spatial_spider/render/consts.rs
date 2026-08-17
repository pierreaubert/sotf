/// Default reference radius (pixels) above which the polygon labels sit.
pub(super) const LABEL_RADIUS_FACTOR: f32 = 1.07;

/// Speaker dot radius in pixels.
pub(super) const SPEAKER_DOT_PX: f32 = 4.0;

/// Baseline graph viewport height. The graph container is `20rem` at the
/// default 16px root size; using it as the visual-scale reference keeps the
/// custom painter aligned with the rem-based GPUI container at other zooms.
pub(super) const BASE_VIEWPORT_PX: f32 = 320.0;

pub(super) const LABEL_FONT_PX: f32 = 10.0;
pub(super) const LFE_LABEL_FONT_PX: f32 = 9.0;

pub(super) fn viewport_visual_scale(min_dimension: f32) -> f32 {
    (min_dimension / BASE_VIEWPORT_PX).clamp(0.75, 2.0)
}

/// Concentric grid radii (fraction of unit). 1.0 marks 0 dB / |r|=1.
pub(super) const GRID_RING_FRACTIONS: &[f32] = &[0.25, 0.5, 0.75, 1.0];

/// Radial rays drawn every N degrees.
pub(super) const RAY_STEP_DEG: f32 = 30.0;

#[cfg(test)]
mod tests {
    use super::viewport_visual_scale;

    #[test]
    fn viewport_visual_scale_tracks_zoomed_graph_bounds() {
        assert_eq!(viewport_visual_scale(320.0), 1.0);
        assert_eq!(viewport_visual_scale(640.0), 2.0);
        assert_eq!(viewport_visual_scale(80.0), 0.75);
    }
}
