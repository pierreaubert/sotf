#[cfg(test)]
mod tests {
    use crate::postscore::{PostOptMetrics, compute_post_optimization_metrics};
    use autoeq::Curve;
    use autoeq::cli::Args;
    use autoeq::cli::PeqModel;
    use autoeq::loss::LossType;
    use clap::Parser;
    use ndarray::Array1;

    #[tokio::test]
    async fn test_compute_post_optimization_headphone() {
        let args = Args::try_parse_from([
            "autoeq-test",
            "--loss",
            "headphone-flat",
            "--sample-rate",
            "48000",
        ])
        .unwrap();

        let freqs = Array1::from_vec(vec![100.0, 500.0, 1000.0, 5000.0, 10000.0]);
        let target_spl = Array1::from_vec(vec![0.0; 5]);
        let input_spl = Array1::from_vec(vec![2.0, 1.5, 1.0, 1.2, 0.8]);

        let target_curve = Curve {
            freq: freqs.clone(),
            spl: target_spl.clone(),
            phase: None,
        };

        let input_curve = Curve {
            freq: freqs.clone(),
            spl: input_spl.clone(),
            phase: None,
        };

        let objective_data = autoeq::optim::ObjectiveData {
            freqs: freqs.clone(),
            deviation: &input_spl - &target_spl,
            target: target_spl.clone(),
            srate: 48000.0,
            min_spacing_oct: 0.1,
            spacing_weight: 0.0,
            max_db: 10.0,
            min_db: -10.0,
            min_freq: 20.0,
            max_freq: 20000.0,
            peq_model: PeqModel::Pk,
            loss_type: LossType::HeadphoneFlat,
            speaker_score_data: None,
            headphone_score_data: None,
            input_curve: None,
            drivers_data: None,
            fixed_crossover_freqs: None,
            penalty_w_ceiling: 0.0,
            penalty_w_spacing: 0.0,
            penalty_w_mingain: 0.0,
            integrality: None,
        };

        let opt_params = vec![500.0, 2.0, -2.0]; // Example PEQ params

        let result = compute_post_optimization_metrics(
            &args,
            &objective_data,
            false,
            &opt_params,
            &freqs,
            &target_curve,
            &input_curve,
            &None,
            None,
            None,
        )
        .await;

        assert!(result.is_ok());
        let metrics = result.unwrap();
        assert!(metrics.headphone_loss.is_some());
    }

    #[test]
    fn test_print_optimization_scores_headphone() {
        let args = Args::try_parse_from(["autoeq-test", "--loss", "headphone-flat"]).unwrap();

        let metrics = PostOptMetrics {
            cea2034_metrics: None,
            headphone_loss: Some(5.5),
            pre_cea2034: None,
            pre_headphone_loss: Some(8.2),
        };

        // Should not panic
        crate::postscore::print_optimization_scores(&args, &metrics);
    }
}
