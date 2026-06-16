use super::consts::ISO_FILTER_COUNT;
use super::loudness_compensation_plugin::LoudnessCompensationPlugin;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;

#[test]
fn test_loudness_basic() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    let mut b = vec![0.5; 1000];
    p.process_in_place(&mut b, &ProcessContext::new(48000, 1000))
        .unwrap();
    assert!(b[999] > 0.0);
}

/// Regression: rebuild_filters() used to call Biquad::new() which resets
/// filter delay state (x1/x2/y1/y2), causing a click artifact on every
/// parameter change. Now it uses update_params() to preserve state.
#[test]
fn test_param_change_no_click() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();

    // Process a block to establish filter state
    let mut b = vec![0.3f32; 4800];
    let ctx = ProcessContext::new(48000, 4800);
    p.process_in_place(&mut b, &ctx).unwrap();
    let last_before = b[4799];

    // Change gain parameter — this should NOT reset filter state
    p.set_parameter(ParameterId::from("low_gain"), ParameterValue::Float(7.0))
        .unwrap();

    // Process another block of the same signal
    let mut b2 = vec![0.3f32; 480];
    p.process_in_place(&mut b2, &ProcessContext::new(48000, 480))
        .unwrap();

    // The first sample after param change should be close to the last
    // sample before the change. A filter state reset would cause a
    // transient (click) where the output jumps to near-zero.
    let first_after = b2[0];
    let jump = (first_after - last_before).abs();
    assert!(
        jump < 0.2,
        "Parameter change caused discontinuity: last={last_before:.4}, first={first_after:.4}, \
             jump={jump:.4}. Filter state may have been reset."
    );
}

/// Verify 3-band topology: 2 lowshelf + 1 peak + 2 highshelf = 5 filters.
/// When mid_enabled is toggled off, the peak filter gain becomes 0 dB (passthrough).
#[test]
fn test_three_band_topology_filter_count() {
    let p = LoudnessCompensationPlugin::new(2, 100.0, 6.0, 10000.0, 6.0);
    // Each channel should have exactly 5 filters
    assert_eq!(
        p.filters[0].len(),
        LoudnessCompensationPlugin::FILTER_COUNT,
        "Channel 0 should have {} filters",
        LoudnessCompensationPlugin::FILTER_COUNT
    );
    assert_eq!(
        p.filters[1].len(),
        LoudnessCompensationPlugin::FILTER_COUNT,
        "Channel 1 should have {} filters",
        LoudnessCompensationPlugin::FILTER_COUNT
    );
}

#[test]
fn test_manual_cascaded_shelf_approximates_requested_passband_gain() {
    let mut p = LoudnessCompensationPlugin::new(1, 200.0, 12.0, 10000.0, 0.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();

    let gain_db: f64 = p.filters[0]
        .iter()
        .map(|filter| filter.log_result(40.0))
        .sum();
    assert!(
        (8.0..=14.0).contains(&(gain_db as f32)),
        "two half-gain shelves should approximate the requested low passband gain; got {gain_db:.2} dB"
    );
}

#[test]
fn test_mid_disabled_sets_peak_gain_zero() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();

    // Confirm mid is enabled by default
    assert!(p.mid_enabled);

    // Disable mid band
    p.set_parameter(
        ParameterId::from("mid_enabled"),
        ParameterValue::Bool(false),
    )
    .unwrap();
    assert!(!p.mid_enabled);

    // The peak filter (index 2) should have gain_db == 0.0.
    // We verify by processing: with mid disabled, a mid-frequency signal
    // should see the same behavior as if the peak band didn't exist.
    // Process two paths: one with mid_enabled=false, one with mid_gain=0.
    let nf = 4800;
    let ctx = ProcessContext::new(48000, nf);

    let signal: Vec<f32> = (0..nf)
        .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 3500.0 * i as f32 / 48000.0).sin())
        .collect();

    // Path A: mid disabled
    let mut buf_a = signal.clone();
    p.process_in_place(&mut buf_a, &ctx).unwrap();

    // Path B: mid enabled but gain=0
    let mut p2 = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p2, 48000).unwrap();
    p2.set_parameter(ParameterId::from("mid_gain"), ParameterValue::Float(0.0))
        .unwrap();
    let mut buf_b = signal.clone();
    p2.process_in_place(&mut buf_b, &ctx).unwrap();

    // RMS of both should be very close
    let rms_a: f32 = (buf_a[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
    let rms_b: f32 = (buf_b[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
    let diff_db = 20.0 * (rms_a / rms_b).log10();
    assert!(
        diff_db.abs() < 0.5,
        "mid_enabled=false should behave like mid_gain=0, but RMS diff is {diff_db:.2} dB"
    );
}

/// Verify that the plugin actually applies gain when configured.
/// With shelving filters active, a low-frequency signal should be processed
/// differently than a mid-frequency signal (spectral shaping occurs).
#[test]
fn test_loudness_comp_applies_gain() {
    // Process a low-frequency signal (within the low shelf)
    let mut p_low = LoudnessCompensationPlugin::new(1, 100.0, 12.0, 10000.0, 12.0);
    ParametricInPlacePlugin::initialize(&mut p_low, 48000).unwrap();

    // Process a mid-frequency signal (outside both shelves)
    let mut p_mid = LoudnessCompensationPlugin::new(1, 100.0, 12.0, 10000.0, 12.0);
    ParametricInPlacePlugin::initialize(&mut p_mid, 48000).unwrap();

    let nf = 9600;
    let sr = 48000.0f32;

    let mut low_buf: Vec<f32> = (0..nf)
        .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 50.0 * i as f32 / sr).sin())
        .collect();
    let mut mid_buf: Vec<f32> = (0..nf)
        .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr).sin())
        .collect();

    let ctx = ProcessContext::new(48000, nf);
    p_low.process_in_place(&mut low_buf, &ctx).unwrap();
    p_mid.process_in_place(&mut mid_buf, &ctx).unwrap();

    // Measure RMS in the settled second half
    let low_rms: f32 =
        (low_buf[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
    let mid_rms: f32 =
        (mid_buf[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();

    // The low-freq signal should be louder relative to mid-freq due to shelf boost
    assert!(
        low_rms > mid_rms * 1.3,
        "Loudness compensation should boost 50 Hz relative to 1 kHz, \
             but low RMS {low_rms:.4} is not significantly greater than mid RMS {mid_rms:.4}"
    );
}

#[test]
fn test_default_mode_is_manual() {
    let p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    assert_eq!(p.mode_index, 0, "Default mode should be Manual (0)");
}

#[test]
fn test_iso226_mode_has_seven_filters_per_channel() {
    let p = LoudnessCompensationPlugin::new(2, 100.0, 6.0, 10000.0, 6.0);
    for ch in 0..2 {
        assert_eq!(
            p.iso_filters[ch].len(),
            ISO_FILTER_COUNT,
            "Channel {ch} should have {ISO_FILTER_COUNT} ISO filters"
        );
    }
}

#[test]
fn test_iso226_mode_equal_levels_passthrough() {
    // When playback_level == reference_level, ISO 226 mode should be near-passthrough
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(1))
        .unwrap();
    p.set_parameter(
        ParameterId::from("playback_level_db"),
        ParameterValue::Float(83.0),
    )
    .unwrap();
    p.set_parameter(
        ParameterId::from("reference_level_db"),
        ParameterValue::Float(83.0),
    )
    .unwrap();

    let nf = 4800;
    let ctx = ProcessContext::new(48000, nf);
    let signal: Vec<f32> = (0..nf)
        .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0).sin())
        .collect();

    let mut buf = signal.clone();
    p.process_in_place(&mut buf, &ctx).unwrap();

    // Measure RMS in the settled half
    let input_rms: f32 =
        (signal[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
    let output_rms: f32 =
        (buf[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
    let diff_db = 20.0 * (output_rms / input_rms).log10();
    assert!(
        diff_db.abs() < 1.0,
        "Equal playback and reference levels should be near-passthrough, got {diff_db:.2} dB difference"
    );
}

#[test]
fn test_iso226_mode_low_volume_boosts_bass() {
    // At lower playback level, bass should be boosted relative to mid
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 0.0, 10000.0, 0.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(1))
        .unwrap();
    p.set_parameter(
        ParameterId::from("playback_level_db"),
        ParameterValue::Float(60.0),
    )
    .unwrap();
    p.set_parameter(
        ParameterId::from("reference_level_db"),
        ParameterValue::Float(83.0),
    )
    .unwrap();

    let nf = 9600;
    let sr = 48000.0f32;
    let ctx = ProcessContext::new(48000, nf);

    // Process a 50 Hz signal
    let mut low_buf: Vec<f32> = (0..nf)
        .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 50.0 * i as f32 / sr).sin())
        .collect();
    p.process_in_place(&mut low_buf, &ctx).unwrap();

    // Process a 1 kHz signal with a fresh plugin at same settings
    let mut p2 = LoudnessCompensationPlugin::new(1, 100.0, 0.0, 10000.0, 0.0);
    ParametricInPlacePlugin::initialize(&mut p2, 48000).unwrap();
    p2.set_parameter(ParameterId::from("mode"), ParameterValue::Int(1))
        .unwrap();
    p2.set_parameter(
        ParameterId::from("playback_level_db"),
        ParameterValue::Float(60.0),
    )
    .unwrap();
    p2.set_parameter(
        ParameterId::from("reference_level_db"),
        ParameterValue::Float(83.0),
    )
    .unwrap();

    let mut mid_buf: Vec<f32> = (0..nf)
        .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr).sin())
        .collect();
    p2.process_in_place(&mut mid_buf, &ctx).unwrap();

    let low_rms: f32 =
        (low_buf[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
    let mid_rms: f32 =
        (mid_buf[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();

    assert!(
        low_rms > mid_rms * 1.2,
        "ISO 226 at low volume should boost bass: low RMS={low_rms:.4} should be > mid RMS={mid_rms:.4} * 1.2"
    );
}

#[test]
fn test_mode_switch_via_set_parameter() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    assert_eq!(p.mode_index, 0);

    p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(1))
        .unwrap();
    assert_eq!(p.mode_index, 1);

    p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(0))
        .unwrap();
    assert_eq!(p.mode_index, 0);
}

#[test]
fn test_get_parameter_new_fields() {
    let p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    assert_eq!(
        p.get_parameter(&ParameterId::from("mode")),
        Some(ParameterValue::Int(0))
    );
    assert_eq!(
        p.get_parameter(&ParameterId::from("playback_level_db")),
        Some(ParameterValue::Float(70.0))
    );
    assert_eq!(
        p.get_parameter(&ParameterId::from("reference_level_db")),
        Some(ParameterValue::Float(83.0))
    );
    assert_eq!(
        p.get_parameter(&ParameterId::from("playback_volume_db")),
        Some(ParameterValue::Float(0.0))
    );
}

#[test]
fn test_auto_mode_applies_compensation() {
    // Auto mode with volume=-20 should produce bass boost
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 0.0, 10000.0, 0.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(2))
        .unwrap();
    p.set_parameter(
        ParameterId::from("reference_level_db"),
        ParameterValue::Float(83.0),
    )
    .unwrap();
    p.set_parameter(
        ParameterId::from("playback_volume_db"),
        ParameterValue::Float(-20.0),
    )
    .unwrap();

    let nf = 9600;
    let sr = 48000.0f32;
    let ctx = ProcessContext::new(48000, nf);

    // Process a 50 Hz signal
    let mut low_buf: Vec<f32> = (0..nf)
        .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 50.0 * i as f32 / sr).sin())
        .collect();
    p.process_in_place(&mut low_buf, &ctx).unwrap();

    // Process a 1 kHz signal with a fresh plugin at same settings
    let mut p2 = LoudnessCompensationPlugin::new(1, 100.0, 0.0, 10000.0, 0.0);
    ParametricInPlacePlugin::initialize(&mut p2, 48000).unwrap();
    p2.set_parameter(ParameterId::from("mode"), ParameterValue::Int(2))
        .unwrap();
    p2.set_parameter(
        ParameterId::from("reference_level_db"),
        ParameterValue::Float(83.0),
    )
    .unwrap();
    p2.set_parameter(
        ParameterId::from("playback_volume_db"),
        ParameterValue::Float(-20.0),
    )
    .unwrap();

    let mut mid_buf: Vec<f32> = (0..nf)
        .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / sr).sin())
        .collect();
    p2.process_in_place(&mut mid_buf, &ctx).unwrap();

    let low_rms: f32 =
        (low_buf[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
    let mid_rms: f32 =
        (mid_buf[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();

    assert!(
        low_rms > mid_rms * 1.2,
        "Auto mode at -20dB volume should boost bass: low RMS={low_rms:.4} should be > mid RMS={mid_rms:.4} * 1.2"
    );
}

#[test]
fn test_auto_mode_zero_volume_flat_response() {
    // Auto mode with volume=0 and reference=83 means estimated_spl = 83 = reference
    // => no compensation (flat response)
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 0.0, 10000.0, 0.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(2))
        .unwrap();
    p.set_parameter(
        ParameterId::from("reference_level_db"),
        ParameterValue::Float(83.0),
    )
    .unwrap();
    p.set_parameter(
        ParameterId::from("playback_volume_db"),
        ParameterValue::Float(0.0),
    )
    .unwrap();

    let nf = 4800;
    let ctx = ProcessContext::new(48000, nf);
    let signal: Vec<f32> = (0..nf)
        .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0).sin())
        .collect();

    let mut buf = signal.clone();
    p.process_in_place(&mut buf, &ctx).unwrap();

    let input_rms: f32 =
        (signal[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
    let output_rms: f32 =
        (buf[nf / 2..].iter().map(|s| s * s).sum::<f32>() / (nf / 2) as f32).sqrt();
    let diff_db = 20.0 * (output_rms / input_rms).log10();
    assert!(
        diff_db.abs() < 1.0,
        "Auto mode at 0dB volume should be near-passthrough, got {diff_db:.2} dB difference"
    );
}

#[test]
fn test_auto_mode_switch_via_set_parameter() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    assert_eq!(p.mode_index, 0);

    p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(2))
        .unwrap();
    assert_eq!(p.mode_index, 2);

    p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(0))
        .unwrap();
    assert_eq!(p.mode_index, 0);
}

/// Bug #1: comp gain must account for inter-band constructive interference.
///
/// When all 7 ISO bands have large gains (e.g. 10 dB each), the combined
/// frequency response can produce ripples above the maximum band-centre gain.
/// The comp smoother target must attenuate enough so the combined peak never
/// exceeds 0 dBFS when the input is at 0 dBFS.
///
/// Specifically, comp_gain_target = 10^(-max_combined_db / 20).  If we only
/// sample at the 7 band centres, we miss the ripple peak, and the smoother
/// target is set too high (not enough attenuation), allowing output > 0 dBFS.
#[test]
fn test_comp_gain_does_not_allow_clipping_in_iso_mode() {
    // Use a large bass boost scenario: playback=40, reference=83 -> big delta at bass
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 0.0, 10000.0, 0.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(1))
        .unwrap();
    p.set_parameter(
        ParameterId::from("playback_level_db"),
        ParameterValue::Float(40.0), // extreme low: large ISO delta
    )
    .unwrap();
    p.set_parameter(
        ParameterId::from("reference_level_db"),
        ParameterValue::Float(83.0),
    )
    .unwrap();

    // Warm-up pass: let the smoother settle from its initial value (1.0) to the
    // compensated target over several blocks.  At 20 ms time constant, 10 time
    // constants (200 ms = 9600 samples) is enough for >99.99% convergence.
    let nf = 9600;
    let ctx = ProcessContext::new(48000, nf);
    let warmup: Vec<f32> = (0..nf)
        .map(|i| (2.0 * std::f32::consts::PI * 50.0 * i as f32 / 48000.0).sin())
        .collect();
    let mut warm_buf = warmup.clone();
    p.process_in_place(&mut warm_buf, &ctx).unwrap();

    // Second pass with smoother fully settled: peak must not exceed 0 dBFS
    let mut buf: Vec<f32> = (0..nf)
        .map(|i| (2.0 * std::f32::consts::PI * 50.0 * i as f32 / 48000.0).sin())
        .collect();
    p.process_in_place(&mut buf, &ctx).unwrap();

    // With proper comp gain the peak must not exceed 1.0 (0 dBFS) once settled
    let peak = buf.iter().map(|s| s.abs()).fold(0.0_f32, f32::max);
    assert!(
        peak <= 1.0 + 1e-3,
        "comp_gain under-attenuated: peak = {peak:.4} > 1.0 (clipping) after smoother settled"
    );
}

/// Bug #2: auto-gain measurement must happen every block, not every 10.
///
/// With the old bug, `do_measure` was only true every 10 blocks.  All auto-gain
/// measurement and cache writes were skipped otherwise.  So for 9 blocks after
/// each measurement cycle, the cache held stale data.
///
/// The fix makes measurement happen every block.  To observe a measurable difference
/// we compare what happens after 9 blocks of silence followed by 1 block of loud
/// signal vs 10 blocks of loud signal.  With the old code the 10th block overwrites;
/// with the fix every block updates.
///
/// Practical test: process enough audio for EBU R128 momentary measurement to
/// accumulate (≥400 ms = ~19200 samples), then verify that `input_lufs` reflects
/// the actual signal level — not the plugin default (-120.0).
#[test]
fn test_auto_gain_measurement_not_stale_after_one_block() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 0.0, 10000.0, 0.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    p.set_parameter(
        ParameterId::from("auto_gain_enabled"),
        ParameterValue::Bool(true),
    )
    .unwrap();

    // Process 9 small blocks (9 * 512 = 4608 samples) of silence.
    // With the old bug, the cache update counter fires on the 10th block only.
    let nf = 512;
    let ctx = ProcessContext::new(48000, nf);
    for _ in 0..9 {
        let mut buf = vec![0.0_f32; nf];
        p.process_in_place(&mut buf, &ctx).unwrap();
    }

    // Now feed loud audio for enough blocks to fill the EBU R128 400ms window
    // (~19200 samples = 38 blocks of 512).  With the fix, measurement and cache
    // update happen on every block, so after these blocks input_lufs is live.
    let loud_nf = 19200;
    let loud_ctx = ProcessContext::new(48000, loud_nf);
    let mut loud_buf: Vec<f32> = (0..loud_nf)
        .map(|i| if i % 2 == 0 { 0.5_f32 } else { -0.5_f32 })
        .collect();
    p.process_in_place(&mut loud_buf, &loud_ctx).unwrap();

    let data_arc = p.get_data().expect("auto_gain should produce data");
    let ag_data = data_arc
        .downcast_ref::<sotf_host::auto_gain::AutoGainData>()
        .expect("data should be AutoGainData");
    // After 400ms of loud audio the EBU momentary measurement must be well above
    // the plugin default of -120.0 dB.  With the old throttled code the cache
    // holds the measurement from block 10 (still all silence), so input_lufs
    // would remain near -inf / -120.
    assert!(
        ag_data.input_lufs > -40.0,
        "input_lufs should reflect loud signal after 400ms, got {:.2} dB (still default/stale?)",
        ag_data.input_lufs
    );
}

/// Bug #3: in Post mode, output measurement must see post-compensation level.
///
/// The AutoGain feedback loop sets its next gain target via:
///   `target = input_lufs - output_lufs`
///
/// If `ag.measure_output` is called BEFORE `ag.apply_compensation` (the bug),
/// `output_lufs` reflects the signal BEFORE the AutoGain's own gain is applied.
/// When the AutoGain is boosting (gain_linear > 1.0), the measurement will be
/// lower than the actual output, causing the feedback loop to increase gain further.
/// This positive feedback drives gain to `max_gain_db` → audible pumping.
///
/// The fix applies `ag.apply_compensation` first, then calls `ag.measure_output`.
///
/// This test verifies:
///   (a) Both `input_lufs` and `output_lufs` are finite after sufficient audio.
///   (b) The difference is bounded by the AutoGain's max_gain_db range.
///
/// Full regression of the feedback instability requires fine-grained control over
/// the EBU R128 internal state, which is out of scope here.  The code fix is
/// verified by code review (apply then measure).
#[test]
fn test_post_mode_output_measurement_after_compensation() {
    let params = crate::LoudnessCompensationPluginParams {
        auto_gain_enabled: true,
        auto_gain_position: "post".to_string(),
        auto_gain_max_db: 12.0,
        auto_gain_smoothing_ms: 5.0,
        ..Default::default()
    };
    let mut p = LoudnessCompensationPlugin::from_params(1, params).unwrap();
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();

    let nf = 4800; // 100ms per block
    let ctx = ProcessContext::new(48000, nf);
    let signal: Vec<f32> = (0..nf)
        .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0).sin())
        .collect();

    // 10 blocks = 1 second; enough for EBU R128 momentary window to fill
    for _ in 0..10 {
        let mut buf = signal.clone();
        p.process_in_place(&mut buf, &ctx).unwrap();
    }

    let data_arc = p.get_data().expect("auto_gain should produce data");
    let ag_data = data_arc
        .downcast_ref::<sotf_host::auto_gain::AutoGainData>()
        .expect("data should be AutoGainData");

    // Both measurements must be finite after 1 second
    assert!(
        ag_data.input_lufs.is_finite(),
        "input_lufs must be finite after 1s, got {}",
        ag_data.input_lufs
    );
    assert!(
        ag_data.output_lufs.is_finite(),
        "output_lufs must be finite after 1s, got {}",
        ag_data.output_lufs
    );

    // In steady state, |output_lufs - input_lufs| must be within AutoGain's range.
    // This bound would be violated if the feedback loop ran away due to the
    // wrong measurement order.
    let diff = (ag_data.output_lufs - ag_data.input_lufs).abs();
    let max_gain_db = 12.0_f64;
    assert!(
        diff <= max_gain_db + 1.0,
        "output_lufs ({:.2}) and input_lufs ({:.2}) should be within {:.1} dB \
             (Post mode, Bug #3 fix: measure AFTER compensation); diff = {:.2}",
        ag_data.output_lufs,
        ag_data.input_lufs,
        max_gain_db + 1.0,
        diff
    );
}

/// Bug #5 + #7: manual mode should not rebuild ISO filters or call
/// `maybe_rebuild_auto_filters` on every block.
///
/// Indirect verification: setting `playback_level_db` in manual mode (mode=0)
/// must not panic or corrupt internal state.  If it incorrectly rebuilt ISO
/// filters AND mode were 0, the iso_filters would be recomputed — harmless but
/// indicates the guard is absent.  We test stability by processing after the
/// parameter change.
#[test]
fn test_manual_mode_level_change_does_not_corrupt() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    assert_eq!(p.mode_index, 0);

    // Change ISO-related params in manual mode — must be a no-op for filter bank
    p.set_parameter(
        ParameterId::from("playback_level_db"),
        ParameterValue::Float(60.0),
    )
    .unwrap();
    p.set_parameter(
        ParameterId::from("reference_level_db"),
        ParameterValue::Float(83.0),
    )
    .unwrap();

    // Process must succeed and produce finite output
    let nf = 480;
    let mut buf: Vec<f32> = (0..nf).map(|i| 0.2 * (i as f32 / 48.0).sin()).collect();
    p.process_in_place(&mut buf, &ProcessContext::new(48000, nf))
        .unwrap();
    assert!(
        buf.iter().all(|s| s.is_finite()),
        "output should be finite after parameter change in manual mode"
    );
}

// ============================================================================
// set_parameter / get_parameter round-trip coverage
// ============================================================================

#[test]
fn test_set_get_parameter_all_fields() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();

    // low_gain
    p.set_parameter(ParameterId::from("low_gain"), ParameterValue::Float(8.5))
        .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("low_gain")),
        Some(ParameterValue::Float(8.5))
    );

    // high_gain
    p.set_parameter(ParameterId::from("high_gain"), ParameterValue::Float(-3.0))
        .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("high_gain")),
        Some(ParameterValue::Float(-3.0))
    );

    // low_freq
    p.set_parameter(ParameterId::from("low_freq"), ParameterValue::Float(150.0))
        .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("low_freq")),
        Some(ParameterValue::Float(150.0))
    );

    // high_freq
    p.set_parameter(
        ParameterId::from("high_freq"),
        ParameterValue::Float(12000.0),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("high_freq")),
        Some(ParameterValue::Float(12000.0))
    );

    // mid_enabled
    p.set_parameter(
        ParameterId::from("mid_enabled"),
        ParameterValue::Bool(false),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("mid_enabled")),
        Some(ParameterValue::Bool(false))
    );

    // mid_freq
    p.set_parameter(ParameterId::from("mid_freq"), ParameterValue::Float(4000.0))
        .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("mid_freq")),
        Some(ParameterValue::Float(4000.0))
    );

    // mid_gain
    p.set_parameter(ParameterId::from("mid_gain"), ParameterValue::Float(-2.0))
        .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("mid_gain")),
        Some(ParameterValue::Float(-2.0))
    );

    // mid_q
    p.set_parameter(ParameterId::from("mid_q"), ParameterValue::Float(1.2))
        .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("mid_q")),
        Some(ParameterValue::Float(1.2))
    );

    // auto_gain_enabled
    p.set_parameter(
        ParameterId::from("auto_gain_enabled"),
        ParameterValue::Bool(true),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("auto_gain_enabled")),
        Some(ParameterValue::Bool(true))
    );

    // auto_gain_max_db
    p.set_parameter(
        ParameterId::from("auto_gain_max_db"),
        ParameterValue::Float(6.0),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("auto_gain_max_db")),
        Some(ParameterValue::Float(6.0))
    );

    // auto_gain_smoothing_ms
    p.set_parameter(
        ParameterId::from("auto_gain_smoothing_ms"),
        ParameterValue::Float(50.0),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("auto_gain_smoothing_ms")),
        Some(ParameterValue::Float(50.0))
    );

    // auto_gain_position
    p.set_parameter(
        ParameterId::from("auto_gain_position"),
        ParameterValue::String("pre".to_string()),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("auto_gain_position")),
        Some(ParameterValue::String("pre".to_string()))
    );

    // mode
    p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(1))
        .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("mode")),
        Some(ParameterValue::Int(1))
    );

    // playback_level_db
    p.set_parameter(
        ParameterId::from("playback_level_db"),
        ParameterValue::Float(65.0),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("playback_level_db")),
        Some(ParameterValue::Float(65.0))
    );

    // reference_level_db
    p.set_parameter(
        ParameterId::from("reference_level_db"),
        ParameterValue::Float(80.0),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("reference_level_db")),
        Some(ParameterValue::Float(80.0))
    );

    // playback_volume_db
    p.set_parameter(
        ParameterId::from("playback_volume_db"),
        ParameterValue::Float(-10.0),
    )
    .unwrap();
    assert_eq!(
        p.get_parameter(&ParameterId::from("playback_volume_db")),
        Some(ParameterValue::Float(-10.0))
    );
}

#[test]
fn test_get_parameter_unknown_returns_none() {
    let p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    assert_eq!(p.get_parameter(&ParameterId::from("nonexistent")), None);
}

// ============================================================================
// set_parameter error paths
// ============================================================================

#[test]
fn test_set_parameter_type_errors_preserve_state() {
    let cases = [
        ("unknown_param", ParameterValue::Float(1.0)),
        ("mid_enabled", ParameterValue::Float(1.0)),
        (
            "auto_gain_enabled",
            ParameterValue::String("true".to_string()),
        ),
        ("auto_gain_max_db", ParameterValue::Int(5)),
        ("auto_gain_smoothing_ms", ParameterValue::Int(50)),
        ("auto_gain_position", ParameterValue::Int(0)),
        ("mode", ParameterValue::String("auto".to_string())),
    ];

    for (param, value) in cases {
        let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
        let original_low_gain = p.low_gain;
        let original_mid_enabled = p.mid_enabled;
        let original_mode = p.mode_index;
        let original_auto_gain_present = p.auto_gain.is_some();

        let result = p.set_parameter(ParameterId::from(param), value);
        assert!(result.is_err(), "{param} should reject invalid type/value");
        assert_eq!(p.low_gain, original_low_gain, "{param} mutated low_gain");
        assert_eq!(
            p.mid_enabled, original_mid_enabled,
            "{param} mutated mid_enabled"
        );
        assert_eq!(p.mode_index, original_mode, "{param} mutated mode");
        assert_eq!(
            p.auto_gain.is_some(),
            original_auto_gain_present,
            "{param} changed auto_gain allocation"
        );
    }
}

#[test]
fn test_set_parameter_mode_out_of_range_rejected_by_validation() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    assert_eq!(p.mode_index, 0);
    let result = p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(5));
    assert!(
        result.is_err(),
        "mode out of range should be rejected by validation"
    );
    assert_eq!(p.mode_index, 0);
}

#[test]
fn test_set_parameter_mode_float_rejected_by_validation() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    let result = p.set_parameter(ParameterId::from("mode"), ParameterValue::Float(2.0));
    assert!(
        result.is_err(),
        "mode as float should be rejected by validation (mode is int/choice)"
    );
}

#[test]
fn test_set_parameter_non_finite_float_rejected_by_validation() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    let orig = p.low_gain;
    let result = p.set_parameter(
        ParameterId::from("low_gain"),
        ParameterValue::Float(f32::NAN),
    );
    assert!(result.is_err(), "NaN should be rejected by validation");
    assert_eq!(p.low_gain, orig);

    let result2 = p.set_parameter(
        ParameterId::from("low_gain"),
        ParameterValue::Float(f32::INFINITY),
    );
    assert!(
        result2.is_err(),
        "Infinity should be rejected by validation"
    );
    assert_eq!(p.low_gain, orig);
}

// ============================================================================
// set_parameter state changes (auto_gain creation / removal / rebuild triggers)
// ============================================================================

#[test]
fn test_set_parameter_auto_gain_position_pre_creates_auto_gain() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    assert!(p.auto_gain.is_none());

    p.set_parameter(
        ParameterId::from("auto_gain_position"),
        ParameterValue::String("pre".to_string()),
    )
    .unwrap();

    assert!(p.auto_gain.is_some());
    assert!(p.auto_gain_enabled);
    assert_eq!(
        p.auto_gain_position,
        super::auto_gain_position::AutoGainPosition::Pre
    );
}

#[test]
fn test_set_parameter_auto_gain_position_disabled_removes_auto_gain() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();

    // Enable first
    p.set_parameter(
        ParameterId::from("auto_gain_enabled"),
        ParameterValue::Bool(true),
    )
    .unwrap();
    assert!(p.auto_gain.is_some());

    // Disable via position
    p.set_parameter(
        ParameterId::from("auto_gain_position"),
        ParameterValue::String("disabled".to_string()),
    )
    .unwrap();

    assert!(p.auto_gain.is_none());
    assert!(!p.auto_gain_enabled);
}

#[test]
fn test_set_parameter_auto_gain_max_db_updates_existing_auto_gain() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    p.set_parameter(
        ParameterId::from("auto_gain_enabled"),
        ParameterValue::Bool(true),
    )
    .unwrap();

    p.set_parameter(
        ParameterId::from("auto_gain_max_db"),
        ParameterValue::Float(6.0),
    )
    .unwrap();
    assert_eq!(p.auto_gain_max_db, 6.0);
}

#[test]
fn test_set_parameter_auto_gain_max_db_without_auto_gain_no_panic() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    p.set_parameter(
        ParameterId::from("auto_gain_max_db"),
        ParameterValue::Float(6.0),
    )
    .unwrap();
    assert_eq!(p.auto_gain_max_db, 6.0);
}

#[test]
fn test_set_parameter_auto_gain_smoothing_ms_updates_existing_auto_gain() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    p.set_parameter(
        ParameterId::from("auto_gain_enabled"),
        ParameterValue::Bool(true),
    )
    .unwrap();

    p.set_parameter(
        ParameterId::from("auto_gain_smoothing_ms"),
        ParameterValue::Float(25.0),
    )
    .unwrap();
    assert_eq!(p.auto_gain_smoothing_ms, 25.0);
}

#[test]
fn test_set_parameter_auto_gain_smoothing_ms_without_auto_gain_no_panic() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    p.set_parameter(
        ParameterId::from("auto_gain_smoothing_ms"),
        ParameterValue::Float(25.0),
    )
    .unwrap();
    assert_eq!(p.auto_gain_smoothing_ms, 25.0);
}

#[test]
fn test_set_parameter_playback_volume_db_manual_mode_no_rebuild() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    assert_eq!(p.mode_index, 0);

    // Save iso_deltas before
    let deltas_before = p.iso_deltas;

    p.set_parameter(
        ParameterId::from("playback_volume_db"),
        ParameterValue::Float(-20.0),
    )
    .unwrap();

    // In manual mode, iso_deltas should not change
    assert_eq!(p.iso_deltas[0].0, deltas_before[0].0);
    assert_eq!(p.iso_deltas[0].1, deltas_before[0].1);
}

#[test]
fn test_set_parameter_playback_volume_db_auto_mode_triggers_rebuild() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(2))
        .unwrap();
    p.set_parameter(
        ParameterId::from("reference_level_db"),
        ParameterValue::Float(83.0),
    )
    .unwrap();

    let last_vol_before = p.last_auto_volume_db;
    p.set_parameter(
        ParameterId::from("playback_volume_db"),
        ParameterValue::Float(-20.0),
    )
    .unwrap();

    // A large volume change should trigger rebuild
    assert_eq!(p.last_auto_volume_db, -20.0);
    assert_ne!(p.last_auto_volume_db, last_vol_before);
}

#[test]
fn test_set_parameter_playback_level_db_auto_mode_no_panic() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(2))
        .unwrap();

    // playback_level_db in auto mode forces a rebuild but does not change
    // the coefficients (auto mode uses reference_level_db + playback_volume_db).
    // This test just verifies no panic and the value is stored.
    p.set_parameter(
        ParameterId::from("playback_level_db"),
        ParameterValue::Float(60.0),
    )
    .unwrap();
    assert_eq!(p.playback_level_db, 60.0);
}

#[test]
fn test_set_parameter_reference_level_db_auto_mode_triggers_rebuild() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(2))
        .unwrap();

    // Set a non-zero playback volume so estimated_spl != reference_level_db
    p.set_parameter(
        ParameterId::from("playback_volume_db"),
        ParameterValue::Float(-10.0),
    )
    .unwrap();

    let deltas_before = p.iso_deltas;
    p.set_parameter(
        ParameterId::from("reference_level_db"),
        ParameterValue::Float(70.0),
    )
    .unwrap();

    assert_ne!(
        p.iso_deltas[0].1, deltas_before[0].1,
        "auto mode should rebuild iso_filters when reference_level_db changes"
    );
}

#[test]
fn test_set_parameter_playback_level_db_iso_mode_rebuilds() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(1))
        .unwrap();

    let deltas_before = p.iso_deltas;
    p.set_parameter(
        ParameterId::from("playback_level_db"),
        ParameterValue::Float(50.0),
    )
    .unwrap();

    // ISO mode should rebuild iso_filters
    assert_ne!(p.iso_deltas[0].1, deltas_before[0].1);
}

#[test]
fn test_set_parameter_playback_level_db_manual_mode_no_rebuild() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    assert_eq!(p.mode_index, 0);

    let deltas_before = p.iso_deltas;
    p.set_parameter(
        ParameterId::from("playback_level_db"),
        ParameterValue::Float(50.0),
    )
    .unwrap();

    assert_eq!(p.iso_deltas[0].1, deltas_before[0].1);
}

// ============================================================================
// initialize
// ============================================================================

#[test]
fn test_initialize_different_sample_rate() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    assert_eq!(p.sample_rate, 48000);
    ParametricInPlacePlugin::initialize(&mut p, 96000).unwrap();
    assert_eq!(p.sample_rate, 96000);
}

#[test]
fn test_initialize_rebuilds_filters() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();

    // Change sample rate
    ParametricInPlacePlugin::initialize(&mut p, 44100).unwrap();

    // Filters should have been rebuilt for new sample rate
    let mut b = vec![0.5f32; 480];
    p.process_in_place(&mut b, &ProcessContext::new(44100, 480))
        .unwrap();
    assert!(b.iter().all(|s| s.is_finite()));
}

// ============================================================================
// reset
// ============================================================================

#[test]
fn test_reset_clears_filter_state() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();

    // Process to establish state
    let mut b = vec![0.3f32; 4800];
    let ctx = ProcessContext::new(48000, 4800);
    p.process_in_place(&mut b, &ctx).unwrap();
    let last_before = b[4799];

    // Reset should clear filter delay state
    ParametricInPlacePlugin::reset(&mut p);

    // Process another block — first sample should differ because state was reset
    let mut b2 = vec![0.3f32; 480];
    p.process_in_place(&mut b2, &ProcessContext::new(48000, 480))
        .unwrap();

    // After reset, the first output should NOT match the continuous stream.
    // The exact value isn't critical; we just need a visible discontinuity.
    let jump = (b2[0] - last_before).abs();
    assert!(
        jump > 0.01,
        "reset should clear filter state: last={last_before:.4}, first_after_reset={:.4}, jump={jump:.4}",
        b2[0]
    );
}

// ============================================================================
// process_in_place — Pre mode, multi-channel, edge cases
// ============================================================================

#[test]
fn test_process_in_place_pre_mode() {
    let params = crate::LoudnessCompensationPluginParams {
        auto_gain_enabled: true,
        auto_gain_position: "pre".to_string(),
        auto_gain_max_db: 12.0,
        auto_gain_smoothing_ms: 5.0,
        ..Default::default()
    };
    let mut p = LoudnessCompensationPlugin::from_params(1, params).unwrap();
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();

    let nf = 4800;
    let ctx = ProcessContext::new(48000, nf);
    let signal: Vec<f32> = (0..nf)
        .map(|i| 0.3 * (2.0 * std::f32::consts::PI * 1000.0 * i as f32 / 48000.0).sin())
        .collect();

    // Process several blocks
    for _ in 0..10 {
        let mut buf = signal.clone();
        p.process_in_place(&mut buf, &ctx).unwrap();
    }

    let data_arc = p.get_data().expect("auto_gain should produce data");
    let ag_data = data_arc
        .downcast_ref::<sotf_host::auto_gain::AutoGainData>()
        .expect("data should be AutoGainData");

    assert!(ag_data.input_lufs.is_finite(), "input_lufs must be finite");
    assert!(
        ag_data.output_lufs.is_finite(),
        "output_lufs must be finite"
    );
}

#[test]
fn test_process_in_place_disabled_mode() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();

    let nf = 480;
    let ctx = ProcessContext::new(48000, nf);
    let mut b: Vec<f32> = (0..nf).map(|i| 0.2 * (i as f32 / 48.0).sin()).collect();
    p.process_in_place(&mut b, &ctx).unwrap();
    assert!(b.iter().all(|s| s.is_finite()));
}

#[test]
fn test_process_in_place_stereo() {
    let mut p = LoudnessCompensationPlugin::new(2, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();

    let nf = 480;
    let ctx = ProcessContext::new(48000, nf);
    // Interleaved stereo: [L0, R0, L1, R1, ...]
    let mut b: Vec<f32> = (0..nf * 2).map(|i| 0.2 * (i as f32 / 48.0).sin()).collect();
    p.process_in_place(&mut b, &ctx).unwrap();
    assert!(b.iter().all(|s| s.is_finite()));
}

#[test]
fn test_process_in_place_empty_buffer() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();

    let mut b: Vec<f32> = vec![];
    let ctx = ProcessContext::new(48000, 0);
    let result = p.process_in_place(&mut b, &ctx);
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 0);
}

#[test]
fn test_process_in_place_single_frame() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();

    let mut b = vec![0.5f32];
    let ctx = ProcessContext::new(48000, 1);
    p.process_in_place(&mut b, &ctx).unwrap();
    assert!(b[0].is_finite());
}

// ============================================================================
// get_data
// ============================================================================

#[test]
fn test_get_data_none_without_auto_gain() {
    let p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    assert!(p.get_data().is_none());
}

#[test]
fn test_get_data_some_with_auto_gain() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    p.set_parameter(
        ParameterId::from("auto_gain_enabled"),
        ParameterValue::Bool(true),
    )
    .unwrap();
    assert!(p.get_data().is_some());
}

// ============================================================================
// from_params edge cases
// ============================================================================

#[test]
fn test_from_params_auto_gain_disabled_overrides_position() {
    let params = crate::LoudnessCompensationPluginParams {
        auto_gain_enabled: false,
        auto_gain_position: "pre".to_string(),
        ..Default::default()
    };
    let p = LoudnessCompensationPlugin::from_params(1, params).unwrap();
    assert!(!p.auto_gain_enabled);
    assert!(p.auto_gain.is_none());
    assert_eq!(
        p.auto_gain_position,
        super::auto_gain_position::AutoGainPosition::Disabled
    );
}

#[test]
fn test_from_params_pre_position() {
    let params = crate::LoudnessCompensationPluginParams {
        auto_gain_enabled: true,
        auto_gain_position: "pre".to_string(),
        ..Default::default()
    };
    let p = LoudnessCompensationPlugin::from_params(1, params).unwrap();
    assert!(p.auto_gain_enabled);
    assert!(p.auto_gain.is_some());
    assert_eq!(
        p.auto_gain_position,
        super::auto_gain_position::AutoGainPosition::Pre
    );
}

// ============================================================================
// maybe_rebuild_auto_filters edge cases
// ============================================================================

#[test]
fn test_maybe_rebuild_skips_small_volume_change() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    p.set_parameter(ParameterId::from("mode"), ParameterValue::Int(2))
        .unwrap();
    p.set_parameter(
        ParameterId::from("reference_level_db"),
        ParameterValue::Float(83.0),
    )
    .unwrap();
    p.set_parameter(
        ParameterId::from("playback_volume_db"),
        ParameterValue::Float(-10.0),
    )
    .unwrap();

    // Force a rebuild so last_auto_volume_db is current
    p.maybe_rebuild_auto_filters();
    assert_eq!(p.last_auto_volume_db, -10.0);

    // Tiny change (< 0.5 dB) should not trigger rebuild
    p.playback_volume_db = -10.3;
    let deltas_before = p.iso_deltas;
    p.maybe_rebuild_auto_filters();
    assert_eq!(p.last_auto_volume_db, -10.0);
    assert_eq!(p.iso_deltas[0].1, deltas_before[0].1);
}

#[test]
fn test_maybe_rebuild_skips_non_auto_mode() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    assert_eq!(p.mode_index, 0);

    let deltas_before = p.iso_deltas;
    p.playback_volume_db = -20.0;
    p.maybe_rebuild_auto_filters();

    // Nothing should change in manual mode
    assert_eq!(p.iso_deltas[0].1, deltas_before[0].1);
}

// ============================================================================
// update_comp_gain_smoother edge cases
// ============================================================================

#[test]
fn test_update_comp_gain_smoother_manual_mode_mid_disabled() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();
    p.set_parameter(
        ParameterId::from("mid_enabled"),
        ParameterValue::Bool(false),
    )
    .unwrap();

    // Manual mode with mid disabled: max gain should be max of low/high abs
    let expected = p.low_gain.abs().max(p.high_gain.abs());
    let target = 10.0_f32.powf(-expected / 20.0);
    assert!(
        (p.comp_gain_smoother[0].target() - target).abs() < 1e-4,
        "comp_gain target mismatch"
    );
}

#[test]
fn test_process_sample_all_modes() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();

    // Manual mode
    let s_manual = p.process_sample(0, 0.5);
    assert!(s_manual.is_finite());

    // ISO 226 mode
    p.mode_index = 1;
    let s_iso = p.process_sample(0, 0.5);
    assert!(s_iso.is_finite());

    // Auto mode
    p.mode_index = 2;
    let s_auto = p.process_sample(0, 0.5);
    assert!(s_auto.is_finite());
}

#[test]
fn test_info() {
    let p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    let info = ParametricInPlacePlugin::info(&p);
    assert_eq!(info.name, "Loudness Compensation");
    assert_eq!(info.version, "3.0.0");
    assert_eq!(info.author, "Sotf");
}

#[test]
fn test_channels() {
    let p1 = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    assert_eq!(ParametricInPlacePlugin::channels(&p1), 1);

    let p2 = LoudnessCompensationPlugin::new(2, 100.0, 6.0, 10000.0, 6.0);
    assert_eq!(ParametricInPlacePlugin::channels(&p2), 2);
}

#[test]
fn test_parameters_returns_clone() {
    let p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    let params = ParametricInPlacePlugin::parameters(&p);
    assert!(!params.is_empty());
    // Should contain at least the expected parameters
    assert!(params.iter().any(|param| param.id.0 == "low_gain"));
    assert!(params.iter().any(|param| param.id.0 == "mode"));
}

#[test]
fn test_rebuild_cached_parameters_updates_values() {
    let mut p = LoudnessCompensationPlugin::new(1, 100.0, 6.0, 10000.0, 6.0);
    ParametricInPlacePlugin::initialize(&mut p, 48000).unwrap();

    // Change a field directly and rebuild
    p.low_gain = 12.0;
    p.rebuild_cached_parameters();

    let params = ParametricInPlacePlugin::parameters(&p);
    let low_gain_param = params
        .iter()
        .find(|param| param.id.0 == "low_gain")
        .unwrap();
    assert_eq!(
        low_gain_param.default_value,
        sotf_host::parameters::ParameterValue::Float(12.0)
    );
}
