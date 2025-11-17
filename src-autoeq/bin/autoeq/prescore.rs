use autoeq::Curve;
use autoeq::cea2034 as score;
use autoeq::loss;
use std::collections::HashMap;

/// Pre-optimization metrics for CEA2034 or headphone loss
pub(super) struct PreOptMetrics {
    pub(super) cea2034_metrics: Option<score::ScoreMetrics>,
    pub(super) headphone_loss: Option<f64>,
}

/// Compute pre-optimization metrics
pub(super) async fn compute_pre_optimization_metrics(
    _args: &autoeq::cli::Args,
    objective_data: &autoeq::optim::ObjectiveData,
    use_cea: bool,
    deviation_curve: &Curve,
    spin_data: &Option<HashMap<String, Curve>>,
) -> Result<PreOptMetrics, Box<dyn std::error::Error>> {
    let mut cea2034_metrics: Option<score::ScoreMetrics> = None;
    let mut headphone_loss_val: Option<f64> = None;

    match objective_data.loss_type {
        autoeq::LossType::HeadphoneFlat | autoeq::LossType::HeadphoneScore => {
            // headphone_loss expects deviation from Harman target, not raw curve
            headphone_loss_val = Some(loss::headphone_loss(deviation_curve));
        }
        autoeq::LossType::SpeakerFlat | autoeq::LossType::SpeakerScore => {
            if use_cea {
                let metrics = score::compute_cea2034_metrics(
                    &objective_data.freqs,
                    spin_data.as_ref().unwrap(),
                    None,
                )
                .await?;
                cea2034_metrics = Some(metrics);
            }
        }
        autoeq::LossType::DriversFlat => {
            // Unreachable: DriversFlat mode uses a separate code path
            unreachable!("DriversFlat mode should not reach this point");
        }
    }

    Ok(PreOptMetrics {
        cea2034_metrics,
        headphone_loss: headphone_loss_val,
    })
}
