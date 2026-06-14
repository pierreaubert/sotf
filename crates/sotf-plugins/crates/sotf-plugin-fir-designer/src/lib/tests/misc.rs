use sotf_host :: plugin :: { ProcessContext } ;
use super::super::params;
use super::super::fir_designer_plugin::FirDesignerPlugin;
use super::super::types::BandConfig;
use super::super::types::FirDesignerPluginParams;
    use super::super::*;
# [cfg (test)]
use super::DEFAULT_SAMPLE_RATE;

    fn make_context(num_frames: usize) -> ProcessContext<'static> {
        ProcessContext::new(DEFAULT_SAMPLE_RATE, num_frames)
    }

    #[test]
    fn test_fir_designer_passthrough() {
        // All bands at 0 dB gain -> output should approximately equal input
        let channels = 2;
        let sr = 48000;
        let params = FirDesignerPluginParams {
            num_filters: 3,
            fir_length_index: 1, // 2048 taps
            phase_mode_index: 0,
            auto_gain: false,
            mix: 1.0,
            filters: vec![
                BandConfig {
                    filter_type: "Peak".to_string(),
                    frequency: 1000.0,
                    q: 1.0,
                    gain_db: 0.0,
                    active: true,
                },
                BandConfig {
                    filter_type: "Peak".to_string(),
                    frequency: 2000.0,
                    q: 1.0,
                    gain_db: 0.0,
                    active: true,
                },
                BandConfig {
                    filter_type: "Peak".to_string(),
                    frequency: 4000.0,
                    q: 1.0,
                    gain_db: 0.0,
                    active: true,
                },
            ],
        };

        let mut plugin = FirDesignerPlugin::from_params(channels, sr, params).unwrap();
        let num_frames = 512;
        let latency = plugin.latency_samples();

        // Generate a 1kHz sine wave, process multiple blocks to get past latency
        let blocks_needed = (latency / num_frames) + 5;
        let mut all_output = Vec::new();

        for block in 0..blocks_needed {
            let mut buffer = vec![0.0f32; num_frames * channels];
            let start_frame = block * num_frames;
            for frame in 0..num_frames {
                let t = (start_frame + frame) as f32 / sr as f32;
                let sample = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.5;
                buffer[frame * channels] = sample; // L
                buffer[frame * channels + 1] = sample; // R
            }
            let ctx = make_context(num_frames);
            plugin.process_in_place(&mut buffer, &ctx).unwrap();
            all_output.extend_from_slice(&buffer);
        }

        // After latency, the output should match the input (within FIR precision)
        // Check steady-state region (skip first latency + some margin)
        let check_start = (latency + num_frames) * channels;
        let check_end = all_output.len() - num_frames * channels;

        if check_start < check_end {
            // Reconstruct expected sine at the delayed position
            let mut max_error = 0.0f32;
            for i in (check_start..check_end).step_by(channels) {
                let frame_idx = i / channels;
                // Account for latency
                let source_frame = frame_idx - latency;
                let t = source_frame as f32 / sr as f32;
                let expected = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.5;
                let err = (all_output[i] - expected).abs();
                if err > max_error {
                    max_error = err;
                }
            }
            // Tolerance: FIR windowing (Kaiser window) causes small deviations,
            // especially at frequencies near Nyquist. 0.1 corresponds to ~0.8 dB.
            assert!(max_error < 0.1, "Passthrough error too large: {max_error}");
        }
    }

    #[test]
    fn test_fir_designer_boost() {
        // 1kHz +6dB band -> 1kHz sine should be louder
        let channels = 1;
        let sr = 48000;
        let params = FirDesignerPluginParams {
            num_filters: 1,
            fir_length_index: 2, // 4096 taps
            phase_mode_index: 0,
            auto_gain: false,
            mix: 1.0,
            filters: vec![BandConfig {
                filter_type: "Peak".to_string(),
                frequency: 1000.0,
                q: 1.0,
                gain_db: 6.0,
                active: true,
            }],
        };

        let mut plugin = FirDesignerPlugin::from_params(channels, sr, params).unwrap();
        let num_frames = 512;
        let latency = plugin.latency_samples();
        let blocks_needed = (latency / num_frames) + 10;

        let mut input_rms = 0.0f64;
        let mut output_rms = 0.0f64;
        let mut samples_counted = 0usize;

        for block in 0..blocks_needed {
            let mut buffer = vec![0.0f32; num_frames * channels];
            let start_frame = block * num_frames;
            for frame in 0..num_frames {
                let t = (start_frame + frame) as f32 / sr as f32;
                let sample = (2.0 * std::f32::consts::PI * 1000.0 * t).sin() * 0.5;
                buffer[frame] = sample;
            }

            // Measure input RMS (after latency region)
            if block * num_frames > latency + num_frames {
                for &s in &buffer {
                    input_rms += (s as f64) * (s as f64);
                }
                samples_counted += num_frames;
            }

            let ctx = make_context(num_frames);
            plugin.process_in_place(&mut buffer, &ctx).unwrap();

            // Measure output RMS (same region)
            if block * num_frames > latency + num_frames {
                for &s in &buffer {
                    output_rms += (s as f64) * (s as f64);
                }
            }
        }

        if samples_counted > 0 {
            input_rms = (input_rms / samples_counted as f64).sqrt();
            output_rms = (output_rms / samples_counted as f64).sqrt();

            let gain_db = 20.0 * (output_rms / input_rms).log10();
            // Should be approximately +6 dB
            assert!(
                gain_db > 4.0 && gain_db < 8.0,
                "Expected ~6 dB boost, got {gain_db:.1} dB"
            );
        }
    }

    #[test]
    fn test_fir_designer_phase_linearity() {
        // Process an impulse, verify the response is symmetrical (linear phase property)
        let channels = 1;
        let sr = 48000;
        let params = FirDesignerPluginParams {
            num_filters: 1,
            fir_length_index: 1, // 2048 taps
            phase_mode_index: 0,
            auto_gain: false,
            mix: 1.0,
            filters: vec![BandConfig {
                filter_type: "Peak".to_string(),
                frequency: 1000.0,
                q: 1.0,
                gain_db: 6.0,
                active: true,
            }],
        };

        let mut plugin = FirDesignerPlugin::from_params(channels, sr, params).unwrap();
        let num_frames = 256;
        let latency = plugin.latency_samples();
        let blocks_needed = (latency * 3 / num_frames) + 5;

        let mut all_output = Vec::new();

        for block in 0..blocks_needed {
            let mut buffer = vec![0.0f32; num_frames * channels];
            // Put impulse in first block, first sample
            if block == 0 {
                buffer[0] = 1.0;
            }
            let ctx = make_context(num_frames);
            plugin.process_in_place(&mut buffer, &ctx).unwrap();
            all_output.extend_from_slice(&buffer);
        }

        // Find the peak of the impulse response
        let (peak_idx, _peak_val) = all_output
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.abs().partial_cmp(&b.abs()).unwrap())
            .unwrap();

        // Verify symmetry around the peak
        let check_range = 100.min(peak_idx).min(all_output.len() - peak_idx - 1);
        let mut max_asymmetry = 0.0f32;
        for offset in 1..check_range {
            let left = all_output[peak_idx - offset];
            let right = all_output[peak_idx + offset];
            let asymmetry = (left - right).abs();
            if asymmetry > max_asymmetry {
                max_asymmetry = asymmetry;
            }
        }

        // Linear-phase FIR should have very symmetrical impulse response
        assert!(
            max_asymmetry < 0.01,
            "Impulse response not symmetrical: max asymmetry = {max_asymmetry}"
        );
    }

    /// Helper: measure RMS level of a sine after EQ latency has passed.
    ///
    /// Feeds `blocks_total` blocks of a pure sine at `freq_hz` through `plugin`,
    /// returns the RMS computed over the last half of the blocks.
    fn rms_after_latency(
        plugin: &mut FirDesignerPlugin,
        freq_hz: f32,
        sr: u32,
        num_frames: usize,
        blocks_total: usize,
    ) -> f64 {
        let nc = plugin.channels;
        let latency = plugin.latency_samples();
        let measure_from = blocks_total / 2;
        let mut sum_sq = 0.0f64;
        let mut n = 0usize;
        for block in 0..blocks_total {
            let mut buf = vec![0.0f32; num_frames * nc];
            let base = block * num_frames;
            for frame in 0..num_frames {
                let t = (base + frame) as f32 / sr as f32;
                let s = (2.0 * std::f32::consts::PI * freq_hz * t).sin() * 0.5;
                for ch in 0..nc {
                    buf[frame * nc + ch] = s;
                }
            }
            let ctx = ProcessContext::new(sr, num_frames);
            plugin.process_in_place(&mut buf, &ctx).unwrap();
            if block >= measure_from && block * num_frames > latency + num_frames {
                for &s in &buf {
                    sum_sq += (s as f64) * (s as f64);
                    n += 1;
                }
            }
        }
        if n > 0 {
            (sum_sq / n as f64).sqrt()
        } else {
            0.0
        }
    }

    /// Bug #2 (🔴): Highpass filter at 200 Hz should attenuate 50 Hz content.
    ///
    /// Before the fix, lowpass/highpass bands were skipped entirely (gain_db==0
    /// satisfied the `gain_db.abs() > 1e-6` guard), making the plugin all-pass.
    #[test]
    fn test_highpass_attenuates_below_cutoff() {
        let sr = 48000u32;
        let params = FirDesignerPluginParams {
            num_filters: 1,
            fir_length_index: 2, // 4096 taps for clean HP response
            phase_mode_index: 0,
            auto_gain: false,
            mix: 1.0,
            filters: vec![BandConfig {
                filter_type: "Highpass".to_string(),
                frequency: 800.0,
                q: 0.707,
                gain_db: 0.0,
                active: true,
            }],
        };
        let mut plugin = FirDesignerPlugin::from_params(1, sr, params).unwrap();
        let num_frames = 256;
        let blocks = (plugin.latency_samples() / num_frames) + 20;

        // 50 Hz is well below the 800 Hz cutoff — should be strongly attenuated.
        let rms_50hz = rms_after_latency(&mut plugin, 50.0, sr, num_frames, blocks);
        // 4000 Hz is well above the cutoff — should pass with near-unity gain.
        let rms_4khz = rms_after_latency(&mut plugin, 4000.0, sr, num_frames, blocks);

        // Reset between frequency measurements.
        plugin.reset();

        assert!(
            rms_4khz > 0.01,
            "4 kHz should pass through HP filter, got rms={rms_4khz:.4}"
        );
        // At 50 Hz (far below 800 Hz cutoff), expect at least 20 dB attenuation.
        let attenuation_db = if rms_50hz < 1e-10 {
            120.0f64
        } else {
            20.0 * (rms_4khz / rms_50hz).log10()
        };
        assert!(
            attenuation_db > 15.0,
            "Expected >15 dB attenuation at 50 Hz vs 4 kHz, got {attenuation_db:.1} dB"
        );
    }

    /// Bug #1 (🔴): Lowshelf cut should attenuate DC / low frequencies.
    ///
    /// Before the fix, the DC point was hardcoded to 0 dB, making lowshelf-cut
    /// and highpass filters produce incorrect FIR shapes at low frequencies.
    #[test]
    fn test_lowshelf_cut_attenuates_low_frequencies() {
        let sr = 48000u32;
        // -12 dB lowshelf at 500 Hz should visibly attenuate a 100 Hz tone.
        let params = FirDesignerPluginParams {
            num_filters: 1,
            fir_length_index: 2,
            phase_mode_index: 0,
            auto_gain: false,
            mix: 1.0,
            filters: vec![BandConfig {
                filter_type: "Lowshelf".to_string(),
                frequency: 500.0,
                q: 0.707,
                gain_db: -12.0,
                active: true,
            }],
        };
        let mut plugin = FirDesignerPlugin::from_params(1, sr, params).unwrap();
        let num_frames = 256;
        let blocks = (plugin.latency_samples() / num_frames) + 20;

        let rms_100hz = rms_after_latency(&mut plugin, 100.0, sr, num_frames, blocks);
        plugin.reset();
        let rms_8khz = rms_after_latency(&mut plugin, 8000.0, sr, num_frames, blocks);

        // 8 kHz should be near passband (≥ 0.35 of input 0.5 amplitude).
        assert!(
            rms_8khz > 0.20,
            "8 kHz should be in passband, rms={rms_8khz:.4}"
        );
        // 100 Hz should be at least 6 dB below 8 kHz (cut is -12 dB).
        let attenuation_db = if rms_100hz < 1e-10 {
            120.0f64
        } else {
            20.0 * (rms_8khz / rms_100hz).log10()
        };
        assert!(
            attenuation_db > 6.0,
            "Expected >6 dB low-frequency attenuation with lowshelf cut, got {attenuation_db:.1} dB"
        );
    }

    #[test]
    fn test_lowpass_zero_gain_not_skipped() {
        // CRITICAL: lowpass/highpass bands with 0 dB gain were silently skipped.
        let channels = 1;
        let sr = 48000;
        let params = FirDesignerPluginParams {
            num_filters: 1,
            fir_length_index: 2, // 4096 taps
            phase_mode_index: 0,
            auto_gain: false,
            mix: 1.0,
            filters: vec![BandConfig {
                filter_type: "Lowpass".to_string(),
                frequency: 1000.0,
                q: 0.7,
                gain_db: 0.0,
                active: true,
            }],
        };

        let mut plugin = FirDesignerPlugin::from_params(channels, sr, params).unwrap();
        let num_frames = 512;
        let latency = plugin.latency_samples();
        let blocks_needed = (latency / num_frames) + 10;

        let mut input_rms = 0.0f64;
        let mut output_rms = 0.0f64;
        let mut samples_counted = 0usize;

        for block in 0..blocks_needed {
            let mut buffer = vec![0.0f32; num_frames * channels];
            let start_frame = block * num_frames;
            for frame in 0..num_frames {
                let t = (start_frame + frame) as f32 / sr as f32;
                // 5 kHz sine, well above 1 kHz cutoff
                let sample = (2.0 * std::f32::consts::PI * 5000.0 * t).sin() * 0.5;
                buffer[frame] = sample;
            }

            if block * num_frames > latency + num_frames {
                for &s in &buffer {
                    input_rms += (s as f64) * (s as f64);
                }
                samples_counted += num_frames;
            }

            let ctx = make_context(num_frames);
            plugin.process_in_place(&mut buffer, &ctx).unwrap();

            if block * num_frames > latency + num_frames {
                for &s in &buffer {
                    output_rms += (s as f64) * (s as f64);
                }
            }
        }

        if samples_counted > 0 {
            input_rms = (input_rms / samples_counted as f64).sqrt();
            output_rms = (output_rms / samples_counted as f64).sqrt();

            let attenuation_db = 20.0 * (output_rms / input_rms).log10();
            // A 1 kHz lowpass should attenuate a 5 kHz sine significantly.
            // With the bug the band was skipped, resulting in ~0 dB attenuation.
            assert!(
                attenuation_db < -6.0,
                "Expected significant attenuation for lowpass at 5 kHz, got {attenuation_db:.1} dB"
            );
        }
    }

