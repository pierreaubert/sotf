// ============================================================================
// XTC Plugin Demo
// ============================================================================
//
// Demonstrates the usage of the Crosstalk Cancellation (XTC) plugin.
//
// Run with:
// cargo run --example xtc_demo --release

use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_host::{ParameterId, ParameterValue};
use sotf_plugin_xtc::{XtcPlugin, XtcPluginParams};

fn main() {
    println!("=== XTC Plugin Demo ===\n");

    // Create XTC plugin with default parameters
    let params = XtcPluginParams::default();
    println!("Creating XTC plugin with parameters:");
    println!("  Distance: {}m", params.distance_m);
    println!("  Speaker angle: {}°", params.speaker_angle_deg);
    println!("  Head radius: {}m", params.head_radius_m);
    println!("  FFT size: {}", params.fft_size);
    println!("  Beta (regularization): {}", params.beta_base);
    println!();

    let sample_rate = 48000;
    let mut plugin = XtcPlugin::from_params(params, sample_rate).expect("Failed to create plugin");

    // Initialize plugin
    plugin
        .initialize(sample_rate)
        .expect("Failed to initialize");

    let info = plugin.info();
    println!("Plugin Info:");
    println!("  Name: {}", info.name);
    println!("  Version: {}", info.version);
    println!("  Description: {}", info.description);
    println!("  Input channels: {}", plugin.input_channels());
    println!("  Output channels: {}", plugin.output_channels());
    println!(
        "  Latency: {} samples ({:.2}ms @ {}Hz)",
        plugin.latency_samples(),
        plugin.latency_samples() as f32 * 1000.0 / sample_rate as f32,
        sample_rate
    );
    println!();

    // Test with a stereo signal - use longer duration to get past latency
    let num_frames = 8192;
    let mut input = vec![0.0_f32; num_frames * 2];
    let mut output = vec![0.0_f32; num_frames * 2];

    // Generate test signal: stereo 1kHz with different phases (stereo image)
    for i in 0..num_frames {
        let t = i as f32 / sample_rate as f32;
        let phase = 2.0 * std::f32::consts::PI * 1000.0 * t;
        input[i * 2] = phase.sin() * 0.5; // Left: sine
        input[i * 2 + 1] = phase.cos() * 0.5; // Right: cosine (90° phase shift)
    }

    let context = ProcessContext::new(sample_rate, num_frames);

    // Process audio
    plugin
        .process(&input, &mut output, &context)
        .expect("Failed to process audio");

    // Compute energy (skip initial latency period)
    let skip_samples = 2048;
    let input_energy: f32 = input[skip_samples * 2..].iter().map(|x| x.powi(2)).sum();
    let output_energy: f32 = output[skip_samples * 2..].iter().map(|x| x.powi(2)).sum();

    let energy_ratio = output_energy / input_energy;

    println!("Processing Results (stereo signal):");
    println!("  Input energy:  {:.2}", input_energy);
    println!("  Output energy: {:.2}", output_energy);
    println!(
        "  Energy ratio:  {:.3} ({:.1} dB)",
        energy_ratio,
        10.0 * energy_ratio.log10()
    );
    println!();

    // Demonstrate parameter adjustment
    println!("Testing parameter changes:");

    // Change speaker distance
    plugin
        .set_parameter(ParameterId::from("distance_m"), ParameterValue::Float(3.0))
        .expect("Failed to set distance");
    println!("  Set distance to 3.0m");

    // Change speaker angle
    plugin
        .set_parameter(
            ParameterId::from("speaker_angle_deg"),
            ParameterValue::Float(45.0),
        )
        .expect("Failed to set angle");
    println!("  Set speaker angle to 45°");

    // Disable plugin
    plugin
        .set_parameter(ParameterId::from("enabled"), ParameterValue::Bool(false))
        .expect("Failed to disable plugin");
    println!("  Disabled plugin");

    // Reset and process with bypass (use smaller buffer for quick test)
    plugin.reset();
    let bypass_frames = 1024;
    let bypass_input: Vec<f32> = (0..bypass_frames * 2)
        .map(|i| ((i as f32) * 0.01).sin())
        .collect();
    let mut bypass_output = vec![0.0_f32; bypass_frames * 2];
    let bypass_context = ProcessContext::new(sample_rate, bypass_frames);
    plugin
        .process(&bypass_input, &mut bypass_output, &bypass_context)
        .expect("Failed to process audio");

    // Verify bypass
    let mut bypass_ok = true;
    for i in 0..(bypass_frames * 2) {
        if (bypass_output[i] - bypass_input[i]).abs() > 1e-6 {
            bypass_ok = false;
            break;
        }
    }

    println!(
        "  Bypass test: {}",
        if bypass_ok { "PASSED" } else { "FAILED" }
    );
    println!();

    println!("=== Demo Complete ===");
}
