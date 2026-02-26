// Integration tests for Compressor plugin

use sotf_host::{InPlacePluginAdapter, ParameterId, ParameterValue, PluginHost};
use sotf_plugin_compressor::CompressorPlugin;

#[test]
fn test_compressor_basic() {
    let mut host = PluginHost::new(2, 48000);

    // Add compressor: -20dB threshold, 4:1 ratio
    let compressor = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 6.0, 0.0);
    host.add_plugin(Box::new(InPlacePluginAdapter::new(compressor)))
        .unwrap();

    // Test with loud signal (should be compressed)
    let input = vec![0.8; 2048 * 2]; // Stereo, loud signal
    let mut output = vec![0.0; 2048 * 2];

    host.process(&input, &mut output).unwrap();

    // Output should be attenuated
    let input_rms: f32 = input.iter().map(|x| x * x).sum::<f32>() / input.len() as f32;
    let output_rms: f32 = output.iter().map(|x| x * x).sum::<f32>() / output.len() as f32;

    assert!(output_rms < input_rms, "Compressor should reduce RMS level");
    println!(
        "Compressor: Input RMS = {:.4}, Output RMS = {:.4}",
        input_rms.sqrt(),
        output_rms.sqrt()
    );
}

#[test]
fn test_compressor_parameters() {
    use sotf_host::InPlacePlugin;

    let mut compressor = CompressorPlugin::new(2, -20.0, 4.0, 5.0, 50.0, 6.0, 0.0);
    compressor.initialize(48000).unwrap();

    // Test parameter queries
    let params = compressor.parameters();
    assert_eq!(params.len(), 10);

    // Modify threshold
    compressor
        .set_parameter(ParameterId::from("threshold"), ParameterValue::Float(-30.0))
        .unwrap();

    let threshold = compressor.get_parameter(&ParameterId::from("threshold"));
    assert_eq!(threshold, Some(ParameterValue::Float(-30.0)));

    // Modify ratio
    compressor
        .set_parameter(ParameterId::from("ratio"), ParameterValue::Float(8.0))
        .unwrap();

    let ratio = compressor.get_parameter(&ParameterId::from("ratio"));
    assert_eq!(ratio, Some(ParameterValue::Float(8.0)));
}
