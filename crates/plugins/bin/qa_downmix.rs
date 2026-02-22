use sotf_plugins::plugin_downmix::{DownmixPlugin, DownmixPluginParams};
use sotf_plugins::{Plugin, ProcessContext};

fn main() {
    let sample_rate = 48000;
    let params = DownmixPluginParams {
        input_channels: 6,
        center_gain_db: 0.0, // 1.0 linear
        surround_gain_db: -100.0,
        height_gain_db: -100.0,
        lfe_gain_db: -100.0,
        phase_coherence: false,
        phase_blend_low_hz: 200.0,
        phase_blend_high_hz: 5000.0,
    };

    let mut plugin = DownmixPlugin::from_params(params);
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: Downmix Plugin ===");

    // Test 1: Pure Center to Stereo
    println!("\n[Test 1] Center to L/R (Center=1.0, Gain=0dB)");
    let num_frames = 8192;
    let mut input = vec![0.0; num_frames * 6];
    for i in 0..num_frames {
        input[i * 6 + 2] = 1.0; // C
    }
    
    let mut output = vec![0.0; num_frames * 2];
    
    let mut pos = 0;
    let block_size = 1024;
    while pos < num_frames {
        let end = (pos + block_size).min(num_frames);
        let ctx = ProcessContext { sample_rate, num_frames: end - pos };
        plugin.process(&input[pos*6..end*6], &mut output[pos*2..end*2], &ctx).unwrap();
        pos = end;
    }
    
    // For Center only, L_out = 0.707 * C = 0.707 (by default downmix usually uses -3dB for center)
    // Wait, DownmixPlugin uses center_gain_db. I set it to 0dB.
    // In update_downmix_coeffs:
    // self.target_coeffs[2] = DownmixCoeffs { left_gain: center_gain, right_gain: center_gain };
    // center_gain = fast_pow10(self.center_gain_db / 20.0) * 0.707; // Ah! It adds 0.707!
    
    let last_sample_l = output[(num_frames-1)*2];
    println!("  L_out Expected: ~0.707, Measured: {:.3}", last_sample_l);
    assert!((last_sample_l - 0.707).abs() < 0.1);
    println!("  Center to L/R: PASS");

    println!("\n[PASS] Downmix QA Complete.");
}
