#[cfg(test)]
mod tests {
    use crate::save::save_peq_to_file;
    use autoeq::cli::Args;
    use autoeq::loss::LossType;
    use clap::Parser;
    use std::fs;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_save_peq_to_file_apo_format() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("test_output");

        let args = Args::parse_from(["autoeq-test", "--loss", "speaker-flat"]);

        // Example optimized parameters (3 filters)
        // Note: Frequency parameters must be in log10 scale
        let x = vec![
            500.0f64.log10(), 2.0, -3.0, // Filter 1
            1000.0f64.log10(), 5.0, 2.0, // Filter 2
            3000.0f64.log10(), 3.0, -1.0, // Filter 3
        ];

        let result = save_peq_to_file(&args, &x, &output_path, &LossType::SpeakerFlat).await;

        assert!(result.is_ok());

        // Verify file was created
        let apo_path = output_path.parent().unwrap().join("iir-autoeq-flat.txt");
        assert!(apo_path.exists());

        // Verify content
        let content = fs::read_to_string(&apo_path).unwrap();
        assert!(content.contains("AutoEQ"));
        assert!(content.contains("Filter")); // Check for presence of filter lines
    }

    #[tokio::test]
    async fn test_save_peq_to_file_score_loss() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("test_output");

        let args = Args::parse_from(["autoeq-test", "--loss", "speaker-score"]);

        let x = vec![500.0f64.log10(), 2.0, -2.0];

        let result = save_peq_to_file(&args, &x, &output_path, &LossType::SpeakerScore).await;

        assert!(result.is_ok());

        // Verify filename for score loss
        let apo_path = output_path.parent().unwrap().join("iir-autoeq-score.txt");
        assert!(apo_path.exists());
    }

    #[tokio::test]
    async fn test_save_peq_creates_multiple_formats() {
        let temp_dir = TempDir::new().unwrap();
        let output_path = temp_dir.path().join("test_output");

        let args = Args::parse_from(["autoeq-test", "--loss", "speaker-flat"]);

        let x = vec![500.0f64.log10(), 2.0, -2.0];

        let _ = save_peq_to_file(&args, &x, &output_path, &LossType::SpeakerFlat).await;

        // Check APO format
        assert!(
            output_path
                .parent()
                .unwrap()
                .join("iir-autoeq-flat.txt")
                .exists()
        );

        // Check RME format
        assert!(
            output_path
                .parent()
                .unwrap()
                .join("iir-autoeq-flat.tmreq")
                .exists()
        );

        // Check Apple format
        assert!(
            output_path
                .parent()
                .unwrap()
                .join("iir-autoeq-flat.aupreset")
                .exists()
        );
    }
}
