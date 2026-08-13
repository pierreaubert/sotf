use sotf_host::ParametricInPlacePlugin;
use sotf_host::{CountingAlloc, ParameterId, ParameterValue, ProcessContext, assert_no_allocs};
use sotf_plugin_crossfeed::{CrossfeedPlugin, CrossfeedPluginParams};

#[global_allocator]
static ALLOCATOR: CountingAlloc = CountingAlloc;

#[test]
fn realtime_parameter_updates_and_reset_do_not_allocate() {
    let mut plugin = CrossfeedPlugin::new(CrossfeedPluginParams::default()).unwrap();
    plugin.initialize(48_000).unwrap();
    let updates = [
        ("mix", ParameterValue::Float(0.5)),
        ("crossfeed_preset", ParameterValue::Int(1)),
        ("crossfeed_mode", ParameterValue::Int(3)),
        ("enabled", ParameterValue::Bool(true)),
        ("bauer_fcut_hz", ParameterValue::Float(750.0)),
        ("bauer_feed_db", ParameterValue::Float(6.0)),
        ("meier_level", ParameterValue::Float(40.0)),
        ("mb_low_freq_hz", ParameterValue::Float(180.0)),
        ("mb_mid_high_freq_hz", ParameterValue::Float(6_000.0)),
        ("mb_low_feed_db", ParameterValue::Float(-6.0)),
        ("mb_mid_feed_db", ParameterValue::Float(3.0)),
        ("mb_high_feed_db", ParameterValue::Float(1.0)),
        ("itd_delay_ms", ParameterValue::Float(0.4)),
        ("autogain_enabled", ParameterValue::Bool(true)),
        ("autogain_target_lufs", ParameterValue::Float(-20.0)),
        ("autogain_max_gain_db", ParameterValue::Float(10.0)),
        ("autogain_smoothing_ms", ParameterValue::Float(150.0)),
        ("head_yaw_deg", ParameterValue::Float(30.0)),
    ];
    let updates: Vec<_> = updates
        .into_iter()
        .map(|(id, value)| (ParameterId::from(id), value))
        .collect();

    for (id, value) in updates {
        assert_no_allocs("Crossfeed realtime parameter update", || {
            plugin
                .parametric_set_parameter(id.clone(), value.clone())
                .unwrap();
        });
    }
    assert_no_allocs("Crossfeed reset", || plugin.reset());
}

#[test]
fn hrtf_processing_does_not_allocate() {
    let mut params = CrossfeedPluginParams::default();
    params.mode = sotf_plugin_crossfeed::CrossfeedMode::Hrtf;
    let mut plugin = CrossfeedPlugin::new(params).unwrap();
    plugin.initialize(48_000).unwrap();
    let mut buffer = vec![0.0; 256 * 2];
    buffer[0] = 1.0;
    assert_no_allocs("Crossfeed HRTF processing", || {
        plugin
            .process_in_place(&mut buffer, &ProcessContext::new(48_000, 256))
            .unwrap();
    });
}
