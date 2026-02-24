#[cfg(test)]
mod tests {
    use sotf_plugins::test_utils::{SignalGenerator, BufferComparison};

    #[test]
    fn test_sine_generator() {
        let sample_rate = 44100.0;
        let frequency = 1000.0;
        let amplitude = 1.0;
        let num_samples = 100;
        
        let mut signal_gen = SignalGenerator::new_sine(sample_rate, frequency, amplitude);
        let buffer = signal_gen.generate(num_samples);
        
        assert_eq!(buffer.len(), num_samples);
        // First sample of sine(0) should be 0.0
        assert!((buffer[0] - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_buffer_comparison_rms() {
        let buf1 = vec![0.5; 100];
        let buf2 = vec![0.5; 100];
        let buf3 = vec![0.6; 100];
        
        assert!(BufferComparison::compare_rms(&buf1, &buf2, 1e-6));
        assert!(!BufferComparison::compare_rms(&buf1, &buf3, 1e-6));
    }
}
