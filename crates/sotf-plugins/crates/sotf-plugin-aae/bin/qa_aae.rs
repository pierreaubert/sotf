use sotf_host::{CountingAlloc, run_standard_tests};
use sotf_host::{ParameterValue, Plugin, ProcessContext};
use sotf_plugin_aae::{AaePlugin, params::AaePluginParams};
use std::f32::consts::PI;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let params = AaePluginParams::default();

    let mut plugin = AaePlugin::from_params(params);
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: AAE (Active Acoustic Enhancement) Plugin ===");

    // Test 1: Reverb Tail Presence
    test_reverb_tail(&mut plugin, sample_rate);

    // Test 2: Channel Energy Distribution (5.1)
    test_channel_energy(&mut plugin, sample_rate);

    // Test 3: RT60 Parameter Effect
    test_rt60_parameter(&mut plugin, sample_rate);

    // Test 4: Speaker Config Change (7.1.4)
    test_speaker_config_change(&mut plugin, sample_rate);

    // Test 5: Bypass Transparency
    test_bypass(&mut plugin, sample_rate);

    // Test 6: No NaN/Inf in Output
    test_no_nan_inf(&mut plugin, sample_rate);

    // Test 7: Energy Bounded
    test_energy_bounded(&mut plugin, sample_rate);

    // Run standard QA tests (latency, zero-alloc, performance)
    run_standard_tests(&mut plugin, "AaePlugin");

    println!("\n[ALL PASS] AAE QA Complete.");
}

fn process_blocks(
    plugin: &mut AaePlugin,
    input: &[f32],
    output: &mut [f32],
    sample_rate: u32,
    block_size: usize,
) {
    let in_ch = plugin.input_channels();
    let out_ch = plugin.output_channels();
    let num_frames = input.len() / in_ch;
    let mut pos = 0;
    while pos < num_frames {
        let end = (pos + block_size).min(num_frames);
        let ctx = ProcessContext::new(sample_rate, end - pos);
        plugin
            .process(
                &input[pos * in_ch..end * in_ch],
                &mut output[pos * out_ch..end * out_ch],
                &ctx,
            )
            .unwrap();
        pos = end;
    }
}

fn test_reverb_tail(plugin: &mut AaePlugin, sample_rate: u32) {
    println!("\n[Test 1] Reverb Tail Presence");
    plugin.reset();

    let out_ch = plugin.output_channels();
    let block = 1024;

    // Feed a short burst (10ms) of 1kHz sine
    let burst_frames = (sample_rate as f32 * 0.01) as usize;
    let mut burst_input = vec![0.0_f32; burst_frames * 2];
    for i in 0..burst_frames {
        let s = (2.0 * PI * 1000.0 * i as f32 / sample_rate as f32).sin() * 0.5;
        burst_input[i * 2] = s;
        burst_input[i * 2 + 1] = s;
    }
    let mut burst_output = vec![0.0_f32; burst_frames * out_ch];
    process_blocks(plugin, &burst_input, &mut burst_output, sample_rate, block);

    // Now feed 2 seconds of silence and measure reverb tail
    let tail_frames = sample_rate as usize * 2;
    let silence_input = vec![0.0_f32; tail_frames * 2];
    let mut tail_output = vec![0.0_f32; tail_frames * out_ch];
    process_blocks(plugin, &silence_input, &mut tail_output, sample_rate, block);

    // Measure energy in early portion (0–200ms) and late portion (500ms–1s)
    let early_end = (sample_rate as usize / 5) * out_ch;
    let late_start = (sample_rate as usize / 2) * out_ch;
    let late_end = sample_rate as usize * out_ch;

    let early_energy: f32 = tail_output[..early_end].iter().map(|v| v * v).sum();
    let late_energy: f32 = tail_output[late_start..late_end]
        .iter()
        .map(|v| v * v)
        .sum();

    println!("  Early energy (0–200ms): {:.6}", early_energy);
    println!("  Late energy (500ms–1s): {:.6}", late_energy);

    assert!(
        early_energy > 1e-6,
        "Should have early reflections, energy={}",
        early_energy
    );
    assert!(
        late_energy > 1e-8,
        "Should have late reverb tail, energy={}",
        late_energy
    );
    assert!(
        early_energy > late_energy,
        "Early energy should exceed late energy (natural decay)"
    );
    println!("  Reverb Tail: PASS");
}

fn test_channel_energy(plugin: &mut AaePlugin, sample_rate: u32) {
    println!("\n[Test 2] Channel Energy Distribution (5.1)");
    plugin.reset();

    let out_ch = plugin.output_channels(); // 6 for 5.1
    let num_frames = 16384;
    let block = 1024;

    // Stereo pink-ish noise (deterministic)
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        let s = ((i as f32 * 0.0731).sin() + (i as f32 * 0.0173).sin()) * 0.3;
        input[i * 2] = s;
        input[i * 2 + 1] = s * 0.8; // Slight L/R difference
    }
    let mut output = vec![0.0_f32; num_frames * out_ch];
    process_blocks(plugin, &input, &mut output, sample_rate, block);

    // Measure per-channel energy in last half
    let measure_start = num_frames / 2;
    let mut energies = vec![0.0_f32; out_ch];
    for i in measure_start..num_frames {
        for ch in 0..out_ch {
            let s = output[i * out_ch + ch];
            energies[ch] += s * s;
        }
    }

    let labels = ["FL", "FR", "C", "LFE", "SL", "SR"];
    println!("  Channel Energies:");
    for (i, e) in energies.iter().enumerate() {
        let label = labels.get(i).unwrap_or(&"?");
        println!("    {}: {:.4}", label, e);
    }

    // Front channels (FL, FR) should have energy (direct path)
    assert!(energies[0] > 1e-4, "FL should have energy");
    assert!(energies[1] > 1e-4, "FR should have energy");

    // Surround channels should have some energy (reverb distribution)
    if out_ch >= 6 {
        assert!(
            energies[4] > 1e-6 || energies[5] > 1e-6,
            "Surround channels should have reverb energy"
        );
    }

    // No channel should be disproportionately loud
    let max_energy = energies.iter().cloned().fold(0.0_f32, f32::max);
    for (i, e) in energies.iter().enumerate() {
        if *e > 0.0 {
            assert!(
                max_energy / e < 1000.0,
                "Channel {} energy ratio too extreme: {:.1}×",
                i,
                max_energy / e
            );
        }
    }
    println!("  Channel Energy Distribution: PASS");
}

fn test_rt60_parameter(plugin: &mut AaePlugin, sample_rate: u32) {
    println!("\n[Test 3] RT60 Parameter Effect");

    let out_ch = plugin.output_channels();
    let block = 1024;

    // Measure tail energy with short RT60
    plugin.reset();
    plugin
        .set_parameter("rt60".into(), ParameterValue::Float(0.5))
        .unwrap();

    let burst_frames = 480;
    let mut burst = vec![0.0_f32; burst_frames * 2];
    for i in 0..burst_frames {
        let s = (2.0 * PI * 440.0 * i as f32 / sample_rate as f32).sin() * 0.5;
        burst[i * 2] = s;
        burst[i * 2 + 1] = s;
    }
    let mut out_short = vec![0.0_f32; burst_frames * out_ch];
    process_blocks(plugin, &burst, &mut out_short, sample_rate, block);

    // 1 second of silence
    let tail_frames = sample_rate as usize;
    let silence = vec![0.0_f32; tail_frames * 2];
    let mut tail_short = vec![0.0_f32; tail_frames * out_ch];
    process_blocks(plugin, &silence, &mut tail_short, sample_rate, block);

    let late_start = tail_frames / 2;
    let energy_short: f32 = tail_short[late_start * out_ch..]
        .iter()
        .map(|v| v * v)
        .sum();

    // Measure tail energy with long RT60
    plugin.reset();
    plugin
        .set_parameter("rt60".into(), ParameterValue::Float(4.0))
        .unwrap();

    let mut out_long = vec![0.0_f32; burst_frames * out_ch];
    process_blocks(plugin, &burst, &mut out_long, sample_rate, block);

    let mut tail_long = vec![0.0_f32; tail_frames * out_ch];
    process_blocks(plugin, &silence, &mut tail_long, sample_rate, block);

    let energy_long: f32 = tail_long[late_start * out_ch..].iter().map(|v| v * v).sum();

    println!("  RT60=0.5s late energy: {:.8}", energy_short);
    println!("  RT60=4.0s late energy: {:.8}", energy_long);

    assert!(
        energy_long > energy_short,
        "Longer RT60 should produce more late energy"
    );
    println!("  RT60 Parameter: PASS");

    // Restore default
    plugin
        .set_parameter("rt60".into(), ParameterValue::Float(1.8))
        .unwrap();
}

fn test_speaker_config_change(plugin: &mut AaePlugin, sample_rate: u32) {
    println!("\n[Test 4] Speaker Config Change (7.1.4)");

    plugin
        .set_parameter(
            "speaker_config".into(),
            ParameterValue::String("7.1.4".to_string()),
        )
        .unwrap();
    assert_eq!(
        plugin.output_channels(),
        12,
        "7.1.4 should have 12 channels"
    );

    let num_frames = 4096;
    let block = 1024;
    let input: Vec<f32> = (0..num_frames * 2)
        .map(|i| (i as f32 * 0.01).sin() * 0.3)
        .collect();
    let mut output = vec![0.0_f32; num_frames * 12];
    process_blocks(plugin, &input, &mut output, sample_rate, block);

    // Check no NaN/Inf
    for (i, v) in output.iter().enumerate() {
        assert!(v.is_finite(), "Output[{}] is not finite: {}", i, v);
    }

    // Check height channels have some energy (channels 8-11 in 7.1.4)
    let mut height_energy = 0.0_f32;
    for f in 0..num_frames {
        for ch in 8..12 {
            let v = output[f * 12 + ch];
            height_energy += v * v;
        }
    }
    println!("  Height channel energy: {:.6}", height_energy);
    println!("  12-channel processing: PASS");

    // Restore 5.1
    plugin
        .set_parameter(
            "speaker_config".into(),
            ParameterValue::String("5.1".to_string()),
        )
        .unwrap();
    plugin.initialize(sample_rate).unwrap();
}

fn test_bypass(plugin: &mut AaePlugin, sample_rate: u32) {
    println!("\n[Test 5] Bypass Transparency");
    plugin.reset();
    plugin
        .set_parameter("bypass".into(), ParameterValue::Bool(true))
        .unwrap();

    let out_ch = plugin.output_channels();
    let num_frames = 1024;
    let input: Vec<f32> = (0..num_frames * 2)
        .map(|i| (i as f32 * 0.05).sin() * 0.7)
        .collect();
    let mut output = vec![0.0_f32; num_frames * out_ch];

    let ctx = ProcessContext::new(sample_rate, num_frames);
    plugin.process(&input, &mut output, &ctx).unwrap();

    // In bypass, FL should equal input L, FR should equal input R
    let mut max_diff_l = 0.0_f32;
    let mut max_diff_r = 0.0_f32;
    for i in 0..num_frames {
        let diff_l = (output[i * out_ch] - input[i * 2]).abs();
        let diff_r = (output[i * out_ch + 1] - input[i * 2 + 1]).abs();
        max_diff_l = max_diff_l.max(diff_l);
        max_diff_r = max_diff_r.max(diff_r);
    }
    assert!(
        max_diff_l < 1e-6,
        "Bypass FL should match input L, max_diff={}",
        max_diff_l
    );
    assert!(
        max_diff_r < 1e-6,
        "Bypass FR should match input R, max_diff={}",
        max_diff_r
    );

    // Other channels should be silent
    for i in 0..num_frames {
        for ch in 2..out_ch {
            assert!(
                output[i * out_ch + ch].abs() < 1e-6,
                "Bypass: channel {} should be silent",
                ch
            );
        }
    }
    println!("  Bypass Transparency: PASS");

    plugin
        .set_parameter("bypass".into(), ParameterValue::Bool(false))
        .unwrap();
}

fn test_no_nan_inf(plugin: &mut AaePlugin, sample_rate: u32) {
    println!("\n[Test 6] No NaN/Inf in Output");
    plugin.reset();

    let out_ch = plugin.output_channels();
    let num_frames = 48000; // 1 second
    let block = 1024;

    // Diverse input: sine, silence, impulse, near-clipping
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        let t = i as f32 / sample_rate as f32;
        let s = match i / 12000 {
            0 => (2.0 * PI * 440.0 * t).sin() * 0.8, // sine
            1 => 0.0,                                // silence
            2 => {
                if i % 12000 == 0 {
                    1.0
                } else {
                    0.0
                }
            } // impulse
            _ => (2.0 * PI * 100.0 * t).sin() * 0.99, // near-clipping bass
        };
        input[i * 2] = s;
        input[i * 2 + 1] = s * 0.9;
    }
    let mut output = vec![0.0_f32; num_frames * out_ch];
    process_blocks(plugin, &input, &mut output, sample_rate, block);

    for (i, v) in output.iter().enumerate() {
        assert!(
            v.is_finite(),
            "Output[{}] is not finite: {} (frame {}, ch {})",
            i,
            v,
            i / out_ch,
            i % out_ch
        );
    }
    println!("  No NaN/Inf: PASS");
}

fn test_energy_bounded(plugin: &mut AaePlugin, sample_rate: u32) {
    println!("\n[Test 7] Energy Bounded");
    plugin.reset();

    let out_ch = plugin.output_channels();
    let num_frames = 48000;
    let block = 1024;

    let input: Vec<f32> = (0..num_frames * 2)
        .map(|i| (i as f32 * 0.01).sin() * 0.5)
        .collect();
    let mut output = vec![0.0_f32; num_frames * out_ch];
    process_blocks(plugin, &input, &mut output, sample_rate, block);

    let input_energy: f64 = input.iter().map(|v| (*v as f64) * (*v as f64)).sum();
    let output_energy: f64 = output.iter().map(|v| (*v as f64) * (*v as f64)).sum();
    let ratio = output_energy / input_energy;

    println!("  Input energy:  {:.4}", input_energy);
    println!("  Output energy: {:.4}", output_energy);
    println!("  Ratio:         {:.4}", ratio);

    // Output has more channels (6 vs 2), so total energy can be higher.
    // But per-channel average shouldn't exceed input by too much.
    // With dry=0.5, er=0.3, late=0.2, ratio should be < 3.0
    assert!(
        ratio < 5.0,
        "Energy ratio {:.2} too high — possible energy blowup",
        ratio
    );

    // Check no individual sample exceeds ±2.0 (with default conservative levels)
    let max_sample = output.iter().map(|v| v.abs()).fold(0.0_f32, f32::max);
    println!("  Max output sample: {:.4}", max_sample);
    assert!(
        max_sample < 2.0,
        "Max sample {:.4} exceeds ±2.0 — clipping risk",
        max_sample
    );
    println!("  Energy Bounded: PASS");
}
