// ============================================================================
// Denoiser Plugin Demo
// ============================================================================
//
// Demonstrates the usage of the Wiener filter denoiser plugin with MCRA
// noise estimation.
//
// Run with:
// cargo run --example denoiser_demo --release

use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::plugin::{InPlacePlugin, InPlacePluginAdapter, Plugin, ProcessContext};
use sotf_plugin_denoiser::{DenoiserData, DenoiserPlugin, DenoiserPluginParams};

fn main() {
    // env_logger::init();

    println!("=== Denoiser Plugin Demo ===\n");

    // Create denoiser plugin with default parameters
    let params = DenoiserPluginParams::default();
    println!("Creating Denoiser plugin with parameters:");
    println!("  Reduction: {} dB", params.reduction_db);
    println!("  Floor: {} dB", params.floor_db);
    println!("  Smoothing: {}", params.smoothing);
    println!("  Attack: {} ms", params.attack_ms);
    println!("  Release: {} ms", params.release_ms);
    println!("  Low latency: {}", params.low_latency);
    println!("  Transient suppression: {}", params.transient_enabled);
    println!("  Spectral smoothing: {}", params.spectral_smoothing_enabled);
    println!("  Temporal smoothing: {}", params.temporal_smoothing_enabled);
    println!();

    let sample_rate = 48000;
    let channels = 2;
    let denoiser = DenoiserPlugin::from_params(channels, params);
    let mut plugin = InPlacePluginAdapter::new(denoiser);

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

    // Generate test signal: sine wave + noise
    let num_frames = 16384; // Need more frames for STFT processing
    let mut buffer = vec![0.0_f32; num_frames * channels];

    // Add a clean 440Hz signal
    for i in 0..num_frames {
        let t = i as f32 / sample_rate as f32;
        let phase = 2.0 * std::f32::consts::PI * 440.0 * t;
        let signal = phase.sin() * 0.3;
        buffer[i * channels] = signal; // Left
        buffer[i * channels + 1] = signal; // Right
    }

    // Add some noise (simulate background noise)
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    for i in 0..num_frames * channels {
        // Simple pseudo-random noise using hash
        let mut hasher = DefaultHasher::new();
        i.hash(&mut hasher);
        let noise = ((hasher.finish() % 1000) as f32 / 1000.0 - 0.5) * 0.1;
        buffer[i] += noise;
    }

    // Compute input energy
    let input_energy: f32 = buffer.iter().map(|x: &f32| x.powi(2)).sum();

    let context = ProcessContext {
        sample_rate,
        num_frames,
    };

    // Create output buffer (in-place processing)
    let mut output = buffer.clone();

    // Process audio
    plugin
        .process(&buffer, &mut output, &context)
        .expect("Failed to process audio");

    // Compute output energy (skip initial latency)
    let skip_frames = plugin.latency_samples();
    let output_energy: f32 = output[skip_frames * channels..]
        .iter()
        .map(|x: &f32| x.powi(2))
        .sum();
    let input_after_latency: f32 = buffer[skip_frames * channels..]
        .iter()
        .map(|x: &f32| x.powi(2))
        .sum();

    println!("Processing Results (signal + noise):");
    println!("  Input energy:  {:.4}", input_energy);
    println!(
        "  Output energy: {:.4} (after {} sample latency)",
        output_energy, skip_frames
    );
    if input_after_latency > 0.0 {
        let energy_ratio = output_energy / input_after_latency;
        println!(
            "  Energy ratio:  {:.3} ({:.1} dB)",
            energy_ratio,
            10.0 * energy_ratio.log10()
        );
    }
    println!();

    // Get denoiser data for monitoring
    if let Some(data) = plugin.get_data() {
        if let Some(denoiser_data) = data.downcast_ref::<DenoiserData>() {
            println!("Denoiser Monitoring Data:");
            println!(
                "  Average reduction: {:.1} dB",
                denoiser_data.avg_reduction_db
            );
            println!("  Learning active: {}", denoiser_data.learning_active);
            println!(
                "  Noise floor bands: {} (first 5: {:?})",
                denoiser_data.noise_floor_db.len(),
                &denoiser_data.noise_floor_db[..5.min(denoiser_data.noise_floor_db.len())]
            );
            println!();
        }
    }

    // Demonstrate parameter adjustment
    println!("Testing parameter changes:");

    // Change reduction strength
    plugin
        .set_parameter(
            ParameterId::from("reduction_db"),
            ParameterValue::Float(20.0),
        )
        .expect("Failed to set reduction_db");
    println!("  Set reduction to 20 dB");

    // Change floor
    plugin
        .set_parameter(ParameterId::from("floor_db"), ParameterValue::Float(-40.0))
        .expect("Failed to set floor_db");
    println!("  Set floor to -40 dB");

    // Toggle technique flags
    plugin
        .set_parameter(
            ParameterId::from("transient_enabled"),
            ParameterValue::Bool(false),
        )
        .expect("Failed to set transient_enabled");
    println!("  Disabled transient suppression");

    plugin
        .set_parameter(
            ParameterId::from("spectral_smoothing_enabled"),
            ParameterValue::Bool(false),
        )
        .expect("Failed to set spectral_smoothing_enabled");
    println!("  Disabled spectral smoothing");

    plugin
        .set_parameter(
            ParameterId::from("temporal_smoothing_enabled"),
            ParameterValue::Bool(false),
        )
        .expect("Failed to set temporal_smoothing_enabled");
    println!("  Disabled temporal smoothing");

    // Change attack/release
    plugin
        .set_parameter(ParameterId::from("attack_ms"), ParameterValue::Float(2.0))
        .expect("Failed to set attack_ms");
    plugin
        .set_parameter(
            ParameterId::from("release_ms"),
            ParameterValue::Float(100.0),
        )
        .expect("Failed to set release_ms");
    println!("  Set attack=2ms, release=100ms");

    // Get current parameter values
    let reduction = plugin.get_parameter(&ParameterId::from("reduction_db"));
    let floor = plugin.get_parameter(&ParameterId::from("floor_db"));
    println!();
    println!("Current parameters:");
    println!("  reduction_db: {:?}", reduction);
    println!("  floor_db: {:?}", floor);
    println!();

    // Reset and process again with new settings
    plugin.reset();

    let mut output2 = buffer.clone();
    plugin
        .process(&buffer, &mut output2, &context)
        .expect("Failed to process audio");

    let output2_energy: f32 = output2[skip_frames * channels..]
        .iter()
        .map(|x: &f32| x.powi(2))
        .sum();
    println!(
        "After parameter change - Output energy: {:.4}",
        output2_energy
    );
    println!();

    println!("=== Demo Complete ===");
}
