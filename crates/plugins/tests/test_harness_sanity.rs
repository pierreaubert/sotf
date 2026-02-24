#[cfg(test)]
mod tests {
    use sotf_plugins::test_utils::{SignalGen, BufferComparison};

    #[test]
    fn test_sine_generator() {
        let sample_rate = 44100.0;
        let frequency = 1000.0;
        let amplitude = 1.0;
        let num_samples = 100;
        
        let mut signal_gen = SignalGen::new_sine(sample_rate, frequency, amplitude);
        let buffer = signal_gen.generate(num_samples);
        
        assert_eq!(buffer.len(), num_samples);
        // First sample of sine(0) should be 0.0
        assert!((buffer[0] - 0.0).abs() < 1e-6);
        // Frequency check: sine(2*pi*f/fs)
        let expected_1 = (2.0 * std::f64::consts::PI * frequency / sample_rate).sin() as f32;
        assert!((buffer[1] - expected_1).abs() < 1e-6);
    }

    #[test]
    fn test_white_noise_generator() {
        let mut signal_gen = SignalGen::new_white_noise(1.0);
        let buffer = signal_gen.generate(1000);
        
        let mut mean = 0.0;
        for &s in &buffer {
            mean += s;
            assert!(s >= -1.0 && s <= 1.0);
        }
        mean /= buffer.len() as f32;
        // Mean of white noise should be roughly 0
        assert!(mean.abs() < 0.1);

        // Check determinism
        let mut gen2 = SignalGen::new_white_noise(1.0);
        let buf2 = gen2.generate(1000);
        assert_eq!(buffer, buf2);
    }

    #[test]
    fn test_pink_noise_generator() {
        let mut signal_gen = SignalGen::new_pink_noise(1.0);
        let buffer = signal_gen.generate(1000);
        assert_eq!(buffer.len(), 1000);
        assert!(buffer.iter().any(|&s| s != 0.0));

        // Check determinism
        let mut gen2 = SignalGen::new_pink_noise(1.0);
        let buf2 = gen2.generate(1000);
        assert_eq!(buffer, buf2);
    }

    #[test]
    fn test_log_sweep_generator() {
        let mut signal_gen = SignalGen::new_log_sweep(48000.0, 20.0, 20000.0, 1.0, 1.0);
        let buffer = signal_gen.generate(48000);
        assert_eq!(buffer.len(), 48000);
        assert!(buffer.iter().any(|&s| s != 0.0));
        
        // Check that it stops after duration
        let tail = signal_gen.generate(100);
        assert!(tail.iter().all(|&s| s == 0.0));
    }

    #[test]
    fn test_impulse_generator() {
        let mut signal_gen = SignalGen::new_impulse();
        let buffer = signal_gen.generate(10);
        
        assert!((buffer[0] - 1.0).abs() < 1e-5);
        for i in 1..10 {
            assert_eq!(buffer[i], 0.0);
        }
    }

    #[test]
    fn test_step_generator() {
        let mut signal_gen = SignalGen::new_step();
        let buffer = signal_gen.generate(10);
        
        for i in 0..10 {
            assert!((buffer[i] - 1.0).abs() < 1e-5);
        }
    }

    #[test]
    fn test_buffer_comparison_rms() {
        let buf1 = vec![0.5; 100];
        let buf2 = vec![0.5; 100];
        let buf3 = vec![0.6; 100];
        
        assert!(BufferComparison::compare_rms(&buf1, &buf2, 1e-6));
        assert!(!BufferComparison::compare_rms(&buf1, &buf3, 1e-6));
        
        let buf4 = vec![0.0, 1.0];
        let buf5 = vec![0.0, 1.1]; // diff 0.1, sq diff 0.01, rms sqrt(0.01/2) = 0.0707
        assert!(BufferComparison::compare_rms(&buf4, &buf5, 0.08));
        assert!(!BufferComparison::compare_rms(&buf4, &buf5, 0.07));
    }

    #[test]
    fn test_buffer_comparison_bit_accurate() {
        let buf1 = vec![0.1, 0.2, 0.3];
        let buf2 = vec![0.1, 0.2, 0.3];
        let buf3 = vec![0.1, 0.2, 0.3000001];
        
        assert!(BufferComparison::compare_bit_accurate(&buf1, &buf2));
        assert!(!BufferComparison::compare_bit_accurate(&buf1, &buf3));
    }
}
