use super::consts::DIALOGUE_SPATIAL_DEADBAND;
use super::consts::DIALOGUE_SPATIAL_MAX_FALL;
use super::consts::DIALOGUE_SPATIAL_MAX_RISE;
use super::consts::DIFFUSENESS_MAX_STEP;
use super::consts::bin_intensity_doa;
use super::diffuseness_and_doa::DiffusenessAndDoa;
use super::diffuseness_and_doa::compute_diffuseness_and_doa;
use super::diffuseness_and_doa::update_diffuseness_state;
use super::misc::ambient_gain_with_controls;
use super::misc::normalize_decorrelation_blend;
use super::misc::principal_eigenvector;
use super::misc::transition_crossfade_weight;
use super::smooth::smooth_dialogue_spatial_control;
use super::smooth::smooth_diffuseness;
use rustfft::num_complex::Complex;

#[test]
fn quadrature_intensity_counts_as_directional_energy() {
    let mut left = vec![Complex::new(0.0, 0.0); 4];
    let mut right = vec![Complex::new(0.0, 0.0); 4];

    left[1] = Complex::new(1.0, 0.0);
    right[1] = Complex::new(0.0, 1.0);

    let analysis = compute_diffuseness_and_doa(&left, &right, 1, 2);

    assert!(
        analysis.diffuseness < 0.01,
        "quadrature intensity should remain directional, got diffuseness {}",
        analysis.diffuseness
    );
    assert!(
        analysis.doa.abs() > 1.0,
        "DOA should preserve the imaginary intensity axis, got {}",
        analysis.doa
    );
    assert!(analysis.reliable);
    assert!(analysis.direct_gain() > 0.99);
    assert!(analysis.ambient_gain() < 0.1);
}

#[test]
fn near_silence_diffuseness_is_unreliable_not_ambient() {
    let left = vec![Complex::new(1e-10, 0.0); 4];
    let right = vec![Complex::new(-1e-10, 0.0); 4];

    let analysis = compute_diffuseness_and_doa(&left, &right, 1, 3);

    assert!(!analysis.reliable);
    assert_eq!(analysis.diffuseness, 0.0);
    assert_eq!(analysis.ambient_gain(), 0.0);
    assert_eq!(analysis.direct_gain(), 1.0);
}

#[test]
fn silence_carries_previous_diffuseness_in_processing_state() {
    let mut previous = 0.35;
    let mut initialized = true;
    let unreliable = DiffusenessAndDoa {
        diffuseness: 0.0,
        doa: 0.0,
        reliable: false,
    };

    let selected = update_diffuseness_state(&mut previous, &mut initialized, unreliable, 1.0);

    assert_eq!(selected, 0.35);
    assert_eq!(previous, 0.35);
    assert!(initialized);
}

#[test]
fn complex_principal_eigenvector_preserves_covariance_phase() {
    let c_xx = 1.0;
    let c_yy = 1.0;
    let c_xy = Complex::new(0.0, 0.8);
    let lambda1 = 1.8;

    let (ev_l, ev_r) = principal_eigenvector(c_xx, c_yy, c_xy, lambda1);

    assert!(
        ev_l.im.abs() > 0.7,
        "principal eigenvector should retain complex covariance phase: {ev_l:?}"
    );

    let lhs_l = ev_l * c_xx + ev_r * c_xy;
    let lhs_r = ev_l * c_xy.conj() + ev_r * c_yy;
    let rhs_l = ev_l * lambda1;
    let rhs_r = ev_r * lambda1;

    assert!((lhs_l - rhs_l).norm() < 1e-5, "left residual too large");
    assert!((lhs_r - rhs_r).norm() < 1e-5, "right residual too large");
}

#[test]
fn ambient_gain_is_energy_preserving_only_before_user_boosts() {
    let diffuseness: f32 = 0.64;
    let base_direct = (1.0 - diffuseness).sqrt();
    let base_ambient = ambient_gain_with_controls(diffuseness, 1.0, 0.0);

    assert!((base_direct * base_direct + base_ambient * base_ambient - 1.0).abs() < 1e-6);

    let boosted_ambient = ambient_gain_with_controls(diffuseness, 1.5, 0.0);
    assert!(boosted_ambient > base_ambient);
}

#[test]
fn transition_crossfade_uses_raised_cosine_shape() {
    let start = 10;
    let width = 8.0;

    let first = transition_crossfade_weight(start, start, width);
    let quarter = transition_crossfade_weight(start + 2, start, width);
    let middle = transition_crossfade_weight(start + 4, start, width);
    let three_quarter = transition_crossfade_weight(start + 6, start, width);
    let last = transition_crossfade_weight(start + 8, start, width);

    assert!(first.abs() < 1e-6);
    assert!((middle - 0.5).abs() < 0.002);
    assert!((last - 1.0).abs() < 0.002);
    assert!(quarter < 0.25, "raised cosine should ease in below linear");
    assert!(
        three_quarter > 0.75,
        "raised cosine should ease out above linear"
    );
}

#[test]
fn decorrelation_blend_normalization_is_unit_magnitude() {
    let blended = Complex::new(0.37, -0.61);
    let normalized = normalize_decorrelation_blend(blended);

    assert!((normalized.norm() - 1.0).abs() < 1e-6);
    assert_eq!(
        normalize_decorrelation_blend(Complex::new(0.0, 0.0)),
        Complex::new(1.0, 0.0)
    );
}

#[test]
fn bin_intensity_doa_varies_per_frequency_bin() {
    let left_a = Complex::new(1.0, 0.0);
    let right_a = Complex::new(0.2, 0.0);
    let left_b = Complex::new(0.2, 0.0);
    let right_b = Complex::new(1.0, 0.0);

    let doa_a = bin_intensity_doa(left_a, right_a).unwrap();
    let doa_b = bin_intensity_doa(left_b, right_b).unwrap();

    assert!(
        (doa_a - doa_b).abs() > 1.0,
        "per-bin secondary DOA should react to per-bin L/R direction"
    );
}

#[test]
fn bin_intensity_doa_ignores_silent_bins() {
    assert!(bin_intensity_doa(Complex::new(0.0, 0.0), Complex::new(0.0, 0.0)).is_none());
}

#[test]
fn dialogue_spatial_control_slew_limits_jitter() {
    let up = smooth_dialogue_spatial_control(0.0, 1.0);
    assert!(
        up <= DIALOGUE_SPATIAL_MAX_RISE + 1e-6,
        "dialogue control rose too quickly: {up}"
    );

    let down = smooth_dialogue_spatial_control(1.0, 0.0);
    assert!(
        1.0 - down <= DIALOGUE_SPATIAL_MAX_FALL + 1e-6,
        "dialogue control fell too quickly: {}",
        1.0 - down
    );

    let stable = smooth_dialogue_spatial_control(0.5, 0.5 + DIALOGUE_SPATIAL_DEADBAND * 0.5);
    assert_eq!(stable, 0.5);
}

#[test]
fn diffuseness_smoothing_limits_block_steps() {
    let up = smooth_diffuseness(0.0, 1.0, 1.0);
    assert!(
        up <= DIFFUSENESS_MAX_STEP + 1e-6,
        "diffuseness rose too quickly: {up}"
    );

    let down = smooth_diffuseness(1.0, 0.0, 1.0);
    assert!(
        1.0 - down <= DIFFUSENESS_MAX_STEP + 1e-6,
        "diffuseness fell too quickly: {}",
        1.0 - down
    );

    let narrow_band = smooth_diffuseness(0.0, 1.0, 0.35);
    assert!(
        narrow_band <= up,
        "narrow-band analysis should not smooth faster than ERB mode"
    );
}
