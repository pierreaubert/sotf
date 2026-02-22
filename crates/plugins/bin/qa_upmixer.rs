use sotf_plugins::plugin_upmixer::{UpmixerPlugin, UpmixerPluginParams};
use sotf_plugins::{Plugin, ProcessContext};
use std::f32::consts::PI;

fn main() {
    let sample_rate = 48000;
    let mut params = UpmixerPluginParams::default();
    params.fft_size = 2048;
    params.speaker_config = "5.1".to_string();
    params.gain_front_direct = 1.0;
    params.center_spread = 0.0;

    let mut plugin = UpmixerPlugin::from_params(params);
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: Upmixer Plugin ===");

    // Test 1: Center Extraction (Coherent Mono Input)
    println!("\n[Test 1] Center Extraction (Coherent Mono Input)");
    let num_frames = 16384; // 340ms
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        let s = (2.0 * PI * 1000.0 * i as f32 / sample_rate as f32).sin() * 0.5;
        input[i * 2] = s;
        input[i * 2 + 1] = s;
    }
    let mut output = vec![0.0_f32; num_frames * 6];
    
    // Process in blocks of 1024
    let block_size = 1024;
    let mut pos = 0;
    while pos < num_frames {
        let end = (pos + block_size).min(num_frames);
        let ctx = ProcessContext { sample_rate, num_frames: end - pos };
        plugin.process(&input[pos*2..end*2], &mut output[pos*6..end*6], &ctx).unwrap();
        pos = end;
    }

    // Measure energies in last 100ms
    let measure_start = num_frames - 4800;
    let mut energies = vec![0.0f32; 6];
    for i in measure_start..num_frames {
        for ch in 0..6 {
            let s = output[i * 6 + ch];
            energies[ch] += s * s;
        }
    }
    
    println!("  Channel Energies (FL, FR, C, LFE, SL, SR):");
    println!("  {:?}", energies);
    
    // For coherent input, Center (idx 2) should be dominant
    assert!(energies[2] > 1.0, "Center should have significant energy");
    assert!(energies[2] > energies[0], "Center should be stronger than FL");
    println!("  Center Extraction: PASS");

    // Test 2: Center Spread
    println!("\n[Test 2] Center Spread (spread=1.0)");
    plugin.set_parameter("center_spread".into(), sotf_plugins::ParameterValue::Float(1.0)).unwrap();
    
    // Process another 1s to see change
    let mut output2 = vec![0.0_f32; num_frames * 6];
    let mut pos = 0;
    while pos < num_frames {
        let end = (pos + block_size).min(num_frames);
        let ctx = ProcessContext { sample_rate, num_frames: end - pos };
        plugin.process(&input[pos*2..end*2], &mut output2[pos*6..end*6], &ctx).unwrap();
        pos = end;
    }
    
    let mut energies_spread = vec![0.0f32; 6];
    for i in measure_start..num_frames {
        for ch in 0..6 {
            let s = output2[i * 6 + ch];
            energies_spread[ch] += s * s;
        }
    }
    println!("  Channel Energies (spread=1.0):");
    println!("  {:?}", energies_spread);
    
    assert!(energies_spread[2] < energies[2] * 0.2, "Center energy should have dropped");
    assert!(energies_spread[0] > energies[0], "Front Left energy should have increased");
    println!("  Center Spread: PASS");

    println!("\n[PASS] Upmixer QA Complete.");
}
