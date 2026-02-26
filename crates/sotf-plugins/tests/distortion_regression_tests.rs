use sotf_plugins::{
    Plugin, ProcessContext, UpmixerPlugin, UpmixerPluginParams, XtcPlugin, XtcPluginParams,
    test_utils::{SignalGen, measure_rms_db},
};

/// Calculate Signal-to-Error Ratio (SER) in dB.
/// Higher is better (more faithful reproduction).
fn calculate_ser(signal: &[f32], error: &[f32]) -> f32 {
    let signal_rms = measure_rms_db(signal);
    let error_rms = measure_rms_db(error);
    signal_rms - error_rms
}

#[test]
fn test_xtc_bypass_fidelity() {
    let sample_rate = 48000;
    let fft_size = 1024;
    let mut params = XtcPluginParams::default();
    params.fft_size = fft_size;
    params.bypass_xtc_filters = true;
    params.auto_gain_enabled = false;

    let mut plugin = XtcPlugin::new(params, sample_rate).unwrap();
    plugin.initialize(sample_rate).unwrap();

    let num_frames = 16384;
    let mut signal_gen = SignalGen::new_sine(sample_rate as f64, 1000.0, 0.5);
    let mono_input = signal_gen.generate(num_frames);
    
    // Stereo input
    let mut input = vec![0.0; num_frames * 2];
    for (i, &s) in mono_input.iter().enumerate() {
        input[i * 2] = s;
        input[i * 2 + 1] = s;
    }
    
    let mut output = vec![0.0; num_frames * 2];
    let context = ProcessContext {
        sample_rate,
        num_frames,
    };

    plugin.process(&input, &mut output, &context).unwrap();

    // Account for STFT latency
    let latency = plugin.latency_samples();
    let start = latency;
    let end = num_frames - latency;
    
    let signal_segment = &input[start * 2..end * 2];
    let output_segment = &output[start * 2..end * 2];
    
    let mut error = vec![0.0; signal_segment.len()];
    for i in 0..signal_segment.len() {
        error[i] = output_segment[i] - signal_segment[i];
    }
    
    let ser = calculate_ser(signal_segment, &error);
    println!("XTC Bypass SER: {:.2} dB", ser);
    
    // For f32, bit-perfect-ish OLA should be > 80 dB.
    // Double-windowing error dropped this to ~30-40 dB.
    assert!(ser > 70.0, "XTC Bypass fidelity too low: {:.2} dB. Possible double-windowing or OLA error.", ser);
}

/// Test that the upmixer STFT path preserves energy.
///
/// The input headroom scale (1/sqrt(2)) applied in fft.rs must be compensated
/// by combined_scale in process.rs. Without compensation, output is ~3dB low.
/// Uses full processing (NOT bypass_all_processing, which skips the STFT entirely).
#[test]
fn test_upmixer_bypass_fidelity() {
    let sample_rate = 48000;
    let fft_size = 2048;
    let mut params = UpmixerPluginParams::default();
    params.fft_size = fft_size;
    // Full processing — NOT bypass. bypass_all_processing copies input directly
    // and never touches the STFT, so it can't catch combined_scale bugs.
    params.bypass_all_processing = false;

    let mut plugin = UpmixerPlugin::from_params(params);
    plugin.initialize(sample_rate).unwrap();

    let num_frames = 16384;
    // Use uncorrelated stereo to exercise full VBAP panning across channels.
    let mut gen_l = SignalGen::new_sine(sample_rate as f64, 440.0, 0.3);
    let mut gen_r = SignalGen::new_sine(sample_rate as f64, 660.0, 0.3);
    let left = gen_l.generate(num_frames);
    let right = gen_r.generate(num_frames);

    let mut input = vec![0.0; num_frames * 2];
    for i in 0..num_frames {
        input[i * 2] = left[i];
        input[i * 2 + 1] = right[i];
    }

    let out_ch = plugin.output_channels();
    let mut output = vec![0.0; num_frames * out_ch];
    let context = ProcessContext { sample_rate, num_frames };
    plugin.process(&input, &mut output, &context).unwrap();

    // Skip latency + warm-up
    let latency = plugin.latency_samples();
    let start = latency + fft_size;
    let end = num_frames - fft_size;

    // Measure total input energy (both channels)
    let mut input_energy = 0.0_f64;
    for i in start..end {
        input_energy += (input[i * 2] as f64).powi(2);
        input_energy += (input[i * 2 + 1] as f64).powi(2);
    }

    // Measure total output energy (all channels)
    let mut output_energy = 0.0_f64;
    for i in start..end {
        for ch in 0..out_ch {
            output_energy += (output[i * out_ch + ch] as f64).powi(2);
        }
    }

    let energy_ratio_db = 10.0 * (output_energy / input_energy).log10();
    println!(
        "Upmixer Energy Conservation: ratio={:.2}dB (input={:.2}, output={:.2})",
        energy_ratio_db, input_energy, output_energy
    );

    // VBAP is energy-preserving: total output energy ≈ total input energy.
    // Allow ±2dB tolerance for panning distribution and processing artifacts.
    // With Bug 3 (combined_scale = 1/N, missing headroom compensation):
    // output is (1/sqrt(2))x → energy is 0.5x → -3dB → FAILS.
    assert!(
        energy_ratio_db > -3.5,
        "Upmixer output too quiet: {:.2}dB. Possible combined_scale error (missing headroom compensation).",
        energy_ratio_db
    );
    assert!(
        energy_ratio_db < 4.0,
        "Upmixer output too hot: {:.2}dB. Possible combined_scale over-compensation.",
        energy_ratio_db
    );
}

#[test]
fn test_xtc_limiter_reactivity() {
    let sample_rate = 48000;
    let mut params = XtcPluginParams::default();
    params.bypass_xtc_filters = false; // Enabled
    params.auto_gain_enabled = false;

    let mut plugin = XtcPlugin::new(params, sample_rate).unwrap();
    plugin.initialize(sample_rate).unwrap();

    // Use a very high amplitude sine wave that should be limited
    let num_frames = 4096;
    let mut signal_gen = SignalGen::new_sine(sample_rate as f64, 1000.0, 10.0); // 20dB above unit
    let mono_input = signal_gen.generate(num_frames);
    
    let mut input = vec![0.0; num_frames * 2];
    for (i, &s) in mono_input.iter().enumerate() {
        input[i * 2] = s;
        input[i * 2 + 1] = s;
    }
    
    let mut output = vec![0.0; num_frames * 2];
    let context = ProcessContext {
        sample_rate,
        num_frames,
    };

    plugin.process(&input, &mut output, &context).unwrap();

    let mut max_peak = 0.0_f32;
    for &s in &output {
        max_peak = max_peak.max(s.abs());
    }
    
    println!("XTC Limiter Max Peak: {:.4}", max_peak);
    
    // A slow limiter (per-block) will let the first block or parts of it through
    // at a very high level before the gain is reduced.
    // A fast limiter (per-sample) should clamp it much more effectively.
    assert!(max_peak < 1.5, "XTC Limiter too slow: peak leaked to {:.4}. Must be < 1.5", max_peak);
}

#[test]
fn test_downmix_bypass_fidelity() {
    let sample_rate = 48000;
    // Downmix with 2 channels should ideally be close to original if gains are unity
    // and phase coherence is disabled.
    let mut plugin = sotf_plugins::DownmixPlugin::new(2);
    plugin.initialize(sample_rate).unwrap();
    
    // Set all gains to unity (0 dB) and disable phase coherence for "bypass" test
    plugin.set_parameter("center_gain_db".into(), sotf_plugins::ParameterValue::Float(0.0)).unwrap();
    plugin.set_parameter("surround_gain_db".into(), sotf_plugins::ParameterValue::Float(0.0)).unwrap();
    plugin.set_parameter("lfe_gain_db".into(), sotf_plugins::ParameterValue::Float(0.0)).unwrap();
    plugin.set_parameter("phase_coherence".into(), sotf_plugins::ParameterValue::Bool(false)).unwrap();

    let num_frames = 16384;
    let mut signal_gen = SignalGen::new_sine(sample_rate as f64, 1000.0, 0.5);
    let mono_input = signal_gen.generate(num_frames);
    
    let mut input = vec![0.0; num_frames * 2];
    for (i, &s) in mono_input.iter().enumerate() {
        input[i * 2] = s; // L
        input[i * 2 + 1] = 0.0; // R (test L separately to avoid summing logic interference)
    }
    
    let mut output = vec![0.0; num_frames * 2];
    let context = ProcessContext {
        sample_rate,
        num_frames,
    };

    plugin.process(&input, &mut output, &context).unwrap();

    // With phase_coherence = false, Downmix uses a simple per-sample path (zero latency)
    let latency = plugin.latency_samples();
    assert_eq!(latency, 0);
    
    let mut error = vec![0.0; input.len()];
    for i in 0..input.len() {
        error[i] = output[i] - input[i];
    }
    
    let ser = calculate_ser(&input, &error);
    println!("Downmix Bypass SER: {:.2} dB", ser);
    
    assert!(ser > 70.0, "Downmix Bypass fidelity too low: {:.2} dB", ser);
}

#[test]
fn test_xtc_envelope_stability() {
    let sample_rate = 48000;
    let fft_size = 1024;
    let mut params = XtcPluginParams::default();
    params.fft_size = fft_size;
    params.bypass_xtc_filters = true; 

    let mut plugin = XtcPlugin::new(params, sample_rate).unwrap();
    plugin.initialize(sample_rate).unwrap();

    let num_frames = 16384;
    // Use DC signal (Step) to test OLA flatness perfectly
    let mut signal_gen = SignalGen::new_step();
    let mono_input = signal_gen.generate(num_frames);
    
    let mut input = vec![0.0; num_frames * 2];
    for (i, &s) in mono_input.iter().enumerate() {
        input[i * 2] = s;
        input[i * 2 + 1] = s;
    }
    
    let mut output = vec![0.0; num_frames * 2];
    let context = ProcessContext {
        sample_rate,
        num_frames,
    };

    plugin.process(&input, &mut output, &context).unwrap();

    let latency = plugin.latency_samples();
    let start = latency + fft_size; // Extra warm-up
    let end = num_frames - fft_size;
    
    // Measure envelope of output (just the values since it's DC)
    let mut envelope = Vec::new();
    for i in start..end {
        envelope.push(output[i * 2]);
    }
    
    let avg = envelope.iter().sum::<f32>() / envelope.len() as f32;
    let mut max_dev = 0.0_f32;
    for &s in &envelope {
        max_dev = max_dev.max((s - avg).abs());
    }
    
    let rel_dev = max_dev / avg.abs().max(1e-6);
    println!("XTC Envelope Relative Deviation (DC): {:.6}%", rel_dev * 100.0);
    
    assert!(rel_dev < 0.001, "XTC Amplitude modulation detected on DC: {:.4}%. Windowing error?", rel_dev * 100.0);
}

/// Test STFT energy conservation with LFO decorrelation mode.
///
/// Uses decorrelation_mode=1 (LFO) to exercise the per-bin phase gradient
/// (Bug 1) and a different speaker config to stress the combined_scale
/// compensation. This complements test_upmixer_bypass_fidelity which uses
/// the default velvet noise decorrelation.
#[test]
fn test_upmixer_envelope_stability() {
    let sample_rate = 48000;
    let fft_size = 2048;
    let mut params = UpmixerPluginParams::default();
    params.fft_size = fft_size;
    params.bypass_all_processing = false;
    params.decorrelation_mode = 1; // LFO mode — exercises Bug 1 code path
    params.speaker_config = "7.1".to_string();

    let mut plugin = UpmixerPlugin::from_params(params);
    plugin.initialize(sample_rate).unwrap();

    let num_frames = 16384;
    let mut gen_l = SignalGen::new_sine(sample_rate as f64, 440.0, 0.3);
    let mut gen_r = SignalGen::new_sine(sample_rate as f64, 660.0, 0.3);
    let left = gen_l.generate(num_frames);
    let right = gen_r.generate(num_frames);

    let mut input = vec![0.0; num_frames * 2];
    for i in 0..num_frames {
        input[i * 2] = left[i];
        input[i * 2 + 1] = right[i];
    }

    let out_ch = plugin.output_channels();
    let mut output = vec![0.0; num_frames * out_ch];
    let context = ProcessContext { sample_rate, num_frames };
    plugin.process(&input, &mut output, &context).unwrap();

    let latency = plugin.latency_samples();
    let start = latency + fft_size;
    let end = num_frames - fft_size;

    let mut input_energy = 0.0_f64;
    for i in start..end {
        input_energy += (input[i * 2] as f64).powi(2);
        input_energy += (input[i * 2 + 1] as f64).powi(2);
    }

    let mut output_energy = 0.0_f64;
    for i in start..end {
        for ch in 0..out_ch {
            output_energy += (output[i * out_ch + ch] as f64).powi(2);
        }
    }

    let energy_ratio_db = 10.0 * (output_energy / input_energy).log10();
    println!(
        "Upmixer 7.1 LFO Energy Conservation: ratio={:.2}dB",
        energy_ratio_db
    );

    // Same energy conservation check as bypass_fidelity but with different config.
    // With Bug 3 (combined_scale = 1/N): output is ~3dB low → FAILS.
    assert!(
        energy_ratio_db > -3.5,
        "Upmixer 7.1 output too quiet: {:.2}dB. Possible combined_scale error.",
        energy_ratio_db
    );
    assert!(
        energy_ratio_db < 4.0,
        "Upmixer 7.1 output too hot: {:.2}dB.",
        energy_ratio_db
    );
}

#[test]
fn test_downmix_phase_coherence_stability() {
    let sample_rate = 48000;
    let mut plugin = sotf_plugins::DownmixPlugin::new(2);
    plugin.initialize(sample_rate).unwrap();
    
    // Enable phase coherence and set unity gains
    plugin.set_parameter("center_gain_db".into(), sotf_plugins::ParameterValue::Float(0.0)).unwrap();
    plugin.set_parameter("surround_gain_db".into(), sotf_plugins::ParameterValue::Float(0.0)).unwrap();
    plugin.set_parameter("lfe_gain_db".into(), sotf_plugins::ParameterValue::Float(0.0)).unwrap();
    plugin.set_parameter("phase_coherence".into(), sotf_plugins::ParameterValue::Bool(true)).unwrap();

    let num_frames = 16384;
    let mut signal_gen = SignalGen::new_step();
    let mono_input = signal_gen.generate(num_frames);
    
    let mut input = vec![0.0; num_frames * 2];
    for (i, &s) in mono_input.iter().enumerate() {
        input[i * 2] = s;
        input[i * 2 + 1] = 0.0;
    }
    
    let mut output = vec![0.0; num_frames * 2];
    let context = ProcessContext {
        sample_rate,
        num_frames,
    };

    plugin.process(&input, &mut output, &context).unwrap();

    let latency = plugin.latency_samples();
    let start = latency + 2048; // Warm-up for STFT
    let end = num_frames - 2048;
    
    let mut envelope = Vec::new();
    for i in start..end {
        envelope.push(output[i * 2]);
    }
    
    let avg = envelope.iter().sum::<f32>() / envelope.len() as f32;
    let mut max_dev = 0.0_f32;
    for &s in &envelope {
        max_dev = max_dev.max((s - avg).abs());
    }
    
    let rel_dev = max_dev / avg.abs().max(1e-6);
    println!("Downmix (PC) Envelope Relative Deviation (DC): {:.6}%", rel_dev * 100.0);
    
    // Phase coherence path uses dual windowing (Hann*Hann) with 75% overlap.
    // COLA should still be perfect (deviation < 0.001%).
    assert!(rel_dev < 0.001, "Downmix Phase Coherence Amplitude modulation detected: {:.4}%. Windowing error?", rel_dev * 100.0);
}

/// Test energy conservation with prime-sized blocks to stress OLA alignment.
///
/// Same as test_upmixer_bypass_fidelity but processes in small, non-power-of-2
/// block sizes to verify that the STFT OLA accumulator and combined_scale work
/// correctly across block boundaries.
#[test]
fn test_upmixer_prime_block_size_fidelity() {
    let sample_rate = 48000;
    let fft_size = 2048;
    let mut params = UpmixerPluginParams::default();
    params.fft_size = fft_size;
    params.bypass_all_processing = false;

    let mut plugin = UpmixerPlugin::from_params(params);
    plugin.initialize(sample_rate).unwrap();

    let total_frames = 32768;
    let prime_block = 127; // Stress OLA alignment

    let mut gen_l = SignalGen::new_sine(sample_rate as f64, 440.0, 0.3);
    let mut gen_r = SignalGen::new_sine(sample_rate as f64, 660.0, 0.3);
    let left = gen_l.generate(total_frames);
    let right = gen_r.generate(total_frames);

    let mut input = vec![0.0; total_frames * 2];
    for i in 0..total_frames {
        input[i * 2] = left[i];
        input[i * 2 + 1] = right[i];
    }

    let out_ch = plugin.output_channels();
    let mut output = vec![0.0; total_frames * out_ch];

    let mut frames_processed = 0;
    while frames_processed < total_frames {
        let nf = prime_block.min(total_frames - frames_processed);
        let ctx = ProcessContext { sample_rate, num_frames: nf };

        let start_in = frames_processed * 2;
        let end_in = start_in + nf * 2;
        let start_out = frames_processed * out_ch;
        let end_out = start_out + nf * out_ch;

        plugin.process(&input[start_in..end_in], &mut output[start_out..end_out], &ctx).unwrap();
        frames_processed += nf;
    }

    let latency = plugin.latency_samples();
    let start = latency + fft_size;
    let end = total_frames - fft_size;

    let mut input_energy = 0.0_f64;
    for i in start..end {
        input_energy += (input[i * 2] as f64).powi(2);
        input_energy += (input[i * 2 + 1] as f64).powi(2);
    }

    let mut output_energy = 0.0_f64;
    for i in start..end {
        for ch in 0..out_ch {
            output_energy += (output[i * out_ch + ch] as f64).powi(2);
        }
    }

    let energy_ratio_db = 10.0 * (output_energy / input_energy).log10();
    println!(
        "Upmixer Prime Block Energy Conservation: ratio={:.2}dB",
        energy_ratio_db
    );

    assert!(
        energy_ratio_db > -3.5,
        "Upmixer output too quiet with prime blocks: {:.2}dB. Possible combined_scale error.",
        energy_ratio_db
    );
    assert!(
        energy_ratio_db < 4.0,
        "Upmixer output too hot with prime blocks: {:.2}dB.",
        energy_ratio_db
    );
}






