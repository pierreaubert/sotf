use autoeq::Curve;
use autoeq::read;
use std::collections::HashMap;

/// Load input data and prepare frequency grid and curves
pub(super) async fn load_and_prepare(
    args: &autoeq::cli::Args,
) -> Result<
    (
        ndarray::Array1<f64>,
        Curve,
        Curve,
        Curve,
        Option<HashMap<String, Curve>>,
    ),
    Box<dyn std::error::Error>,
> {
    // Load input data
    let (input_curve_raw, spin_data_raw) = autoeq::workflow::load_input_curve(args).await?;

    // Determine if this is headphone or speaker optimization
    let is_headphone = matches!(
        args.loss,
        autoeq::LossType::HeadphoneFlat | autoeq::LossType::HeadphoneScore
    );

    // Determine if we can use the original frequency grid from CEA2034 data
    // to avoid unnecessary resampling while maintaining accuracy
    let use_original_freq = if let Some(ref spin_data) = spin_data_raw {
        // Check if all curves have the same frequency grid as input_curve_raw
        let input_freq = &input_curve_raw.freq;
        spin_data.iter().all(|(_, curve)| {
            curve.freq.len() == input_freq.len()
                && curve
                    .freq
                    .iter()
                    .zip(input_freq.iter())
                    .all(|(a, b)| (a - b).abs() < 1e-9) // Allow tiny numerical differences
        })
    } else {
        false
    };

    // Use original frequency grid from API data if available and consistent,
    // otherwise create a standard log-spaced grid
    let standard_freq = if use_original_freq {
        input_curve_raw.freq.clone()
    } else {
        let num_points = if is_headphone { 120 } else { 200 };
        read::create_log_frequency_grid(num_points, 20.0, 20000.0)
    };

    // Interpolate input curve first (needed for deviation calculation)
    let input_curve = read::normalize_and_interpolate_response(&standard_freq, &input_curve_raw);

    // Build target curve in parallel with spinorama interpolation
    let (target_res, spin_data_res) = tokio::join!(
        async {
            let target_raw =
                autoeq::workflow::build_target_curve(args, &standard_freq, &input_curve_raw)?;
            Ok::<_, Box<dyn std::error::Error>>(read::interpolate_log_space(
                &standard_freq,
                &target_raw,
            ))
        },
        async {
            // Interpolate spinorama data if available
            match spin_data_raw {
                Some(spin_data) => {
                    let interpolated: HashMap<String, Curve> = spin_data
                        .into_iter()
                        .map(|(name, curve)| {
                            let interp = read::interpolate_log_space(&standard_freq, &curve);
                            (name, interp)
                        })
                        .collect();
                    Ok::<_, Box<dyn std::error::Error>>(Some(interpolated))
                }
                None => Ok::<_, Box<dyn std::error::Error>>(None),
            }
        }
    );

    // Unpack results and compute deviation
    let target_curve = target_res?;
    let deviation_curve = Curve {
        freq: standard_freq.clone(),
        spl: &target_curve.spl - &input_curve.spl,
        phase: None,
    };
    let spin_data = spin_data_res?;

    Ok((
        standard_freq,
        input_curve,
        target_curve,
        deviation_curve,
        spin_data,
    ))
}
