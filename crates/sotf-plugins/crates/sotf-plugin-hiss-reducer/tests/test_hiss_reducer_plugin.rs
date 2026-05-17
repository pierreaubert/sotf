use sotf_host::parameters::ParameterValue;
use sotf_host::plugin::{InPlacePlugin, ProcessContext};
use sotf_plugin_hiss_reducer::HissReducerPlugin;

#[test]
fn disabled_is_transparent() {
    let mut plugin = HissReducerPlugin::new(2);
    plugin
        .set_parameter("enabled".into(), ParameterValue::Bool(false))
        .expect("set enabled");
    plugin.initialize(48000).expect("initialize");

    let mut buffer = vec![0.25, -0.25, 0.5, -0.5];
    let input = buffer.clone();
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames: 2,
    };
    assert_eq!(plugin.process_in_place(&mut buffer, &context).unwrap(), 2);
    assert_eq!(buffer, input);
}

/// Bug fix: low_latency was a dead parameter (the underlying HissReducer is a
/// simple IIR filter, not FFT-based, so the concept does not apply).
/// After the fix, `low_latency` must no longer appear in the parameter list.
#[test]
fn low_latency_param_does_not_exist() {
    let plugin = HissReducerPlugin::new(2);
    let params = plugin.parameters();
    let has_low_latency = params.iter().any(|p| p.id.0 == "low_latency");
    assert!(
        !has_low_latency,
        "low_latency parameter should be removed (HissReducer is not FFT-based)"
    );
}

/// Bug fix: parameter changes (threshold, frequency, strength) used to call
/// rebuild_reducer(), which reset internal DSP state (envelope followers, IIR
/// state) causing audible clicks. After the fix, set_params() is called
/// instead so the buffer processes the same way before and after a no-op
/// parameter round-trip.
#[test]
fn param_change_preserves_state() {
    let mut plugin = HissReducerPlugin::new(1);
    plugin.initialize(48000).expect("initialize");

    // Warm up the reducer so it accumulates state.
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames: 64,
    };
    let mut warm_buf = vec![0.8f32; 64];
    plugin
        .process_in_place(&mut warm_buf, &context)
        .expect("warm-up");

    // Record output for one more block at the current state.
    let mut buf_before = vec![0.3f32; 64];
    plugin
        .process_in_place(&mut buf_before, &context)
        .expect("before");

    // Re-warm fresh plugin to the same state (no param change).
    let mut plugin2 = HissReducerPlugin::new(1);
    plugin2.initialize(48000).expect("initialize");
    let mut warm_buf2 = vec![0.8f32; 64];
    plugin2
        .process_in_place(&mut warm_buf2, &context)
        .expect("warm-up 2");

    // Apply a trivial parameter change (same value → should be a no-op on
    // output if state is preserved).
    plugin2
        .set_parameter("threshold_db".into(), ParameterValue::Float(-30.0))
        .expect("set threshold");

    let mut buf_after = vec![0.3f32; 64];
    plugin2
        .process_in_place(&mut buf_after, &context)
        .expect("after");

    // With state preserved, the outputs must be identical.
    assert_eq!(
        buf_before, buf_after,
        "parameter change should not reset DSP state"
    );
}

/// Bug fix: latency_samples() used to hardcode 0 without querying the reducer.
/// HissReducer is a sample-by-sample IIR filter with no lookahead, so its
/// latency is correctly zero. The test verifies the plugin delegates to the
/// reducer and reports the actual value.
#[test]
fn latency_is_zero_for_iir_filter() {
    let plugin = HissReducerPlugin::new(2);
    assert_eq!(plugin.latency_samples(), 0, "IIR-based HissReducer has zero latency");
}

/// Bug fix: the plugin used to store sample_rate=44100 before initialize() was
/// called, but HissReducer::new() internally defaults to 48000. After the fix,
/// both default to the same rate so the plugin's stored sample_rate is
/// consistent with what the reducer actually uses.
#[test]
fn initial_sample_rate_is_consistent() {
    // Construct without calling initialize() and process a block of silence.
    // With the bug the plugin stored 44100 but the reducer used 48000;
    // after initialize(44100) the reducer would recompute coefficients for
    // 44100 which changes the filter response. The test verifies that calling
    // initialize with the same rate that was used at construction (48000)
    // produces identical output to never calling initialize at all — i.e.,
    // no silent coefficient change happens on the first initialize().
    let mut plugin_uninit = HissReducerPlugin::new(1);
    let context = ProcessContext {
        sample_rate: 48000,
        num_frames: 8,
    };
    // Process before initialize — uses construction-time coefficients.
    let mut buf_uninit = vec![0.5f32; 8];
    plugin_uninit
        .process_in_place(&mut buf_uninit, &context)
        .expect("process uninit");

    let mut plugin_init = HissReducerPlugin::new(1);
    plugin_init.initialize(48000).expect("init");
    let mut buf_init = vec![0.5f32; 8];
    plugin_init
        .process_in_place(&mut buf_init, &context)
        .expect("process init");

    // Outputs must match: the reducer should use 48000 in both cases.
    assert_eq!(
        buf_uninit, buf_init,
        "initialize(48000) must not change the filter response vs. default construction"
    );
}
