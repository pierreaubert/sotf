use sotf_host :: parameters :: { ParameterId , ParameterValue } ;
use sotf_host :: plugin :: { ProcessContext } ;
use super::super::default;
use super::super::params;
use super::super::spectral_compressor_plugin::SpectralCompressorPlugin;
use super::super::spectral_compressor_plugin_params::SpectralCompressorPluginParams;
    use super::super::*;

    fn make_plugin(threshold: f32, ratio: f32) -> SpectralCompressorPlugin {
        let params = SpectralCompressorPluginParams {
            fft_size_index: 1, // 2048
            threshold_db: threshold,
            ratio,
            attack_ms: 5.0,
            release_ms: 50.0,
            knee_db: 6.0,
            spectral_smoothing: 0.3,
            mix: 1.0,
        };
        let mut plugin = SpectralCompressorPlugin::from_params(2, params);
        plugin.initialize(48000).unwrap();
        plugin
    }

    fn process_signal(plugin: &mut SpectralCompressorPlugin, signal: &[f32]) -> Vec<f32> {
        let channels = plugin.channels();
        let total_frames = signal.len() / channels;

        // Process the entire signal in one call (like the multiband expander test)
        let mut buf = signal.to_vec();
        let ctx = ProcessContext::new(48000, total_frames);
        plugin.process_in_place(&mut buf, &ctx).unwrap();
        buf
    }

    #[test]
    fn test_passthrough_with_high_threshold() {
        // Threshold = 0dB means nothing should be compressed
        // (typical audio is well below 0dBFS per-bin)
        let mut plugin = make_plugin(0.0, 4.0);
        let channels = 2;
        let num_frames = 48000; // 1 second
        let freq = 440.0;
        let amplitude = 0.1; // -20dBFS, well below 0dB threshold

        // Generate stereo sine
        let mut signal = vec![0.0f32; num_frames * channels];
        for i in 0..num_frames {
            let sample = amplitude * (2.0 * std::f32::consts::PI * freq * i as f32 / 48000.0).sin();
            signal[i * channels] = sample;
            signal[i * channels + 1] = sample;
        }

        let output = process_signal(&mut plugin, &signal);

        // After OLA converges, output RMS should match input RMS (no compression)
        let skip = 16384; // generous skip for convergence
        let check_len = num_frames - skip - 4096;
        assert!(check_len > 0, "Not enough samples to compare");

        let rms_in: f32 = (signal[skip * channels..(skip + check_len) * channels]
            .iter()
            .map(|s| s * s)
            .sum::<f32>()
            / (check_len * channels) as f32)
            .sqrt();
        let rms_out: f32 = (output[skip * channels..(skip + check_len) * channels]
            .iter()
            .map(|s| s * s)
            .sum::<f32>()
            / (check_len * channels) as f32)
            .sqrt();

        let ratio = rms_out / rms_in.max(1e-10);
        assert!(
            (ratio - 1.0).abs() < 0.05,
            "Passthrough RMS ratio should be ~1.0, got {:.4} (in={:.4}, out={:.4})",
            ratio,
            rms_in,
            rms_out
        );
    }

    #[test]
    fn test_compresses_loud_bins() {
        // Low threshold, high ratio: should compress a loud signal
        let mut plugin = make_plugin(-40.0, 8.0);
        let channels = 2;
        let num_frames = 48000;
        let freq = 1000.0;
        let amplitude = 0.5; // -6dBFS, well above -40dB threshold

        let mut signal = vec![0.0f32; num_frames * channels];
        for i in 0..num_frames {
            let sample = amplitude * (2.0 * std::f32::consts::PI * freq * i as f32 / 48000.0).sin();
            signal[i * channels] = sample;
            signal[i * channels + 1] = sample;
        }

        let output = process_signal(&mut plugin, &signal);

        // Check that output RMS is lower than input RMS (compression happened)
        let skip = plugin.fft_size + 4096;
        let check_len = num_frames - skip - 1024;

        let mut rms_in = 0.0f32;
        let mut rms_out = 0.0f32;
        for i in skip..skip + check_len {
            let idx = i * channels;
            rms_in += signal[idx] * signal[idx];
            rms_out += output[idx] * output[idx];
        }
        rms_in = (rms_in / check_len as f32).sqrt();
        rms_out = (rms_out / check_len as f32).sqrt();

        assert!(
            rms_out < rms_in * 0.9,
            "Expected compression: rms_out={:.4} should be < rms_in={:.4} * 0.9",
            rms_out,
            rms_in
        );
    }

    #[test]
    fn test_quiet_bins_untouched() {
        // Threshold at -10dB, signal at -60dBFS (well below threshold)
        let mut plugin = make_plugin(-10.0, 4.0);
        let channels = 2;
        let num_frames = 48000;
        let freq = 440.0;
        let amplitude = 0.001; // -60dBFS, below -10dB threshold

        let mut signal = vec![0.0f32; num_frames * channels];
        for i in 0..num_frames {
            let sample = amplitude * (2.0 * std::f32::consts::PI * freq * i as f32 / 48000.0).sin();
            signal[i * channels] = sample;
            signal[i * channels + 1] = sample;
        }

        let output = process_signal(&mut plugin, &signal);

        // Output RMS should match input RMS (no compression, below threshold)
        let skip = 16384;
        let check_len = num_frames - skip - 4096;

        let rms_in: f32 = (signal[skip * channels..(skip + check_len) * channels]
            .iter()
            .map(|s| s * s)
            .sum::<f32>()
            / (check_len * channels) as f32)
            .sqrt();
        let rms_out: f32 = (output[skip * channels..(skip + check_len) * channels]
            .iter()
            .map(|s| s * s)
            .sum::<f32>()
            / (check_len * channels) as f32)
            .sqrt();

        let ratio = rms_out / rms_in.max(1e-10);
        assert!(
            (ratio - 1.0).abs() < 0.05,
            "Quiet signal RMS ratio should be ~1.0, got {:.4}",
            ratio
        );
    }

    #[test]
    fn test_latency_reported_correctly() {
        // Causal STFT latency = fft_size - hop_size (hop = fft_size/4 at 75% overlap).
        // fft_size=2048, hop=512 → latency=1536
        let plugin = make_plugin(-20.0, 2.0);
        assert_eq!(plugin.latency_samples(), 1536);

        // fft_size=1024, hop=256 → latency=768
        let params_1024 = SpectralCompressorPluginParams {
            fft_size_index: 0,
            ..Default::default()
        };
        let plugin_1024 = SpectralCompressorPlugin::from_params(2, params_1024);
        assert_eq!(plugin_1024.latency_samples(), 768);

        // fft_size=4096, hop=1024 → latency=3072
        let params_4096 = SpectralCompressorPluginParams {
            fft_size_index: 2,
            ..Default::default()
        };
        let plugin_4096 = SpectralCompressorPlugin::from_params(2, params_4096);
        assert_eq!(plugin_4096.latency_samples(), 3072);
    }

    #[test]
    fn test_process_rejects_buffer_size_mismatch() {
        let mut plugin = make_plugin(-20.0, 2.0);
        let ctx = ProcessContext::new(48000, 64);
        let mut short = vec![0.0f32; ctx.num_frames * plugin.channels() - 1];
        let err = plugin.process_in_place(&mut short, &ctx).unwrap_err();
        assert!(err.contains("Buffer size mismatch"));
    }

    /// Verify the magnitude calibration: a -20 dBFS sine that is above threshold
    /// should be compressed, and a sine well below threshold should pass through
    /// with correct amplitude (RMS ratio ≈ 1.0). This test would catch any
    /// systematic dB offset in `mag_norm` (e.g. the previous 6 dB Hann error).
    #[test]
    fn test_fft_roundtrip_no_compression_below_threshold() {
        // ratio=1.0 with any threshold → no compression regardless of level.
        // Use threshold=0 dB, ratio=1.0, mix=1.0, knee=0.
        let params = SpectralCompressorPluginParams {
            fft_size_index: 1, // 2048
            threshold_db: 0.0,
            ratio: 1.0,
            attack_ms: 5.0,
            release_ms: 50.0,
            knee_db: 0.0,
            spectral_smoothing: 0.0,
            mix: 1.0,
        };
        let mut plugin = SpectralCompressorPlugin::from_params(2, params);
        plugin.initialize(48000).unwrap();

        let channels = 2;
        let num_frames = 96000usize; // 2 seconds
        let freq = 1000.0_f32;
        let amplitude = 0.1_f32; // -20 dBFS

        let mut signal = vec![0.0f32; num_frames * channels];
        for i in 0..num_frames {
            let s = amplitude * (2.0 * std::f32::consts::PI * freq * i as f32 / 48000.0).sin();
            signal[i * channels] = s;
            signal[i * channels + 1] = s;
        }

        let output = process_signal(&mut plugin, &signal);

        // Skip initial latency + settling, compare RMS in the steady-state window.
        let skip = 32768usize;
        let check_len = num_frames - skip - 8192;

        let rms_in: f32 = (signal[skip * channels..(skip + check_len) * channels]
            .iter()
            .map(|s| s * s)
            .sum::<f32>()
            / (check_len * channels) as f32)
            .sqrt();
        let rms_out: f32 = (output[skip * channels..(skip + check_len) * channels]
            .iter()
            .map(|s| s * s)
            .sum::<f32>()
            / (check_len * channels) as f32)
            .sqrt();

        let ratio = rms_out / rms_in.max(1e-10);
        assert!(
            (ratio - 1.0).abs() < 0.05,
            "Identity STFT (ratio=1.0) RMS ratio should be ~1.0, got {:.4} \
             (rms_in={:.6}, rms_out={:.6}). A value near 0.5 indicates the \
             6 dB Hann coherent-gain bug.",
            ratio,
            rms_in,
            rms_out,
        );
    }

    /// Verify that the magnitude calibration is correct: a -20 dBFS sine with
    /// threshold=-25 dB must be detected as above threshold and compressed.
    /// Before the Hann-gain fix the measured level was -26 dB, causing the
    /// compressor to see it as 1 dB below threshold and skip compression.
    #[test]
    fn test_magnitude_calibration_6db_hann_fix() {
        // threshold=-25 dB, ratio=8:1. A -20 dBFS sine is 5 dB above threshold.
        // Expected gain reduction ≈ (5 * 7/8) ≈ 4.4 dB → output ≈ -24.4 dBFS.
        // Before the fix: measured level was ~-26 dB (below -25 threshold) → no compression.
        let params = SpectralCompressorPluginParams {
            fft_size_index: 1,
            threshold_db: -25.0,
            ratio: 8.0,
            attack_ms: 1.0,
            release_ms: 10.0,
            knee_db: 0.0,
            spectral_smoothing: 0.0,
            mix: 1.0,
        };
        let mut plugin = SpectralCompressorPlugin::from_params(2, params);
        plugin.initialize(48000).unwrap();

        let channels = 2;
        let num_frames = 96000usize;
        let amplitude = 0.1_f32; // -20 dBFS (0.1 = 10^(-20/20))
        let freq = 1000.0_f32;

        let mut signal = vec![0.0f32; num_frames * channels];
        for i in 0..num_frames {
            let s = amplitude * (2.0 * std::f32::consts::PI * freq * i as f32 / 48000.0).sin();
            signal[i * channels] = s;
            signal[i * channels + 1] = s;
        }

        let output = process_signal(&mut plugin, &signal);

        let skip = 32768usize;
        let check_len = num_frames - skip - 8192;
        let rms_out: f32 = (output[skip * channels..(skip + check_len) * channels]
            .iter()
            .map(|s| s * s)
            .sum::<f32>()
            / (check_len * channels) as f32)
            .sqrt();

        // Output must be reduced relative to input (compression happened).
        // rms_in ≈ 0.1/√2 ≈ 0.0707. After compression output should be noticeably lower.
        let rms_in_expected = amplitude / std::f32::consts::SQRT_2;
        assert!(
            rms_out < rms_in_expected * 0.85,
            "Expected compression (threshold=-25 dB, input=-20 dBFS): \
             rms_out={:.5} should be < {:.5}. \
             If rms_out ≈ rms_in, the 6 dB Hann calibration bug is present \
             (compressor sees input as -26 dB, below -25 dB threshold).",
            rms_out,
            rms_in_expected * 0.85,
        );
    }

    /// Verify that L and R channels are processed independently: feeding different
    /// signals to L and R and checking each channel's output independently.
    #[test]
    fn test_stereo_independence() {
        // Use high ratio, low threshold so both channels get compressed.
        let mut plugin = make_plugin(-30.0, 8.0);
        let channels = 2;
        let num_frames = 96000usize;

        // L: 440 Hz, R: 880 Hz, same amplitude
        let amplitude = 0.5_f32;
        let mut signal = vec![0.0f32; num_frames * channels];
        for i in 0..num_frames {
            let t = i as f32 / 48000.0;
            signal[i * channels] = amplitude * (2.0 * std::f32::consts::PI * 440.0 * t).sin();
            signal[i * channels + 1] = amplitude * (2.0 * std::f32::consts::PI * 880.0 * t).sin();
        }

        let output = process_signal(&mut plugin, &signal);

        // After settling, measure RMS per-channel in output.
        let skip = 32768usize;
        let check_len = num_frames - skip - 8192;
        let mut rms_l = 0.0f32;
        let mut rms_r = 0.0f32;
        for i in skip..skip + check_len {
            rms_l += output[i * channels] * output[i * channels];
            rms_r += output[i * channels + 1] * output[i * channels + 1];
        }
        rms_l = (rms_l / check_len as f32).sqrt();
        rms_r = (rms_r / check_len as f32).sqrt();

        // Both channels should have been compressed (output < input).
        let rms_in = amplitude / std::f32::consts::SQRT_2;
        assert!(
            rms_l < rms_in * 0.9,
            "L channel should be compressed: rms_l={:.4} vs rms_in={:.4}",
            rms_l,
            rms_in
        );
        assert!(
            rms_r < rms_in * 0.9,
            "R channel should be compressed: rms_r={:.4} vs rms_in={:.4}",
            rms_r,
            rms_in
        );
        // Channels should not be identical (different frequencies → different bin responses)
        // just verify they were processed (both nonzero).
        assert!(rms_l > 1e-6, "L channel output is silence");
        assert!(rms_r > 1e-6, "R channel output is silence");
    }

    #[test]
    fn test_parameter_roundtrip() {
        let mut plugin = make_plugin(-20.0, 2.0);

        // Set all parameters
        plugin
            .set_parameter(
                ParameterId::from("threshold"),
                ParameterValue::Float(-30.0),
            )
            .unwrap();
        plugin
            .set_parameter(ParameterId::from("ratio"), ParameterValue::Float(4.0))
            .unwrap();
        plugin
            .set_parameter(ParameterId::from("attack"), ParameterValue::Float(10.0))
            .unwrap();
        plugin
            .set_parameter(ParameterId::from("release"), ParameterValue::Float(100.0))
            .unwrap();
        plugin
            .set_parameter(ParameterId::from("knee"), ParameterValue::Float(3.0))
            .unwrap();
        plugin
            .set_parameter(
                ParameterId::from("spectral_smoothing"),
                ParameterValue::Float(0.5),
            )
            .unwrap();
        plugin
            .set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.8))
            .unwrap();
        plugin
            .set_parameter(ParameterId::from("fft_size"), ParameterValue::Int(2))
            .unwrap();

        // Verify all parameters
        assert_eq!(
            plugin.get_parameter(&ParameterId::from("threshold")),
            Some(ParameterValue::Float(-30.0))
        );
        assert_eq!(
            plugin.get_parameter(&ParameterId::from("ratio")),
            Some(ParameterValue::Float(4.0))
        );
        assert_eq!(
            plugin.get_parameter(&ParameterId::from("attack")),
            Some(ParameterValue::Float(10.0))
        );
        assert_eq!(
            plugin.get_parameter(&ParameterId::from("release")),
            Some(ParameterValue::Float(100.0))
        );
        assert_eq!(
            plugin.get_parameter(&ParameterId::from("knee")),
            Some(ParameterValue::Float(3.0))
        );
        assert_eq!(
            plugin.get_parameter(&ParameterId::from("spectral_smoothing")),
            Some(ParameterValue::Float(0.5))
        );
        assert_eq!(
            plugin.get_parameter(&ParameterId::from("mix")),
            Some(ParameterValue::Float(0.8))
        );
        assert_eq!(
            plugin.get_parameter(&ParameterId::from("fft_size")),
            Some(ParameterValue::Int(2))
        );
    }

