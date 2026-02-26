use sotf_host::{Plugin, ProcessContext};
use sotf_plugin_upmixer::{UpmixerPlugin, UpmixerPluginParams};
use std::f32::consts::PI;

#[test]
fn test_upmixer_voice_leakage() {
    let fft_size = 2048;
    let sample_rate = 48000;
    let mut params = UpmixerPluginParams::default();
    params.fft_size = fft_size;
    params.speaker_config = "5.1".to_string();

    let mut plugin = UpmixerPlugin::from_params(params);
    plugin.initialize(sample_rate).unwrap();

    // Mono signal (simulating a centered voice)
    let num_blocks = 64;
    let num_frames = num_blocks * fft_size;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        let phase = 2.0 * PI * 1000.0 * i as f32 / sample_rate as f32;
        let sample = phase.sin() * 0.5;
        input[i * 2] = sample;
        input[i * 2 + 1] = sample;
    }

    let out_ch = plugin.output_channels();
    let mut output = vec![0.0_f32; num_frames * out_ch];

    let context = ProcessContext {
        sample_rate,
        num_frames,
    };

    plugin.process(&input, &mut output, &context).unwrap();

    // 5.1 layout: L=0, R=1, C=2, LFE=3, SL=4, SR=5
    // For a mono input, we expect most energy in C (2), some in L/R (0,1),
    // and ideally VERY LITTLE in SL/SR (4,5).

    let skip = (num_blocks - 8) * fft_size; // Skip settling time
    let mut energy_c = 0.0_f32;
    let mut energy_sl = 0.0_f32;

    for i in skip..num_frames {
        energy_c += output[i * out_ch + 2].powi(2);
        energy_sl += output[i * out_ch + 4].powi(2);
    }

    let leakage_ratio = energy_sl / energy_c;
    println!("Voice leakage ratio (SL/C): {:.4}", leakage_ratio);

    // If leakage is more than 5%, it's likely audible and problematic for voices.
    assert!(
        leakage_ratio < 0.05,
        "Excessive voice leakage in surrounds: {:.4}",
        leakage_ratio
    );
}

#[test]
fn test_upmixer_phase_alignment_extraction() {
    let fft_size = 2048;
    let sample_rate = 48000;
    let mut params = UpmixerPluginParams::default();
    params.fft_size = fft_size;
    params.speaker_config = "5.1".to_string();
    params.stereo_width = 1.0; // Full center extraction
    params.enable_hr_direct = false; // Disable HR path for pure PCA test
    params.dialogue_weight = 0.0; // Disable dialogue-based steering

    let mut plugin = UpmixerPlugin::from_params(params);
    plugin.initialize(sample_rate).unwrap();

    // L and R are 90 degrees out of phase: highly correlated but not identical
    let num_blocks = 64;
    let num_frames = num_blocks * fft_size;
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        let phase = 2.0 * PI * 1000.0 * i as f32 / sample_rate as f32;
        input[i * 2] = phase.sin() * 0.5;
        input[i * 2 + 1] = phase.cos() * 0.5; // 90 deg shift
    }

    let out_ch = plugin.output_channels();
    let mut output = vec![0.0_f32; num_frames * out_ch];

    let context = ProcessContext {
        sample_rate,
        num_frames,
    };

    plugin.process(&input, &mut output, &context).unwrap();

    // With perfect phase alignment in extraction, L and R residual (front L/R) should be near zero
    let skip = (num_blocks - 8) * fft_size; // Check the very last part
    let mut energy_residual = 0.0_f32;
    let mut energy_total = 0.0_f32;

    for i in skip..num_frames {
        energy_residual += output[i * out_ch + 0].powi(2) + output[i * out_ch + 1].powi(2);
        for ch in 0..out_ch {
            energy_total += output[i * out_ch + ch].powi(2);
        }
    }

    let residual_ratio = energy_residual / energy_total;
    println!(
        "Phase-aligned residual ratio (L+R)/Total: {:.4}",
        residual_ratio
    );

    // Threshold adjusted for smoothing settling time and FFT window leakage
    assert!(
        residual_ratio < 0.05,
        "Phase alignment extraction failed: residual ratio {:.4}",
        residual_ratio
    );
}
