use sotf_plugins::plugin_mono_to_stereo::{MonoToStereoPlugin, MonoToStereoPluginParams};
use sotf_plugins::{Plugin, ProcessContext};
use std::f32::consts::PI;

fn main() {
    let sample_rate = 48000;
    let params = MonoToStereoPluginParams {
        stereo_width: 1.0,
        comp_eq_depth_db: 0.0,
    };

    let mut plugin = MonoToStereoPlugin::from_params(1, params);
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: MonoToStereo Plugin ===");

    // Test 1: Pseudo-Stereo Width (using 1kHz sine where decorrelation is active)
    println!("\n[Test 1] Pseudo-Stereo Width (1kHz Sine)");
    let num_frames = 8192; 
    let input = generate_sine(sample_rate, 1000.0, -6.0, num_frames);
    let mut output = vec![0.0; num_frames * 2];
    
    let mut pos = 0;
    let block_size = 1024;
    while pos < num_frames {
        let end = (pos + block_size).min(num_frames);
        let ctx = ProcessContext { sample_rate, num_frames: end - pos };
        plugin.process(&input[pos..end], &mut output[pos*2..end*2], &ctx).unwrap();
        pos = end;
    }
    
    // Check later frames
    let last_l = output[(num_frames-1)*2];
    let last_r = output[(num_frames-1)*2 + 1];
    let diff = (last_l - last_r).abs();
    println!("  L/R Difference: {:.4}", diff);
    assert!(diff > 0.01, "Pseudo-stereo should produce difference for 1kHz sine");
    
    // Test 2: Energy Preservation
    let mut energy_in = 0.0f32;
    let mut energy_out = 0.0f32;
    for i in num_frames-1024..num_frames {
        energy_in += input[i].powi(2);
        energy_out += (output[i*2].powi(2) + output[i*2+1].powi(2)) * 0.5;
    }
    let ratio = energy_out / energy_in;
    println!("  Energy Ratio: {:.4} (Target: ~1.0)", ratio);
    assert!(ratio > 0.8 && ratio < 1.2);

    println!("\n[PASS] MonoToStereo QA Complete.");
}

fn generate_sine(sr: u32, freq: f32, db: f32, frames: usize) -> Vec<f32> {
    let amp = 10.0f32.powf(db / 20.0);
    (0..frames).map(|i| (2.0 * PI * freq * i as f32 / sr as f32).sin() * amp).collect()
}
