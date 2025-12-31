// Integration tests for XTC (Crosstalk Cancellation) plugin

use sotf_plugins::{XtcPlugin, XtcPluginParams, Plugin, ProcessContext};

#[test]
fn test_xtc_instantiation() {
    let params = XtcPluginParams::default();
    let mut plugin = XtcPlugin::from_params(params, 44100).unwrap();
    
    assert_eq!(plugin.input_channels(), 2);
    assert_eq!(plugin.info().name, "Crosstalk Cancellation (XTC)");
}

#[test]
fn test_xtc_processing() {
    let params = XtcPluginParams::default();
    let mut plugin = XtcPlugin::new(params, 44100).unwrap();
    
    // Needs enough frames to fill FFT buffer and produce output
    let num_frames = 4096; 
    let mut input = vec![0.0; num_frames * 2];
    
    // Left channel sine wave 1kHz
    for i in 0..num_frames {
        let t = i as f32 / 44100.0;
        input[i * 2] = (2.0 * std::f32::consts::PI * 1000.0 * t).sin();
    }
    
    let mut output = vec![0.0; num_frames * 2];
    let context = ProcessContext {
        sample_rate: 44100,
        num_frames,
    };
    
    plugin.process(&input, &mut output, &context).unwrap();
    
    // Check for some energy in Right channel (cancellation signal)
    // Skip latency (fft_size)
    let latency = 1024;
    let right_energy: f32 = (latency..num_frames).map(|i| output[i * 2 + 1].powi(2)).sum();
    
    assert!(right_energy > 0.1, "XTC should produce cancellation signal in opposite channel");
}
