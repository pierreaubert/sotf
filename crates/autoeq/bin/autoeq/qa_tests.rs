#[cfg(test)]
mod tests {
    use crate::qa::{QaAnalysisResult, display_qa_analysis, perform_qa_analysis};

    #[test]
    fn test_perform_qa_analysis_all_pass() {
        let result = perform_qa_analysis(
            true,      // converged
            true,      // spacing_ok
            Some(5.0), // pre_score
            Some(6.0), // post_score (improved)
            0.5,       // threshold
        );

        assert!(result.converge_ok);
        assert!(result.spacing_ok);
        assert!(result.improvement_ok);
    }

    #[test]
    fn test_perform_qa_analysis_no_improvement() {
        let result = perform_qa_analysis(
            true,
            true,
            Some(5.0),
            Some(5.2), // Not enough improvement
            0.5,
        );

        assert!(result.converge_ok);
        assert!(result.spacing_ok);
        assert!(!result.improvement_ok);
    }

    #[test]
    fn test_perform_qa_analysis_with_nan() {
        let result = perform_qa_analysis(
            true,
            true,
            None, // pre_score is NaN
            Some(4.0),
            0.5,
        );

        // Should handle NaN gracefully
        assert!(result.pre_value.is_nan());
        assert!(!result.improvement_ok); // Won't pass with NaN
    }

    #[test]
    fn test_display_qa_analysis() {
        let result = QaAnalysisResult {
            converge_ok: true,
            spacing_ok: false,
            improvement_ok: true,
            improvement_threshold: 4.5,
            pre_value: 5.0,
            post_value: 4.0,
        };

        // Should not panic
        display_qa_analysis(&result);
    }
}
