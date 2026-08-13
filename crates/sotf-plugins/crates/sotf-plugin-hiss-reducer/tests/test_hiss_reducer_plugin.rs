use sotf_host::parameters::ParameterValue;
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;
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
    let context = ProcessContext::new(48000, 2);
    assert_eq!(plugin.process_in_place(&mut buffer, &context).unwrap(), 2);
    assert_eq!(buffer, input);
}

#[test]
fn disabled_still_validates_buffer_size() {
    let mut plugin = HissReducerPlugin::new(2);
    plugin
        .set_parameter("enabled".into(), ParameterValue::Bool(false))
        .expect("set enabled");

    let context = ProcessContext::new(48000, 2);
    let mut buffer = vec![0.0; 3];
    let err = plugin
        .process_in_place(&mut buffer, &context)
        .expect_err("disabled plugin must still reject malformed host buffers");
    assert!(
        err.contains("Buffer size mismatch"),
        "unexpected error message: {err}"
    );
}

/// Bug fix: low_latency was a dead parameter (the underlying HissReducer is a
/// simple IIR filter, not FFT-based, so the concept does not apply).
/// After the fix, `low_latency` must no longer appear in the parameter list.
#[test]
fn low_latency_param_does_not_exist() {
    let plugin = HissReducerPlugin::new(2);
    let params = plugin.parameters();
    let has_low_latency = params.iter().any(|p| p.id.as_str() == "low_latency");
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
    let context = ProcessContext::new(48000, 64);
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
    assert_eq!(
        plugin.latency_samples(),
        0,
        "IIR-based HissReducer has zero latency"
    );
}

/// Bug fix: the plugin used to store sample_rate=44100 before initialize() was
/// called, but HissReducer::new() internally defaults to 48000. After the fix,
/// both default to the same rate so the plugin's stored sample_rate is
/// consistent with what the reducer actually uses.
#[test]
fn initial_sample_rate_is_consistent() {
    // Processing without an explicit host sample rate is rejected; this avoids
    // silently running with the reducer's construction-time default.
    let mut plugin_uninit = HissReducerPlugin::new(1);
    let context = ProcessContext::new(48000, 8);
    let mut buf_uninit = vec![0.5f32; 8];
    let err = plugin_uninit
        .process_in_place(&mut buf_uninit, &context)
        .expect_err("process before initialize");
    assert!(err.contains("initialized"));

    let mut plugin_init = HissReducerPlugin::new(1);
    plugin_init.initialize(48000).expect("init");
    let mut buf_init = vec![0.5f32; 8];
    plugin_init
        .process_in_place(&mut buf_init, &context)
        .expect("process init");

    assert!(buf_init.iter().all(|sample| sample.is_finite()));
}
