#[cfg(test)]
mod tests {
    use crate::prescore::compute_pre_optimization_metrics;
    use autoeq::Curve;
    use autoeq::cli::Args;
    use autoeq::cli::PeqModel;
    use autoeq::loss::LossType;
    use autoeq::optim::ObjectiveData;
    use clap::Parser;
    use ndarray::Array1;

    fn create_test_objective_data(loss_type: LossType) -> ObjectiveData {
        let freqs = Array1::from_vec(vec![100.0, 500.0, 1000.0, 5000.0, 10000.0]);
        let deviation = Array1::from_vec(vec![2.0, 1.5, 1.0, 1.2, 0.8]);
        let target = Array1::zeros(freqs.len());

        ObjectiveData {
            freqs,
            target,
            deviation,
            srate: 48000.0,
            min_spacing_oct: 0.1,
            spacing_weight: 0.0,
            max_db: 10.0,
            min_db: -10.0,
            min_freq: 20.0,
            max_freq: 20000.0,
            peq_model: PeqModel::Pk,
            loss_type,
            speaker_score_data: None,
            headphone_score_data: None,
            input_curve: None,
            drivers_data: None,
            fixed_crossover_freqs: None,
            penalty_w_ceiling: 0.0,
            penalty_w_spacing: 0.0,
            penalty_w_mingain: 0.0,
            integrality: None,
        }
    }

    #[tokio::test]
    async fn test_compute_pre_optimization_headphone() {
        let args = Args::try_parse_from(["autoeq-test", "--loss", "headphone-flat"]).unwrap();
        let objective_data = create_test_objective_data(LossType::HeadphoneFlat);

        let deviation = Curve {
            freq: objective_data.freqs.clone(),
            spl: objective_data.deviation.clone(),
            phase: None,
        };

        let result =
            compute_pre_optimization_metrics(&args, &objective_data, false, &deviation, &None)
                .await;

        assert!(result.is_ok());
        let metrics = result.unwrap();

        // Headphone loss should be computed
        assert!(metrics.headphone_loss.is_some());
        // CEA metrics should be None
        assert!(metrics.cea2034_metrics.is_none());
    }

    #[tokio::test]
    async fn test_compute_pre_optimization_speaker_flat() {
        let args = Args::try_parse_from(["autoeq-test", "--loss", "speaker-flat"]).unwrap();
        let objective_data = create_test_objective_data(LossType::SpeakerFlat);

        let deviation = Curve {
            freq: objective_data.freqs.clone(),
            spl: objective_data.deviation.clone(),
            phase: None,
        };

        let result = compute_pre_optimization_metrics(
            &args,
            &objective_data,
            false, // no CEA data
            &deviation,
            &None,
        )
        .await;

        assert!(result.is_ok());
        let metrics = result.unwrap();

        // Both should be None without CEA data
        assert!(metrics.headphone_loss.is_none());
        assert!(metrics.cea2034_metrics.is_none());
    }
}
