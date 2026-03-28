// ============================================================================
// Variable Channel Linking (0-100%)
// ============================================================================
//
// Blends per-channel detection levels between independent (0%) and fully
// linked (100%). Uses max-based linking: at 100%, all channels see the
// loudest channel's level, preventing overshoot in dynamics processors.
//
// HARD RULES:
// - No allocations — caller provides output slice
// - All functions are #[inline] for hot-path usage

/// Compute linked levels from per-channel levels.
///
/// `link` in `0.0..=1.0`:
/// - `0.0` = fully independent (each channel uses its own level)
/// - `1.0` = fully linked (all channels use the maximum level)
///
/// Formula: `output[ch] = per_channel[ch] * (1 - link) + max_of_all * link`
#[inline]
pub fn compute_linked_levels(per_channel_db: &[f32], link: f32, output: &mut [f32]) {
    debug_assert_eq!(per_channel_db.len(), output.len());
    if per_channel_db.is_empty() {
        return;
    }

    let link = link.clamp(0.0, 1.0);

    if link <= 0.0 {
        output.copy_from_slice(per_channel_db);
        return;
    }

    let max_level = per_channel_db
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);

    if link >= 1.0 {
        output.fill(max_level);
        return;
    }

    let independent_weight = 1.0 - link;
    for (out, &level) in output.iter_mut().zip(per_channel_db.iter()) {
        *out = level * independent_weight + max_level * link;
    }
}

/// Optimized stereo linking.
///
/// Returns `(left_linked, right_linked)`.
#[inline]
pub fn link_stereo(left_db: f32, right_db: f32, link: f32) -> (f32, f32) {
    if link <= 0.0 {
        return (left_db, right_db);
    }
    let max_level = left_db.max(right_db);
    if link >= 1.0 {
        return (max_level, max_level);
    }
    let independent_weight = 1.0 - link;
    (
        left_db * independent_weight + max_level * link,
        right_db * independent_weight + max_level * link,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fully_independent() {
        let levels = [-10.0, -20.0, -15.0];
        let mut output = [0.0; 3];
        compute_linked_levels(&levels, 0.0, &mut output);
        assert_eq!(output, levels);
    }

    #[test]
    fn test_fully_linked() {
        let levels = [-10.0, -20.0, -15.0];
        let mut output = [0.0; 3];
        compute_linked_levels(&levels, 1.0, &mut output);
        assert_eq!(output, [-10.0, -10.0, -10.0]); // max is -10
    }

    #[test]
    fn test_half_linked() {
        let levels = [-10.0, -20.0];
        let mut output = [0.0; 2];
        compute_linked_levels(&levels, 0.5, &mut output);
        // left:  -10 * 0.5 + -10 * 0.5 = -10
        // right: -20 * 0.5 + -10 * 0.5 = -15
        assert!((output[0] - (-10.0)).abs() < 1e-6);
        assert!((output[1] - (-15.0)).abs() < 1e-6);
    }

    #[test]
    fn test_link_stereo() {
        let (l, r) = link_stereo(-6.0, -12.0, 0.5);
        // left:  -6 * 0.5 + -6 * 0.5 = -6
        // right: -12 * 0.5 + -6 * 0.5 = -9
        assert!((l - (-6.0)).abs() < 1e-6);
        assert!((r - (-9.0)).abs() < 1e-6);
    }

    #[test]
    fn test_link_stereo_fully_linked() {
        let (l, r) = link_stereo(-6.0, -12.0, 1.0);
        assert_eq!(l, -6.0);
        assert_eq!(r, -6.0);
    }

    #[test]
    fn test_monotone_linking() {
        // Increasing link should monotonically increase the quieter channel's level
        let left = -6.0;
        let right = -18.0;
        let mut prev_right = right;
        for i in 1..=10 {
            let link = i as f32 / 10.0;
            let (_, r) = link_stereo(left, right, link);
            assert!(r >= prev_right, "Not monotone at link={link}");
            prev_right = r;
        }
    }

    #[test]
    fn test_empty_input() {
        let levels: &[f32] = &[];
        let mut output: [f32; 0] = [];
        compute_linked_levels(levels, 0.5, &mut output);
    }
}
