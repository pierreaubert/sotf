use autoeq::Curve;
use autoeq::cea2034 as score;
use autoeq::cli::PeqModel;
use autoeq::loss;
use ndarray::Array1;
use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

static PEQ_CACHE: OnceLock<Mutex<HashMap<String, Array1<f64>>>> = OnceLock::new();

fn get_peq_cache() -> &'static Mutex<HashMap<String, Array1<f64>>> {
    PEQ_CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

fn compute_peq_cached(
    freq: &Array1<f64>,
    params: &[f64],
    sample_rate: f64,
    model: PeqModel,
) -> Array1<f64> {
    let cache = get_peq_cache();
    let key = format!(
        "{:?}-{:?}-{:?}-{:?}",
        params.len(),
        sample_rate,
        model,
        freq.len()
    );

    {
        let guard = cache.lock().unwrap();
        if let Some(cached) = guard.get(&key) {
            return cached.clone();
        }
    }

    let result = autoeq::x2peq::compute_peq_response_from_x(freq, params, sample_rate, model);

    let mut guard = cache.lock().unwrap();
    // Limit cache size to prevent unbounded growth
    if guard.len() >= 100 {
        guard.clear();
    }
    guard.insert(key, result.clone());

    result
}

/// Post-optimization metrics for CEA2034 or headphone loss
pub(super) struct PostOptMetrics {
    pub(super) cea2034_metrics: Option<score::ScoreMetrics>,
    pub(super) headphone_loss: Option<f64>,
    pub(super) pre_cea2034: Option<score::ScoreMetrics>,
    pub(super) pre_headphone_loss: Option<f64>,
}

/// Compute post-optimization metrics and compare with pre-optimization
#[allow(clippy::too_many_arguments)]
pub(super) async fn compute_post_optimization_metrics(
    args: &autoeq::cli::Args,
    objective_data: &autoeq::optim::ObjectiveData,
    use_cea: bool,
    opt_params: &[f64],
    standard_freq: &ndarray::Array1<f64>,
    target_curve: &Curve,
    input_curve: &Curve,
    spin_data: &Option<HashMap<String, Curve>>,
    pre_cea2034_metrics: Option<score::ScoreMetrics>,
    pre_headphone_loss: Option<f64>,
) -> Result<PostOptMetrics, Box<dyn std::error::Error>> {
    let mut cea2034_metrics: Option<score::ScoreMetrics> = None;
    let mut headphone_loss_val: Option<f64> = None;

    match objective_data.loss_type {
        autoeq::LossType::HeadphoneFlat | autoeq::LossType::HeadphoneScore => {
            let peq_after = compute_peq_cached(
                standard_freq,
                opt_params,
                args.sample_rate,
                args.effective_peq_model(),
            );
            // Compute remaining deviation from target after applying PEQ
            // Use same convention as deviation_curve: target - corrected
            // deviation_after = target - (input + peq)
            let deviation_after = Curve {
                freq: standard_freq.clone(),
                spl: &target_curve.spl - &input_curve.spl - &peq_after,
                phase: None,
            };
            headphone_loss_val = Some(loss::headphone_loss(&deviation_after));
        }
        autoeq::LossType::SpeakerFlat | autoeq::LossType::SpeakerScore => {
            if use_cea {
                let freq = &objective_data.freqs;
                let peq_after = compute_peq_cached(
                    freq,
                    opt_params,
                    args.sample_rate,
                    args.effective_peq_model(),
                );
                let metrics = score::compute_cea2034_metrics(
                    freq,
                    spin_data.as_ref().unwrap(),
                    Some(&peq_after),
                )
                .await?;
                cea2034_metrics = Some(metrics);
            }
        }
        autoeq::LossType::DriversFlat | autoeq::LossType::MultiSubFlat => {
            // Unreachable: DriversFlat mode uses a separate code path
            unreachable!("DriversFlat mode should not reach this point");
        }
    }

    Ok(PostOptMetrics {
        cea2034_metrics,
        headphone_loss: headphone_loss_val,
        pre_cea2034: pre_cea2034_metrics,
        pre_headphone_loss,
    })
}

/// Print pre and post optimization scores
pub(super) fn print_optimization_scores(args: &autoeq::cli::Args, post: &PostOptMetrics) {
    match args.loss {
        autoeq::LossType::HeadphoneFlat | autoeq::LossType::HeadphoneScore => {
            if let Some(before) = post.pre_headphone_loss {
                autoeq::qa_println!(args, "✅  Pre-Optimization Headphone Score: {:.3}", before);
            }
            if let Some(after) = post.headphone_loss {
                autoeq::qa_println!(args, "✅ Post-Optimization Headphone Score: {:.3}", after);
            }
        }
        autoeq::LossType::SpeakerFlat | autoeq::LossType::SpeakerScore => {
            if let Some(before) = &post.pre_cea2034 {
                autoeq::qa_println!(
                    args,
                    "✅  Pre-Optimization CEA2034 Score: pref={:.3} | nbd_on={:.3} nbd_pir={:.3} lfx={:.0}Hz sm_pir={:.3}",
                    before.pref_score,
                    before.nbd_on,
                    before.nbd_pir,
                    10f64.powf(before.lfx),
                    before.sm_pir
                );
            }
            if let Some(after) = &post.cea2034_metrics {
                autoeq::qa_println!(
                    args,
                    "✅ Post-Optimization CEA2034 Score: pref={:.3} | nbd_on={:.3} nbd_pir={:.3} lfx={:.0}hz sm_pir={:.3}",
                    after.pref_score,
                    after.nbd_on,
                    after.nbd_pir,
                    10f64.powf(after.lfx),
                    after.sm_pir
                );
            }
        }
        autoeq::LossType::DriversFlat | autoeq::LossType::MultiSubFlat => {
            // Unreachable: DriversFlat mode uses a separate code path
            unreachable!("DriversFlat mode should not reach this point");
        }
    }
}
