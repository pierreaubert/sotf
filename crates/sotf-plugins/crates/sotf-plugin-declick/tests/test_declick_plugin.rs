use sotf_host::parameters::ParameterValue;
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;
use sotf_plugin_declick::DeclickPlugin;

#[test]
fn public_smoke_test_processes_arbitrary_callback_size() {
    let mut plugin = DeclickPlugin::new(1, 48_000).unwrap();
    plugin
        .set_parameter("sensitivity".into(), ParameterValue::Float(5.0))
        .unwrap();
    let mut buffer: Vec<f32> = (0..127).map(|i| (i as f32 * 0.1).sin() * 0.2).collect();
    let context = ProcessContext::new(48_000, buffer.len());
    assert_eq!(plugin.process_in_place(&mut buffer, &context).unwrap(), 127);
    assert!(buffer.iter().all(|sample| sample.is_finite()));
}
