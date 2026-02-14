#[cfg(test)]
mod tests {
    use crate::spacing::{check_spacing_constraints, print_freq_spacing};
    use autoeq::cli::Args;
    use clap::Parser;

    fn create_test_args() -> Args {
        Args::parse_from(["autoeq-test", "--num-filters", "5", "--min-spacing-oct", "0.2"])
    }

    #[test]
    fn test_check_spacing_constraints_valid() {
        let args = create_test_args();

        // Valid spacing: frequencies are well separated
        // Format: freq_log, Q, gain (repeated for each filter)
        let x = vec![
            3.0, 1.0, -3.0, // ~1000 Hz
            4.0, 2.0, 2.0, // ~10000 Hz
            2.0, 1.5, -1.0, // ~100 Hz
            5.0, 3.0, 1.0, // ~100000 Hz (out of range but valid)
            3.5, 1.0, 0.5, // ~3162 Hz
        ];

        let result = check_spacing_constraints(&x, &args);
        assert!(result);
    }

    #[test]
    fn test_check_spacing_constraints_too_close() {
        let args = create_test_args();

        // Frequencies too close together
        let x = vec![
            3.0, 1.0, -3.0, // ~1000 Hz
            3.05, 1.0, 2.0, // ~1122 Hz (only 0.16 octaves apart, < 0.2)
            3.1, 1.5, -1.0, // ~1259 Hz
            3.2, 3.0, 1.0, 3.3, 1.0, 0.5,
        ];

        let result = check_spacing_constraints(&x, &args);
        assert!(!result);
    }

    #[test]
    fn test_check_spacing_constraints_empty() {
        let args = create_test_args();

        // Empty params (no filters)
        let x: Vec<f64> = vec![];

        let result = check_spacing_constraints(&x, &args);
        assert!(!result); // No filters = no spacing
    }

    #[test]
    fn test_print_freq_spacing() {
        let args = create_test_args();

        let x = vec![
            2.5, 1.0, -2.0, // ~316 Hz
            3.5, 2.0, 1.5, // ~3162 Hz
            4.0, 1.5, -1.0, // ~10000 Hz
        ];

        // Should not panic
        print_freq_spacing(&x, &args, "test");
    }
}
