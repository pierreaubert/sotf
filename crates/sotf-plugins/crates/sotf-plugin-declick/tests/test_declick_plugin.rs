use sotf_host::parameters::ParameterValue;
use sotf_host::plugin::{InPlacePlugin, ProcessContext};
use sotf_plugin_declick::DeclickPlugin;

#[test]
fn disabled_is_transparent() {
    let mut plugin = DeclickPlugin::new(1);
    plugin
        .set_parameter("enabled".into(), ParameterValue::Bool(false))
        .expect("set enabled");

    let mut buffer = vec![0.0, 0.25, 4.0, 0.25];
    let input = buffer.clone();
    let context = ProcessContext::new(48000, buffer.len());
    assert_eq!(plugin.process_in_place(&mut buffer, &context).unwrap(), 4);
    assert_eq!(buffer, input);
}

#[test]
fn click_is_reduced() {
    let mut plugin = DeclickPlugin::new(1);
    plugin
        .set_parameter("sensitivity".into(), ParameterValue::Float(5.0))
        .expect("set sensitivity");

    let mut buffer = vec![0.0; 10];
    for i in 0..100 {
        buffer.push((i as f32 * 0.1).sin() * 0.5);
    }
    let click_idx = buffer.len();
    buffer.push(2.0);
    buffer.extend([0.0; 10]);

    let context = ProcessContext::new(48000, buffer.len());
    plugin.process_in_place(&mut buffer, &context).unwrap();
    assert!(buffer[click_idx] < 2.0);
}
