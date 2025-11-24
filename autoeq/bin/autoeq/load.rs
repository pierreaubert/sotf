use autoeq::Curve;
use autoeq::loss::{CrossoverType, DriverMeasurement, DriversLossData};
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

    // Normalize and interpolate input curve
    let input_curve = read::normalize_and_interpolate_response(&standard_freq, &input_curve_raw);

    // Build/Get target and interpolate it
    let target_curve_raw =
        autoeq::workflow::build_target_curve(args, &standard_freq, &input_curve_raw);
    let target_curve = read::interpolate_log_space(&standard_freq, &target_curve_raw);

    // Compute and interpolate deviation curve
    let deviation_curve_raw = Curve {
        freq: target_curve.freq.clone(),
        spl: target_curve.spl.clone() - &input_curve.spl,
    };
    let deviation_curve = read::interpolate_log_space(&standard_freq, &deviation_curve_raw);

    // Interpolate spinorama data if available
    let spin_data = spin_data_raw.map(|spin_data| {
        spin_data
            .into_iter()
            .map(|(name, curve)| {
                let interpolated = read::interpolate_log_space(&standard_freq, &curve);
                (name, interpolated)
            })
            .collect()
    });

    Ok((
        standard_freq,
        input_curve,
        target_curve,
        deviation_curve,
        spin_data,
    ))
}

/// Load multi-driver measurement data for crossover optimization
///
/// # Arguments
/// * `args` - CLI arguments containing driver file paths and crossover type
///
/// # Returns
/// * DriversLossData containing all driver measurements and configuration
pub(super) async fn load_drivers_data(
    args: &autoeq::cli::Args,
) -> Result<DriversLossData, Box<dyn std::error::Error>> {
    // Collect driver file paths
    let driver_paths: Vec<_> = [&args.driver1, &args.driver2, &args.driver3, &args.driver4]
        .iter()
        .filter_map(|p| p.as_ref())
        .collect();

    if driver_paths.len() < 2 {
        return Err("At least 2 driver files are required for multi-driver optimization".into());
    }

    // Load driver measurements
    let mut drivers = Vec::new();
    for (i, path) in driver_paths.iter().enumerate() {
        match read::load_driver_measurement(path) {
            Ok((freq, spl, phase)) => {
                drivers.push(DriverMeasurement::new(freq, spl, phase));
                eprintln!("✓ Loaded driver {} from {}", i + 1, path.display());
            }
            Err(e) => {
                return Err(format!(
                    "Failed to load driver {} from {}: {}",
                    i + 1,
                    path.display(),
                    e
                )
                .into());
            }
        }
    }

    // Parse crossover type
    let crossover_type = match args.crossover_type.as_str() {
        "butterworth2" => CrossoverType::Butterworth2,
        "linkwitzriley2" => CrossoverType::LinkwitzRiley2,
        "linkwitzriley4" => CrossoverType::LinkwitzRiley4,
        other => {
            return Err(format!(
                "Unknown crossover type '{}'. Valid types: butterworth2, linkwitzriley2, linkwitzriley4",
                other
            )
            .into());
        }
    };

    // Create DriversLossData (this will sort drivers and create freq grid)
    let drivers_data = DriversLossData::new(drivers, crossover_type);

    eprintln!(
        "✓ Initialized {} drivers with {:?} crossover",
        drivers_data.drivers.len(),
        drivers_data.crossover_type
    );

    Ok(drivers_data)
}
