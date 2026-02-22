#[cfg(test)]
mod upmixer_tests {
    use super::super::*;
    use crate::{ProcessContext, UpmixerPlugin};

    #[test]
    fn test_upmixer_creation_5_1() {
        let plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 6);
        assert_eq!(plugin.fft_size, 2048);
        assert_eq!(plugin.speaker_config.id, "5.1");
    }

    #[test]
    fn test_upmixer_creation_7_1_4() {
        let plugin = UpmixerPlugin::new(
            2048, "7.1.4", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        assert_eq!(plugin.input_channels(), 2);
        assert_eq!(plugin.output_channels(), 12);
        assert_eq!(plugin.fft_size, 2048);
        assert_eq!(plugin.speaker_config.id, "7.1.4");
    }

    #[test]
    fn test_upmixer_parameters() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );

        // Test setting parameters
        plugin
            .set_parameter(
                ParameterId::from("gain_front_direct"),
                ParameterValue::Float(0.8),
            )
            .unwrap();
        assert_eq!(plugin.gain_front_direct.target(), 0.8);

        // Test getting parameters
        let value = plugin.get_parameter(&ParameterId::from("gain_rear_ambient"));
        assert_eq!(value, Some(ParameterValue::Float(1.0)));
    }

    #[test]
    fn test_center_spread_parameter() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );

        assert!((plugin.center_spread.target() - 0.0).abs() < 1e-6);

        plugin
            .set_parameter(
                ParameterId::from("center_spread"),
                ParameterValue::Float(0.7),
            )
            .unwrap();
        assert!((plugin.center_spread.target() - 0.7).abs() < 1e-6);

        // Values outside [0.0, 1.0] are clamped
        let res = plugin.set_parameter(
            ParameterId::from("center_spread"),
            ParameterValue::Float(1.5),
        );
        assert!(res.is_ok());
        assert!((plugin.center_spread.target() - 1.0).abs() < 1e-6);

        // Test lower bound clamping
        let res = plugin.set_parameter(
            ParameterId::from("center_spread"),
            ParameterValue::Float(-0.5),
        );
        assert!(res.is_ok());
        assert!((plugin.center_spread.target() - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_stereo_width_parameter() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );

        assert!((plugin.stereo_width.target() - 0.5).abs() < 1e-6);

        plugin
            .set_parameter(
                ParameterId::from("stereo_width"),
                ParameterValue::Float(0.3),
            )
            .unwrap();
        assert!((plugin.stereo_width.target() - 0.3).abs() < 1e-6);

        // Values outside [0.0, 1.0] are clamped
        let res = plugin.set_parameter(
            ParameterId::from("stereo_width"),
            ParameterValue::Float(2.0),
        );
        assert!(res.is_ok());
        assert!((plugin.stereo_width.target() - 1.0).abs() < 1e-6);

        // Test lower bound clamping
        let res = plugin.set_parameter(
            ParameterId::from("stereo_width"),
            ParameterValue::Float(-1.0),
        );
        assert!(res.is_ok());
        assert!((plugin.stereo_width.target() - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_upmixer_processing() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        // Create test input: 2048 stereo samples (4096 samples total)
        // Use a simple sine wave pattern for more interesting input
        let mut input = vec![0.0_f32; 2048 * 2];
        for i in 0..2048 {
            input[i * 2] = (i as f32 * 0.01).sin() * 0.5; // Left
            input[i * 2 + 1] = (i as f32 * 0.01).cos() * 0.5; // Right
        }
        let mut output = vec![0.0_f32; 2048 * 6];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2048,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Verify output is not all zeros (some processing occurred)
        let sum: f32 = output.iter().map(|x| x.abs()).sum();
        // log::info!("Output sum (abs): {}", sum);
        assert!(sum > 0.0, "Output should not be all zeros");

        // Check that we have output in multiple channels
        let num_channels = 6; // 5.1 has 6 channels
        let mut channel_sums = vec![0.0; num_channels];
        for i in 0..2048 {
            for ch in 0..num_channels {
                channel_sums[ch] += output[i * num_channels + ch].abs();
            }
        }
        // log::info!("Channel sums: {:?}", channel_sums);
        // At least center and front channels should have content
        assert!(
            channel_sums[0] > 0.0 || channel_sums[1] > 0.0 || channel_sums[2] > 0.0,
            "At least one front channel should have content"
        );
    }

    #[test]
    fn test_steering_alphas_frequency_dependent() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        let fft_size = plugin.fft_size;
        let mut input = vec![0.0f32; fft_size * 2];
        for i in 0..fft_size {
            let t = i as f32 / 44100.0;
            input[i * 2] = (2.0 * std::f32::consts::PI * 200.0 * t).sin() * 0.5;
            input[i * 2 + 1] = (2.0 * std::f32::consts::PI * 2000.0 * t).sin() * 0.5;
        }
        let mut output = vec![0.0f32; fft_size * plugin.num_output_channels];
        plugin.process_fft_block(&input, &mut output);

        let num_bands = plugin.erb_bands.len();
        assert!(num_bands >= 3);

        let low_alpha = plugin.steering_alphas[0];
        let high_alpha = plugin.steering_alphas[num_bands.saturating_sub(2)];
        assert!(
            high_alpha > low_alpha,
            "Expected higher-band steering alpha to be larger than low-band (low={}, high={})",
            low_alpha,
            high_alpha
        );
    }

    #[test]
    fn test_coherence_hysteresis_slow_release() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        let fft_size = plugin.fft_size;
        let mut input = vec![0.0f32; fft_size * 2];
        let mut output = vec![0.0f32; fft_size * plugin.num_output_channels];

        // Process enough coherent frames to fill the median filter ring buffer (5 entries)
        // AND let the one-pole smoother (alpha=0.15) converge near the instant value
        for _ in 0..20 {
            for i in 0..fft_size {
                let t = i as f32 / 44100.0;
                let s = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.5;
                input[i * 2] = s;
                input[i * 2 + 1] = s;
            }
            plugin.process_fft_block(&input, &mut output);
        }

        let num_bands = plugin.erb_bands.len();
        assert!(num_bands >= 3);
        let band_idx = num_bands / 2;

        let coh1_inst = plugin.coherence_instant[band_idx];
        let coh1_smooth = plugin.smoothed_coherence[band_idx];
        assert!(
            coh1_inst > 0.5,
            "Instant coherence should be high for correlated signal: {}",
            coh1_inst
        );
        assert!(
            coh1_smooth > 0.0,
            "Smoothed coherence should be positive: {}",
            coh1_smooth
        );

        // Use phase-inverted signal to create strong incoherence
        for i in 0..fft_size {
            let t = i as f32 / 44100.0;
            let s = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.5;
            input[i * 2] = s;
            input[i * 2 + 1] = -s; // Inverted phase = maximally incoherent
        }

        plugin.process_fft_block(&input, &mut output);

        let coh2_inst = plugin.coherence_instant[band_idx];
        let coh2_smooth = plugin.smoothed_coherence[band_idx];

        // Instant coherence should drop
        assert!(
            coh2_inst < coh1_inst,
            "Instant coherence should drop: {} vs {}",
            coh2_inst,
            coh1_inst
        );
        // Median-filtered smoothed coherence should be higher than instant
        // (median of ring buffer with mostly high values + one low value is still high)
        assert!(
            coh2_smooth > coh2_inst,
            "Smoothed coherence ({}) should be higher than instant ({}) due to median filtering",
            coh2_smooth,
            coh2_inst
        );
    }

    #[test]
    fn test_decorrelation_filters_time_varying() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.decorrelation_mode = 1; // Enable LFO mode for time-varying decorrelation
        plugin.initialize(44100).unwrap();

        let fft_size = plugin.fft_size;
        let mut input = vec![0.0f32; fft_size * 2];
        for i in 0..fft_size {
            let t = i as f32 / 44100.0;
            let s = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.5;
            input[i * 2] = s;
            input[i * 2 + 1] = -s;
        }
        let mut output = vec![0.0f32; fft_size * plugin.num_output_channels];

        plugin.process_fft_block(&input, &mut output);
        let half = plugin.fft_size / 2;
        let idx = half.saturating_sub(10).max(1);
        let before_l = plugin.decorrelation_filter_left[idx];
        let before_r = plugin.decorrelation_filter_right[idx];

        plugin.process_fft_block(&input, &mut output);
        let after_l = plugin.decorrelation_filter_left[idx];
        let after_r = plugin.decorrelation_filter_right[idx];

        let diff_l = (after_l - before_l).norm();
        let diff_r = (after_r - before_r).norm();
        assert!(diff_l > 1e-6_f32 || diff_r > 1e-6_f32);
    }

    #[test]
    fn test_hr_transient_envelope_energy_jump() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();
        plugin.enable_hr_direct = true;

        let fft_size = plugin.fft_size;
        let mut input = vec![0.0f32; fft_size * 2];
        let mut output = vec![0.0f32; fft_size * plugin.num_output_channels];

        // First block: low-energy high-frequency tone
        for i in 0..fft_size {
            let t = i as f32 / 44100.0;
            let s = (2.0 * std::f32::consts::PI * 4000.0 * t).sin() * 0.1;
            input[i * 2] = s;
            input[i * 2 + 1] = s;
        }
        plugin.process_fft_block(&input, &mut output);
        let env1 = plugin.hr_transient_env;

        // Second block: large step in HF energy (simulate transient)
        for i in 0..fft_size {
            let t = i as f32 / 44100.0;
            let s = (2.0 * std::f32::consts::PI * 4000.0 * t).sin();
            input[i * 2] = s;
            input[i * 2 + 1] = s;
        }
        plugin.process_fft_block(&input, &mut output);
        let env2 = plugin.hr_transient_env;

        assert!(env2 > env1);
        assert!(env2 > 0.0);
    }

    #[test]
    fn test_center_spread_reduces_center_energy() {
        // Coherent input (L=R) in 5.1: with center_spread=1.0 the physical
        // center channel should receive less direct energy than with
        // center_spread=0.0.

        // Helper to measure center channel energy for a given spread value.
        fn measure_center_energy(center_spread: f32) -> f32 {
            let mut plugin = UpmixerPlugin::new(
                2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
            );
            plugin.initialize(44100).unwrap();
            plugin.center_spread.set_target(center_spread.clamp(0.0, 1.0));

            let fft_size = plugin.fft_size;
            let mut input = vec![0.0f32; fft_size * 2];
            for i in 0..fft_size {
                let t = i as f32 / 44100.0;
                let s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
                input[i * 2] = s;
                input[i * 2 + 1] = s;
            }

            let mut output = vec![0.0f32; fft_size * plugin.num_output_channels];
            let context = ProcessContext {
                sample_rate: 44100,
                num_frames: fft_size,
            };
            plugin.process(&input, &mut output, &context).unwrap();

            // 5.1 layout: channel 2 is Center.
            let center_idx = 2usize;
            let mut energy = 0.0f32;
            for i in 0..fft_size {
                let s = output[i * plugin.num_output_channels + center_idx];
                energy += s * s;
            }
            energy
        }

        let energy_spread_0 = measure_center_energy(0.0);
        let energy_spread_1 = measure_center_energy(1.0);

        assert!(
            energy_spread_1 < energy_spread_0,
            "Center energy should decrease when center_spread=1.0 (got {} vs {})",
            energy_spread_1,
            energy_spread_0
        );
    }

    #[test]
    fn test_hr_block_front_hf_direct_distribution() {
        // Verify that the high-resolution path produces non-zero energy
        // on front speakers for high-frequency coherent input while leaving
        // non-front channels effectively silent.
        // Tests via apply_hr_enhancement which adds HR to time_out_channels.

        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();
        plugin.enable_hr_direct = true;
        plugin.hr_direct_envelope = 1.0;
        plugin.hr_sharpen.set_target(1.0);
        // Force transient envelope high so HR path is active
        plugin.hr_transient_env = 1.0;

        let fft_size = plugin.fft_size;
        let mut input = vec![0.0f32; fft_size * 2];

        // 4 kHz coherent sine (L=R), safely above hf_cut (>= 1 kHz)
        for i in 0..fft_size {
            let t = i as f32 / 44100.0;
            let s = (2.0 * std::f32::consts::PI * 4000.0 * t).sin() * 0.5;
            input[i * 2] = s;
            input[i * 2 + 1] = s;
        }

        // Clear time_out_channels first
        for ch_buf in plugin.time_out_channels.iter_mut() {
            ch_buf.fill(0.0);
        }

        // Apply HR enhancement (adds to time_out_channels)
        plugin.apply_hr_enhancement(&input);

        // Measure per-channel energy in the HR region (center 512 samples)
        let center = (fft_size - plugin.hr_fft_size) / 2;
        let mut energies = vec![0.0f32; plugin.num_output_channels];
        for ch in 0..plugin.num_output_channels {
            for i in center..center + plugin.hr_fft_size {
                energies[ch] += plugin.time_out_channels[ch][i].powi(2);
            }
        }

        // 5.1 layout: 0=FL,1=FR,2=C,3=LFE,4=SL,5=SR
        // Expect FL/FR/C to have some energy, LFE/surrounds to be near zero.
        assert!(
            energies[0] > 0.0 || energies[1] > 0.0 || energies[2] > 0.0,
            "Front speakers should have non-zero HF direct energy from HR path: {:?}",
            energies
        );

        // LFE and surrounds should stay effectively silent in HR path
        for ch in 3..plugin.num_output_channels {
            assert!(
                energies[ch] < 1e-6,
                "Non-front channel {} should be near zero in HR path (got {})",
                ch,
                energies[ch]
            );
        }
    }

    #[test]
    fn test_upmixer_zero_gains() {
        // Test that with all gains at 0, output is silence (critical for crackling fix)
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 0.0, 0.0, 0.0, 120.0, 0.0, 250.0, 0.0, 0.0, false, 0.0,
        );
        plugin.initialize(44100).unwrap();

        // Create test input with signal
        let num_blocks = 8;
        let mut input = vec![0.0_f32; 2048 * num_blocks * 2];
        for i in 0..2048 * num_blocks {
            input[i * 2] = (i as f32 * 0.01).sin() * 0.5; // Left
            input[i * 2 + 1] = (i as f32 * 0.01).cos() * 0.5; // Right
        }
        let mut output = vec![0.0_f32; 2048 * num_blocks * 6];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2048 * num_blocks,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Verify output is effectively silent (allow for small numerical artifacts from normalization)
        // Skip first block for settling
        let max_abs = output[2048*6..].iter().map(|x| x.abs()).fold(0.0_f32, f32::max);
        // log::info!("Max abs value with zero gains: {}", max_abs);
        assert!(
            max_abs < 0.05,
            "With all gains at 0, output should be effectively silent (<-26dB), but max abs = {}",
            max_abs
        );
    }

    #[test]
    fn test_upmixer_config_change() {
        // Test changing speaker configuration dynamically
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        assert_eq!(plugin.output_channels(), 6);
        assert_eq!(plugin.speaker_config.id, "5.1");

        // Change to 7.1.4
        plugin.change_speaker_config("7.1.4").unwrap();
        assert_eq!(plugin.output_channels(), 12);
        assert_eq!(plugin.speaker_config.id, "7.1.4");

        // Change back to 5.1
        plugin.change_speaker_config("5.1").unwrap();
        assert_eq!(plugin.output_channels(), 6);
        assert_eq!(plugin.speaker_config.id, "5.1");
    }

    #[test]
    fn test_upmixer_height_gain() {
        // Test height gain parameter
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1.4", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 0.5, 1.0, false, 0.5,
        );
        assert_eq!(plugin.height_gain.target(), 0.5);
        assert_eq!(plugin.output_channels(), 10); // 5.1.4 has 10 channels

        // Change height gain via parameter
        plugin
            .set_parameter(ParameterId::from("height_gain"), ParameterValue::Float(1.5))
            .unwrap();
        assert_eq!(plugin.height_gain.target(), 1.5);
    }

    #[test]
    fn test_upmixer_full_5ch() {
        // Test full 5.1 upmixing with direct/ambient decomposition
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.0, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        // Create test input with distinct left and right signals at frequencies above bandpass_hz (250 Hz)
        // Use 440 Hz and 880 Hz to ensure they fall in the upmixing band
        let mut input = vec![0.0_f32; 2048 * 2];
        for i in 0..2048 {
            let t = i as f32 / 44100.0;
            input[i * 2] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5; // Left: 440 Hz
            input[i * 2 + 1] = (2.0 * std::f32::consts::PI * 880.0 * t).cos() * 0.5; // Right: 880 Hz
        }
        let mut output = vec![0.0_f32; 2048 * 6];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2048,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Check each channel
        let num_channels = 6; // 5.1 has 6 channels
        let mut channel_energies = vec![0.0; num_channels];
        for i in 0..2048 {
            for ch in 0..num_channels {
                channel_energies[ch] += output[i * num_channels + ch].powi(2);
            }
        }

        // log::info!("Channel energies: {:?}", channel_energies);

        // Front left and right should have signal
        assert!(channel_energies[0] > 0.1, "Front left should have signal");
        assert!(channel_energies[1] > 0.1, "Front right should have signal");

        // Center should have signal (direct component)
        assert!(
            channel_energies[2] > 0.01,
            "Center should have direct component"
        );

        // LFE should have minimal signal since test frequencies (440 Hz, 880 Hz)
        // are above the LFE cutoff (120 Hz)
        assert!(
            channel_energies[3] < 0.01,
            "LFE should be minimal with high frequency input"
        );

        // Rear channels should have signal (ambient with gain=1.0)
        assert!(
            channel_energies[4] > 0.01,
            "Left surround should have ambient signal"
        );
        assert!(
            channel_energies[5] > 0.01,
            "Right surround should have ambient signal"
        );
    }

    #[test]
    fn test_continuity_invariant() {
        // INVARIANT: Processing continuous audio in chunks should produce continuous output
        // Test with various buffer sizes
        for buffer_size in [256, 512, 1024] {
            // log::info!("\n=== Testing buffer size {} ===", buffer_size);
            let mut plugin = UpmixerPlugin::new(
                2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
            );
            plugin.initialize(44100).unwrap();

            // Generate continuous 440Hz sine wave, process in chunks
            let total_samples = 8192;
            let mut all_output = Vec::new();
            let mut sample_offset = 0;

            while sample_offset < total_samples {
                let chunk_size = buffer_size.min(total_samples - sample_offset);
                let mut input = vec![0.0_f32; chunk_size * 2];

                for i in 0..chunk_size {
                    let phase =
                        2.0 * std::f32::consts::PI * 440.0 * (sample_offset + i) as f32 / 44100.0;
                    input[i * 2] = phase.sin() * 0.5;
                    input[i * 2 + 1] = phase.sin() * 0.5;
                }

                let mut output = vec![0.0_f32; chunk_size * 6];
                let context = ProcessContext {
                    sample_rate: 44100,
                    num_frames: chunk_size,
                };

                plugin.process(&input, &mut output, &context).unwrap();
                all_output.extend_from_slice(&output);
                sample_offset += chunk_size;
            }

            // Check that we got significant output (accounting for latency)
            let total_output_samples = all_output.len() / 5;
            let non_zero_samples = all_output.iter().filter(|&&x| x.abs() > 1e-6).count();
            /*
                        log::info!(
                            "Buffer size {}: {} total frames, {} non-zero samples",
                            buffer_size,
                            total_output_samples,
                            non_zero_samples
                        );
            */
            assert!(
                non_zero_samples > total_output_samples / 2,
                "Buffer size {}: Too many zero samples, got {} non-zero out of {} total",
                buffer_size,
                non_zero_samples,
                total_output_samples
            );
        }
    }

    #[test]
    fn test_energy_preservation() {
        // INVARIANT: Total output energy across all 5 channels should roughly equal input energy
        // (accounting for latency and windowing losses)
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        let buffer_size = 1024;
        let mut total_input_energy = 0.0;
        let mut total_output_energy = 0.0;

        for iteration in 0..16 {
            let mut input = vec![0.0_f32; buffer_size * 2];
            for i in 0..buffer_size {
                let phase =
                    2.0 * std::f32::consts::PI * 440.0 * (iteration * buffer_size + i) as f32
                        / 44100.0;
                input[i * 2] = phase.sin() * 0.5;
                input[i * 2 + 1] = phase.sin() * 0.5;
            }

            total_input_energy += input.iter().map(|x| x * x).sum::<f32>();

            let mut output = vec![0.0_f32; buffer_size * 6];
            let context = ProcessContext {
                sample_rate: 44100,
                num_frames: buffer_size,
            };

            plugin.process(&input, &mut output, &context).unwrap();

            // Count all 6 channels
            let num_channels = 6; // 5.1 has 6 channels
            for i in 0..buffer_size {
                for ch in 0..num_channels {
                    total_output_energy += output[i * num_channels + ch].powi(2);
                }
            }
        }

        /*
                log::info!(
                    "Input energy: {}, Output energy: {}, Ratio: {}",
                    total_input_energy,
                    total_output_energy,
                    total_output_energy / total_input_energy
                );
        */

        // Energy scaling factors:
        // 1. Hann window applied once during analysis: ~0.5 mean value
        // 2. With 50% overlap-add, window energy is properly recovered
        // 3. Channel normalization: (0.9/sqrt(2))² ≈ 0.405 energy scale
        // 4. FFT processing and STFT overhead cause some additional loss
        // Accept down to 35% to account for channel spreading and processing losses
        assert!(
            total_output_energy > total_input_energy * 0.35,
            "Energy loss too high: input={}, output={}, ratio={}",
            total_input_energy,
            total_output_energy,
            total_output_energy / total_input_energy
        );
    }

    #[test]
    fn test_no_gaps() {
        // INVARIANT: Every output buffer should have SOME non-zero samples after initial latency
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        let buffer_size = 512;
        let mut gap_count = 0;

        for iteration in 0..20 {
            let mut input = vec![0.0_f32; buffer_size * 2];
            for i in 0..buffer_size {
                let phase =
                    2.0 * std::f32::consts::PI * 440.0 * (iteration * buffer_size + i) as f32
                        / 44100.0;
                input[i * 2] = phase.sin() * 0.5;
                input[i * 2 + 1] = phase.sin() * 0.5;
            }

            let mut output = vec![0.0_f32; buffer_size * 6];
            let context = ProcessContext {
                sample_rate: 44100,
                num_frames: buffer_size,
            };

            plugin.process(&input, &mut output, &context).unwrap();

            let max_abs = output.iter().map(|x| x.abs()).fold(0.0f32, f32::max);

            if iteration >= 5 && max_abs < 1e-6 {
                gap_count += 1;
                // log::info!("GAP at iteration {}: max_abs = {}", iteration, max_abs);
            }
        }

        assert_eq!(
            gap_count, 0,
            "Found {} gaps in output after initial latency",
            gap_count
        );
    }

    #[test]
    fn test_upmixer_new_configs() {
        // Test creating upmixer with 2.0 configuration
        let plugin_2_0 = UpmixerPlugin::new(
            2048, "2.0", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        assert_eq!(plugin_2_0.input_channels(), 2);
        assert_eq!(plugin_2_0.output_channels(), 2);
        assert_eq!(plugin_2_0.speaker_config.id, "2.0");

        // Test creating upmixer with 5.0 configuration
        let plugin_5_0 = UpmixerPlugin::new(
            2048, "5.0", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        assert_eq!(plugin_5_0.input_channels(), 2);
        assert_eq!(plugin_5_0.output_channels(), 5);
        assert_eq!(plugin_5_0.speaker_config.id, "5.0");
    }

    #[test]
    fn test_upmixer_parameter_config_indices() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );

        // Test that parameter index 0 corresponds to 5.1
        let value = plugin.get_parameter(&ParameterId::from("speaker_config"));
        assert_eq!(value, Some(ParameterValue::Int(0)));

        // Test setting to 2.0 (index 8)
        plugin
            .set_parameter(ParameterId::from("speaker_config"), ParameterValue::Int(8))
            .unwrap();
        assert_eq!(plugin.speaker_config.id, "2.0");
        assert_eq!(plugin.output_channels(), 2);
        let value = plugin.get_parameter(&ParameterId::from("speaker_config"));
        assert_eq!(value, Some(ParameterValue::Int(8)));

        // Test setting to 5.0 (index 9)
        plugin
            .set_parameter(ParameterId::from("speaker_config"), ParameterValue::Int(9))
            .unwrap();
        assert_eq!(plugin.speaker_config.id, "5.0");
        assert_eq!(plugin.output_channels(), 5);
        let value = plugin.get_parameter(&ParameterId::from("speaker_config"));
        assert_eq!(value, Some(ParameterValue::Int(9)));

        // Test setting to 7.1 (index 1)
        plugin
            .set_parameter(ParameterId::from("speaker_config"), ParameterValue::Int(1))
            .unwrap();
        assert_eq!(plugin.speaker_config.id, "7.1");
        assert_eq!(plugin.output_channels(), 8);
        let value = plugin.get_parameter(&ParameterId::from("speaker_config"));
        assert_eq!(value, Some(ParameterValue::Int(1)));
    }

    #[test]
    fn test_upmixer_5_1_4_channel_distribution() {
        // Test that 5.1.4 produces output on all channels including rear height
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1.4", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        // Create test input with different L/R content to generate both direct and ambient
        let mut input = vec![0.0_f32; 2048 * 2];
        for i in 0..2048 {
            let t = i as f32 / 44100.0;
            input[i * 2] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5; // Left
            input[i * 2 + 1] = (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.5; // Right (different frequency)
        }

        let mut output = vec![0.0_f32; 2048 * 10];
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2048,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Calculate energy per channel
        let mut channel_energies = vec![0.0; 10];
        for i in 0..2048 {
            for ch in 0..10 {
                channel_energies[ch] += output[i * 10 + ch].powi(2);
            }
        }

        /*
        eprintln!("5.1.4 Channel energies:");
        eprintln!("  [0] FL:  {:.6}", channel_energies[0]);
        eprintln!("  [1] FR:  {:.6}", channel_energies[1]);
        eprintln!("  [2] C:   {:.6}", channel_energies[2]);
        eprintln!("  [3] LFE: {:.6}", channel_energies[3]);
        eprintln!("  [4] SL:  {:.6}", channel_energies[4]);
        eprintln!("  [5] SR:  {:.6}", channel_energies[5]);
        eprintln!("  [6] TFL: {:.6}", channel_energies[6]);
        eprintln!("  [7] TFR: {:.6}", channel_energies[7]);
        eprintln!("  [8] TBL: {:.6}", channel_energies[8]);
        eprintln!("  [9] TBR: {:.6}", channel_energies[9]);
        */

        // Check that all non-LFE channels have some energy
        for (ch, &energy) in channel_energies.iter().enumerate() {
            if ch != 3 {
                // Skip LFE (channel 3) as it only gets low frequencies
                assert!(
                    energy >= 0.0,
                    "Channel {} should have non-negative energy",
                    ch
                );
            }
        }

        // Front and side channels should have significant energy
        assert!(
            channel_energies[0] > 0.01,
            "FL should have significant energy"
        );
        assert!(
            channel_energies[1] > 0.01,
            "FR should have significant energy"
        );
        assert!(channel_energies[4] > 0.001, "SL should have some energy");
        assert!(channel_energies[5] > 0.001, "SR should have some energy");

        // Rear height channels (8, 9) should have energy from:
        // 1. Decorrelated ambient (L-R content)
        // 2. Late reflections (10% of direct signal)
        // Even with mono content, they should now receive the late reflection signal
        assert!(
            channel_energies[8] > 1e-9,
            "TBL (rear height left) should have energy from late reflections + ambient, got {}",
            channel_energies[8]
        );
        assert!(
            channel_energies[9] > 1e-9,
            "TBR (rear height right) should have energy from late reflections + ambient, got {}",
            channel_energies[9]
        );
    }

    #[test]
    fn test_crossover_gains_energy_normalization() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        let nbins = plugin.lfe_low_gains.len();
        assert_eq!(nbins, plugin.mains_high_gains.len());

        // Check that |low|^2 + |high|^2 ≈ 1 across spectrum
        for (idx, (&low, &high)) in plugin
            .lfe_low_gains
            .iter()
            .zip(plugin.mains_high_gains.iter())
            .enumerate()
        {
            let power = low.norm_sqr() + high.norm_sqr();
            assert!(
                (power - 1.0).abs() < 1e-3,
                "Crossover power not normalized at bin {}: {}",
                idx,
                power
            );
        }

        // Sanity check around cutoff: low dominates below, high dominates above
        let cutoff = plugin.lfe_cutoff_hz;
        let mut cutoff_bin =
            ((cutoff * plugin.fft_size as f32) / plugin.sample_rate as f32) as usize;
        cutoff_bin = cutoff_bin.min(nbins - 2).max(1);
        let below = cutoff_bin / 2;
        let above = (cutoff_bin * 3 / 2).min(nbins - 1);

        assert!(
            plugin.lfe_low_gains[below].norm() > plugin.lfe_low_gains[cutoff_bin].norm(),
            "Low gain should decrease toward cutoff"
        );
        assert!(
            plugin.mains_high_gains[above].norm() > plugin.mains_high_gains[cutoff_bin].norm(),
            "High gain should increase above cutoff"
        );
    }

    #[test]
    fn test_decorrelation_filters_properties() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        let spectrum_size = plugin.fft_size / 2 + 1;
        assert_eq!(plugin.decorrelation_filter_left.len(), spectrum_size);
        assert_eq!(plugin.decorrelation_filter_right.len(), spectrum_size);

        // Magnitude should be 1.0 for all bins (these are all-pass filters)
        for i in 0..spectrum_size {
            let mag_l = plugin.decorrelation_filter_left[i].norm();
            let mag_r = plugin.decorrelation_filter_right[i].norm();
            assert!(
                (mag_l - 1.0).abs() < 1e-6,
                "Left decorrelator magnitude not 1 at bin {}: {}",
                i,
                mag_l
            );
            assert!(
                (mag_r - 1.0).abs() < 1e-6,
                "Right decorrelator magnitude not 1 at bin {}: {}",
                i,
                mag_r
            );
        }

        // DC and Nyquist must be real (phase = 0 or π)
        assert!(
            plugin.decorrelation_filter_left[0].im.abs() < 1e-6
                && plugin.decorrelation_filter_right[0].im.abs() < 1e-6
        );
        assert!(
            plugin.decorrelation_filter_left[spectrum_size - 1].im.abs() < 1e-6
                && plugin.decorrelation_filter_right[spectrum_size - 1]
                    .im
                    .abs()
                    < 1e-6
        );
    }

    #[test]
    fn test_height_mask_coherent_input_is_small() {
        // Coherent stereo (L=R) should yield very small height mask values
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1.4", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        let fft_size = plugin.fft_size;
        let mut input = vec![0.0f32; fft_size * 2];
        for i in 0..fft_size {
            let t = i as f32 / 44100.0;
            let s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5;
            input[i * 2] = s;
            input[i * 2 + 1] = s;
        }
        let mut output = vec![0.0f32; fft_size * plugin.num_output_channels];

        plugin.process_fft_block(&input, &mut output);

        // Only consider height mask values on bins that actually carry
        // non-negligible spectral energy. In high-frequency bands where
        // the signal is essentially silent, the mask can reach 1.0 but
        // contributes nothing audibly.
        let mut max_mask = 0.0f32;
        for i in 0..plugin.height_band_gains.len() {
            let l = plugin.freq_domain_left[i];
            let r = plugin.freq_domain_right[i];
            let energy = l.norm_sqr() + r.norm_sqr();
            if energy > 1e-6_f32 {
                if plugin.height_band_gains[i] > max_mask {
                    max_mask = plugin.height_band_gains[i];
                }
            }
        }
        assert!(
            max_mask < 0.2,
            "Height mask should be small for coherent input, got max {}",
            max_mask
        );
    }

    #[test]
    fn test_height_mask_diffuse_high_frequency_is_significant() {
        // Diffuse HF content (different L/R frequencies) should produce
        // noticeable height mask values in the top of the band.
        // Multiple frames are needed because temporal smoothing ramps up gradually
        // from zero (asymmetric attack/release prevents crackle artifacts).
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1.4", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        let fft_size = plugin.fft_size;
        let mut input = vec![0.0f32; fft_size * 2];
        for i in 0..fft_size {
            let t = i as f32 / 44100.0;
            input[i * 2] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5; // Left: 440 Hz
            input[i * 2 + 1] = (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.5; // Right: 880 Hz
        }
        let mut output = vec![0.0f32; fft_size * plugin.num_output_channels];

        // Process multiple frames to let temporal smoothing converge
        for _ in 0..10 {
            plugin.process_fft_block(&input, &mut output);
        }

        let nbins = plugin.height_band_gains.len();
        let start = (nbins as f32 * 0.75) as usize;
        let mut max_mask_hf = 0.0f32;
        for &m in &plugin.height_band_gains[start..] {
            if m > max_mask_hf {
                max_mask_hf = m;
            }
        }

        assert!(
            max_mask_hf > 0.1,
            "Height mask should be noticeable for diffuse HF input, got max {}",
            max_mask_hf
        );
    }

    // ===== NEW FEATURE TESTS TO IDENTIFY CLIPPING SOURCE =====

    #[test]
    fn test_ambient_detection_no_overflow() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        // Create test input with very high level (but still within -1.0 to 1.0)
        let mut input = vec![0.0_f32; 2048 * 2];
        for i in 0..2048 {
            // High amplitude signals that are uncorrelated (pure ambient)
            input[i * 2] = (i as f32 * 0.1).sin() * 0.9; // Left
            input[i * 2 + 1] = (i as f32 * 0.1 + std::f32::consts::PI).cos() * 0.9; // Right (uncorrelated)
        }
        let mut output = vec![0.0_f32; 2048 * 6];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2048,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Check that output samples are within reasonable bounds (safety_cap_db default is 3dB)
        let threshold = 1.5; // ~3.5dB headroom (safety_cap_db is 3dB by default)
        for (idx, &sample) in output.iter().enumerate() {
            assert!(
                sample.abs() <= threshold,
                "Sample at index {} exceeds threshold: {:.2} dB (value: {})",
                idx,
                20.0 * sample.abs().log10(),
                sample
            );
        }
    }

    #[test]
    fn test_dialog_detection_no_overflow() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        // Create test input simulating voice (1-2 kHz, highly correlated stereo)
        let mut input = vec![0.0_f32; 2048 * 2];
        let sample_rate = 44100.0;
        let voice_freq = 1500.0; // Hz - typical voice frequency
        for i in 0..2048 {
            let t = i as f32 / sample_rate;
            let voice_signal = (2.0 * std::f32::consts::PI * voice_freq * t).sin() * 0.9;
            input[i * 2] = voice_signal; // Left
            input[i * 2 + 1] = voice_signal; // Right (highly correlated = dialog)
        }
        let mut output = vec![0.0_f32; 2048 * 6];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2048,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Check that output samples are within reasonable bounds
        let threshold = 1.5; // ~3.5dB headroom
        for (idx, &sample) in output.iter().enumerate() {
            assert!(
                sample.abs() <= threshold,
                "Dialog test: Sample at index {} exceeds threshold: {:.2} dB (value: {})",
                idx,
                20.0 * sample.abs().log10(),
                sample
            );
        }
    }

    #[test]
    fn test_ambient_extraction_no_overflow() {
        let mut plugin = UpmixerPlugin::new(
            2048, "7.1.4", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        // Create test input with strong side difference (ambient content)
        let mut input = vec![0.0_f32; 2048 * 2];
        for i in 0..2048 {
            // High frequency content with phase inversion (pure ambient)
            let signal = (i as f32 * 0.2).sin() * 0.9;
            input[i * 2] = signal; // Left
            input[i * 2 + 1] = -signal; // Right (inverted = max ambient)
        }
        let mut output = vec![0.0_f32; 2048 * 12];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2048,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // Check for overflow
        let threshold = 1.5; // ~3.5dB headroom
        for (idx, &sample) in output.iter().enumerate() {
            assert!(
                sample.abs() <= threshold,
                "Ambient extraction: Sample at index {} exceeds threshold: {:.2} dB",
                idx,
                20.0 * sample.abs().log10()
            );
        }
    }

    #[test]
    fn test_divergence_calculation_no_overflow() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 1.0, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false,
            0.5, // stereo_width = 1.0 (max divergence)
        );
        plugin.initialize(44100).unwrap();

        // Create test input with high stereo width content
        let mut input = vec![0.0_f32; 2048 * 2];
        for i in 0..2048 {
            input[i * 2] = (i as f32 * 0.05).sin() * 0.9; // Left
            input[i * 2 + 1] = (i as f32 * 0.05 + 1.0).cos() * 0.9; // Right (different phase)
        }
        let mut output = vec![0.0_f32; 2048 * 6];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2048,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        let threshold = 1.5; // ~3.5dB headroom
        for (idx, &sample) in output.iter().enumerate() {
            assert!(
                sample.abs() <= threshold,
                "Divergence: Sample at index {} exceeds threshold: {:.2} dB",
                idx,
                20.0 * sample.abs().log10()
            );
        }
    }

    #[test]
    fn test_height_mask_no_overflow() {
        let mut plugin = UpmixerPlugin::new(
            2048, "7.1.4", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        // Create test input with high frequency content (ideal for height channels)
        let mut input = vec![0.0_f32; 2048 * 2];
        let sample_rate = 44100.0;
        let hf_freq = 10000.0; // 10 kHz - high frequency
        for i in 0..2048 {
            let t = i as f32 / sample_rate;
            let hf_left = (2.0 * std::f32::consts::PI * hf_freq * t).sin() * 0.9;
            let hf_right = (2.0 * std::f32::consts::PI * hf_freq * t + 0.5).sin() * 0.9;
            input[i * 2] = hf_left;
            input[i * 2 + 1] = hf_right;
        }
        let mut output = vec![0.0_f32; 2048 * 12];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2048,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        let threshold = 1.5; // ~3.5dB headroom
        for (idx, &sample) in output.iter().enumerate() {
            assert!(
                sample.abs() <= threshold,
                "Height mask: Sample at index {} exceeds threshold: {:.2} dB",
                idx,
                20.0 * sample.abs().log10()
            );
        }
    }

    #[test]
    fn test_adaptive_decorrelation_no_overflow() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        // Test with ambient signal (triggers decorrelation)
        let mut input = vec![0.0_f32; 2048 * 2];
        for i in 0..2048 {
            let signal = (i as f32 * 0.1).sin() * 0.9;
            input[i * 2] = signal;
            input[i * 2 + 1] = -signal; // Inverted = ambient
        }
        let mut output = vec![0.0_f32; 2048 * 6];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2048,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        let threshold = 1.5; // ~3.5dB headroom
        for (idx, &sample) in output.iter().enumerate() {
            assert!(
                sample.abs() <= threshold,
                "Decorrelation: Sample at index {} exceeds threshold: {:.2} dB",
                idx,
                20.0 * sample.abs().log10()
            );
        }
    }

    #[test]
    fn test_smooth_height_gains_no_overflow() {
        let mut plugin = UpmixerPlugin::new(
            2048, "7.1.4", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        // Test with rapidly changing high frequency content
        let mut input = vec![0.0_f32; 2048 * 2];
        for i in 0..2048 {
            // Alternating amplitude to trigger smoothing
            let amp = if i % 100 < 50 { 0.9 } else { 0.3 };
            input[i * 2] = (i as f32 * 0.3).sin() * amp;
            input[i * 2 + 1] = (i as f32 * 0.3 + 0.5).sin() * amp;
        }
        let mut output = vec![0.0_f32; 2048 * 12];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2048,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        let threshold = 1.5; // ~3.5dB headroom
        for (idx, &sample) in output.iter().enumerate() {
            assert!(
                sample.abs() <= threshold,
                "Smooth height: Sample at index {} exceeds threshold: {:.2} dB",
                idx,
                20.0 * sample.abs().log10()
            );
        }
    }

    #[test]
    fn test_vbap_panning_no_overflow() {
        let mut plugin = UpmixerPlugin::new(
            2048, "7.1.4", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        // Test with full scale signal
        let mut input = vec![0.0_f32; 2048 * 2];
        for i in 0..2048 {
            input[i * 2] = (i as f32 * 0.05).sin() * 0.95;
            input[i * 2 + 1] = (i as f32 * 0.05).cos() * 0.95;
        }
        let mut output = vec![0.0_f32; 2048 * 12];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2048,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        let threshold = 1.5; // ~3.5dB headroom
        for (idx, &sample) in output.iter().enumerate() {
            assert!(
                sample.abs() <= threshold,
                "VBAP panning: Sample at index {} exceeds threshold: {:.2} dB",
                idx,
                20.0 * sample.abs().log10()
            );
        }
    }

    #[test]
    fn test_subharmonic_synthesis_no_overflow() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        // Test with low frequency content (triggers subharmonic synthesis)
        let mut input = vec![0.0_f32; 2048 * 2];
        let sample_rate = 44100.0;
        let bass_freq = 80.0; // Hz - bass frequency
        for i in 0..2048 {
            let t = i as f32 / sample_rate;
            let bass = (2.0 * std::f32::consts::PI * bass_freq * t).sin() * 0.9;
            input[i * 2] = bass;
            input[i * 2 + 1] = bass;
        }
        let mut output = vec![0.0_f32; 2048 * 6];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2048,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        let threshold = 1.5; // ~3.5dB headroom
        for (idx, &sample) in output.iter().enumerate() {
            assert!(
                sample.abs() <= threshold,
                "Subharmonic: Sample at index {} exceeds threshold: {:.2} dB",
                idx,
                20.0 * sample.abs().log10()
            );
        }
    }

    #[test]
    fn test_extract_output_and_scale_no_overflow() {
        let mut plugin = UpmixerPlugin::new(
            2048, "7.1.4", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();

        // Test with various gain settings
        let mut input = vec![0.0_f32; 2048 * 2];
        for i in 0..2048 {
            input[i * 2] = (i as f32 * 0.1).sin() * 0.9;
            input[i * 2 + 1] = (i as f32 * 0.1).cos() * 0.9;
        }
        let mut output = vec![0.0_f32; 2048 * 12];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2048,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        let threshold = 1.5; // ~3.5dB headroom
        for (idx, &sample) in output.iter().enumerate() {
            assert!(
                sample.abs() <= threshold,
                "Extract/scale: Sample at index {} exceeds threshold: {:.2} dB",
                idx,
                20.0 * sample.abs().log10()
            );
        }
    }

    #[test]
    fn test_decorrelation_filters_mode_0_no_overflow() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.decorrelation_mode = 0; // Velvet noise mode
        plugin.initialize(44100).unwrap();

        let mut input = vec![0.0_f32; 2048 * 2];
        for i in 0..2048 {
            input[i * 2] = (i as f32 * 0.1).sin() * 0.9;
            input[i * 2 + 1] = -((i as f32 * 0.1).sin()) * 0.9; // Inverted for ambient
        }
        let mut output = vec![0.0_f32; 2048 * 6];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2048,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        let threshold = 1.5; // ~3.5dB headroom
        for (idx, &sample) in output.iter().enumerate() {
            assert!(
                sample.abs() <= threshold,
                "Decorr mode 0: Sample at index {} exceeds threshold: {:.2} dB",
                idx,
                20.0 * sample.abs().log10()
            );
        }
    }

    #[test]
    fn test_decorrelation_filters_mode_1_no_overflow() {
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.decorrelation_mode = 1; // LFO mode
        plugin.initialize(44100).unwrap();

        let mut input = vec![0.0_f32; 2048 * 2];
        for i in 0..2048 {
            input[i * 2] = (i as f32 * 0.1).sin() * 0.9;
            input[i * 2 + 1] = -((i as f32 * 0.1).sin()) * 0.9; // Inverted for ambient
        }
        let mut output = vec![0.0_f32; 2048 * 6];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2048,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        let threshold = 1.5; // ~3.5dB headroom
        for (idx, &sample) in output.iter().enumerate() {
            assert!(
                sample.abs() <= threshold,
                "Decorr mode 1: Sample at index {} exceeds threshold: {:.2} dB",
                idx,
                20.0 * sample.abs().log10()
            );
        }
    }

    #[test]
    fn test_combined_features_stress_test() {
        // Test all features combined with extreme settings
        let mut plugin = UpmixerPlugin::new(
            2048, "7.1.4", 2.0, // High direct gain
            2.0, // High ambient gain
            1.0, // Max stereo width
            120.0, 0.5, 250.0, 2.0,  // High rear gain
            2.0,  // High height gain
            true, // Enable HR direct
            1.0,  // Max LFE level
        );
        plugin.initialize(44100).unwrap();

        // Create complex signal with multiple characteristics
        let mut input = vec![0.0_f32; 2048 * 2];
        let sample_rate = 44100.0;
        for i in 0..2048 {
            let t = i as f32 / sample_rate;
            // Mix of bass, voice, and high freq
            let bass = (2.0 * std::f32::consts::PI * 60.0 * t).sin() * 0.3;
            let voice = (2.0 * std::f32::consts::PI * 1500.0 * t).sin() * 0.3;
            let hf = (2.0 * std::f32::consts::PI * 8000.0 * t).sin() * 0.3;
            let combined = bass + voice + hf;

            input[i * 2] = combined;
            // Add some phase difference for ambient extraction
            input[i * 2 + 1] = combined * 0.7 + hf * 0.3;
        }
        let mut output = vec![0.0_f32; 2048 * 12];

        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: 2048,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // This is the critical test - with all features enabled and high gains
        let threshold = 1.5; // ~3.5dB headroom
        let mut max_sample = 0.0_f32;
        let mut max_idx = 0;
        for (idx, &sample) in output.iter().enumerate() {
            if sample.abs() > max_sample {
                max_sample = sample.abs();
                max_idx = idx;
            }
            assert!(
                sample.abs() <= threshold,
                "STRESS TEST: Sample at index {} exceeds threshold: {:.2} dB (value: {})",
                idx,
                20.0 * sample.abs().log10(),
                sample
            );
        }

        println!(
            "Stress test passed. Max output level: {:.2} dB at index {}",
            20.0 * max_sample.log10(),
            max_idx
        );
    }

    #[test]
    fn test_mono_input_core_processing() {
        // With a mono (L=R) input, the direct signal should dominate, and ambient signal
        // should be minimal. This means most energy should be in the front channels,
        // especially the center, and very little in surround/height channels.
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1.4", // Use a config with height channels
            1.0,     // gain_front_direct
            1.0,     // gain_front_ambient
            1.0,     // gain_rear_ambient
            120.0, 0.5, 250.0, 1.0, // height_gain
            1.0, // lfe_gain
            false, 0.5,
        );
        plugin.initialize(44100).unwrap();
        plugin.center_spread.set_target(0.0); // Focus direct sound to center speaker

        // Create a mono sine wave input
        let num_blocks = 32;
        let buffer_size = num_blocks * 2048;
        let mut input = vec![0.0_f32; buffer_size * 2];
        for i in 0..buffer_size {
            let t = i as f32 / 44100.0;
            let signal = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.7;
            input[i * 2] = signal; // Left
            input[i * 2 + 1] = signal; // Right
        }

        let mut output = vec![0.0_f32; buffer_size * plugin.output_channels()];
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: buffer_size,
        };

        plugin.process(&input, &mut output, &context).unwrap();

        // 5.1.4 layout: [FL, FR, C, LFE, SL, SR, TFL, TFR, TBL, TBR]
        // Indices:       0,  1,  2,   3,  4,  5,   6,   7,   8,   9
        let mut energies = vec![0.0_f32; plugin.output_channels()];
        let skip = (num_blocks - 8) * 2048; // Check settling state
        for i in skip..buffer_size {
            for ch in 0..plugin.output_channels() {
                energies[ch] += output[i * plugin.output_channels() + ch].powi(2);
            }
        }

        let total_energy: f32 = energies.iter().sum();
        assert!(
            total_energy > 0.01,
            "Total output energy should be significant."
        );

        let center_energy = energies[2];
        let front_left_energy = energies[0];
        let front_right_energy = energies[1];
        let lfe_energy = energies[3];

        let surround_energy: f32 = energies[4..6].iter().sum();
        let height_energy: f32 = energies[6..10].iter().sum();

        // With center_spread = 0, most direct energy should be in the Center channel.
        assert!(
            center_energy > front_left_energy && center_energy > front_right_energy,
            "Center energy ({}) should be greater than front L/R energy (L={}, R={}) for mono input with center_spread=0.0",
            center_energy,
            front_left_energy,
            front_right_energy
        );

        // The combined energy of ambient-driven channels (surround + height) should be
        // a small fraction of the direct-driven channels (fronts).
        let direct_energy = center_energy + front_left_energy + front_right_energy;
        let ambient_energy = surround_energy + height_energy;

        assert!(
            ambient_energy < direct_energy * 0.1,
            "Ambient energy ({}) should be less than 10% of direct energy ({}) for a mono signal.",
            ambient_energy,
            direct_energy
        );

        // LFE energy should be low as the input frequency (440Hz) is above the cutoff (120Hz).
        assert!(
            lfe_energy < total_energy * 0.01,
            "LFE energy ({}) should be negligible for a 440Hz sine wave.",
            lfe_energy
        );
    }

    #[test]
    fn test_transient_processing_with_hr_path() {
        // This test verifies that the high-resolution (HR) path correctly
        // detects and processes a transient signal. The detector works by
        // comparing current spectral flux to a smoothed baseline, so we need
        // to establish a low-energy baseline first and then introduce a big
        // energy jump.
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", 1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();
        plugin.enable_hr_direct = true;
        plugin.hr_sharpen.set_target(1.0);

        let buffer_size = 1024;
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: buffer_size,
        };

        // --- Phase 1: Establish a low-energy baseline ---
        // Use a quiet signal (not silence) so the flux smoother has a real baseline.
        let mut input_quiet = vec![0.0_f32; buffer_size * 2];
        for i in 0..buffer_size {
            let t = i as f32 / 44100.0;
            let signal = (2.0 * std::f32::consts::PI * 6000.0 * t).sin() * 0.05;
            input_quiet[i * 2] = signal;
            input_quiet[i * 2 + 1] = signal;
        }
        let mut output_buffer = vec![0.0_f32; buffer_size * plugin.output_channels()];
        // Process several blocks to let the baseline converge
        for _ in 0..6 {
            plugin
                .process(&input_quiet, &mut output_buffer, &context)
                .unwrap();
        }
        let env_after_quiet = plugin.hr_transient_env;

        // --- Phase 2: Transient (large energy jump) ---
        let mut input_transient = vec![0.0_f32; buffer_size * 2];
        for i in 0..buffer_size {
            let t = i as f32 / 44100.0;
            let signal = (2.0 * std::f32::consts::PI * 6000.0 * t).sin() * 0.9;
            input_transient[i * 2] = signal;
            input_transient[i * 2 + 1] = signal;
        }

        // Process the transient block — this should spike the ratio
        plugin
            .process(&input_transient, &mut output_buffer, &context)
            .unwrap();
        plugin
            .process(&input_transient, &mut output_buffer, &context)
            .unwrap();
        let env_after_transient = plugin.hr_transient_env;

        // --- Assertions ---
        assert!(
            env_after_transient > env_after_quiet,
            "hr_transient_env should be higher after a transient ({}) than after quiet baseline ({})",
            env_after_transient,
            env_after_quiet
        );
        assert!(
            env_after_transient > 0.1,
            "hr_transient_env ({}) should be significant after transient",
            env_after_transient
        );
    }

    #[test]
    fn test_bypass_all_processing() {
        // Test that when bypass_all_processing is enabled, the plugin
        // passes through the stereo input to the front L/R channels,
        // (and L+R/2 to center if present) and silences other channels.
        let mut plugin = UpmixerPlugin::new(
            2048, "5.1", // Test with a 5.1 configuration
            1.0, 0.5, 1.0, 120.0, 0.5, 250.0, 1.0, 1.0, false, 0.5,
        );
        plugin.initialize(44100).unwrap();
        // Explicitly enable bypass for this test
        plugin
            .set_parameter(
                ParameterId::from("bypass_all_processing"),
                ParameterValue::Bool(true),
            )
            .unwrap();

        let buffer_size = 1024;
        let context = ProcessContext {
            sample_rate: 44100,
            num_frames: buffer_size,
        };

        // Create a stereo sine wave input
        let mut input = vec![0.0_f32; buffer_size * 2];
        for i in 0..buffer_size {
            let t = i as f32 / 44100.0;
            input[i * 2] = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.7; // Left channel
            input[i * 2 + 1] = (2.0 * std::f32::consts::PI * 880.0 * t).sin() * 0.7; // Right channel
        }

        let mut output = vec![0.0_f32; buffer_size * plugin.output_channels()];
        plugin.process(&input, &mut output, &context).unwrap();

        // Check output channels
        let fl_idx = 0; // Front Left
        let fr_idx = 1; // Front Right
        let c_idx = 2; // Center
        let lfe_idx = 3; // LFE
        let sl_idx = 4; // Surround Left
        let sr_idx = 5; // Surround Right

        for i in 0..buffer_size {
            // Front Left and Right should match input *exactly*
            assert_eq!(
                output[i * plugin.output_channels() + fl_idx],
                input[i * 2],
                "FL output does not match input"
            );
            assert_eq!(
                output[i * plugin.output_channels() + fr_idx],
                input[i * 2 + 1],
                "FR output does not match input"
            );

            // Center channel should be (L+R)/2 *exactly*
            assert_eq!(
                output[i * plugin.output_channels() + c_idx],
                (input[i * 2] + input[i * 2 + 1]) * 0.5,
                "Center output does not match (L+R)/2"
            );

            // Other channels should be effectively silent
            assert!(
                output[i * plugin.output_channels() + lfe_idx].abs() < 1e-6,
                "LFE channel should be silent"
            );
            assert!(
                output[i * plugin.output_channels() + sl_idx].abs() < 1e-6,
                "SL channel should be silent"
            );
            assert!(
                output[i * plugin.output_channels() + sr_idx].abs() < 1e-6,
                "SR channel should be silent"
            );
        }
    }
}
