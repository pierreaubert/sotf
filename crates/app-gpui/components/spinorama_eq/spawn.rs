use super::misc::spinorama_runtime;
use gpui::*;

/// Spawn a background thread that loads CEA2034 spinorama curves for the
/// plot, returning a oneshot receiver. If the receiver is dropped (wizard
/// closed) the producer's `send_blocking` silently fails — no leak.
pub(super) fn spawn_spinorama_curves_thread(
    speaker: String,
    version: String,
) -> smol::channel::Receiver<Result<crate::app::types::SpinoramaCurves, String>> {
    let (tx, rx) = smol::channel::bounded::<Result<crate::app::types::SpinoramaCurves, String>>(1);
    std::thread::spawn(move || {
        log::info!(
            "Loading spinorama CEA2034 curves for {} / {}",
            speaker,
            version
        );
        let result = spinorama_runtime().block_on(async {
            // Fetch CEA2034 measurement data
            let plot_data =
                autoeq::read::fetch_measurement_plot_data(&speaker, &version, "CEA2034")
                    .await
                    .map_err(|e| format!("API error: {}", e))?;

            // Extract curves using original frequency grid
            let curves = autoeq::read::extract_cea2034_curves_original(&plot_data, "CEA2034")
                .map_err(|e| format!("Extraction error: {}", e))?;

            // Convert to our SpinoramaCurves format
            let on_axis = curves.get("On Axis").ok_or("On Axis curve not found")?;
            let frequencies: Vec<f64> = on_axis.freq.to_vec();

            // Get PIR (Estimated In-Room Response)
            let estimated_in_room = curves
                .get("Estimated In-Room Response")
                .map(|c| c.spl.to_vec())
                .unwrap_or_else(|| vec![0.0; frequencies.len()]);

            // Try to fetch directivity data (SPL Horizontal and SPL Vertical)
            let directivity = autoeq::read::fetch_directivity_data(&speaker, &version)
                .await
                .ok();

            let (horizontal_directivity, vertical_directivity) = if let Some(dir) = directivity {
                let horizontal: Vec<crate::app::types::DirectivityCurve> = dir
                    .horizontal
                    .iter()
                    .map(|c| crate::app::types::DirectivityCurve {
                        angle: c.angle,
                        frequencies: c.freq.to_vec(),
                        spl: c.spl.to_vec(),
                    })
                    .collect();
                let vertical: Vec<crate::app::types::DirectivityCurve> = dir
                    .vertical
                    .iter()
                    .map(|c| crate::app::types::DirectivityCurve {
                        angle: c.angle,
                        frequencies: c.freq.to_vec(),
                        spl: c.spl.to_vec(),
                    })
                    .collect();
                (horizontal, vertical)
            } else {
                log::warn!(
                    "Directivity data not available for {} / {}",
                    speaker,
                    version
                );
                (Vec::new(), Vec::new())
            };

            let spinorama_curves = crate::app::types::SpinoramaCurves {
                frequencies: frequencies.clone(),
                on_axis: on_axis.spl.to_vec(),
                listening_window: curves
                    .get("Listening Window")
                    .map(|c| c.spl.to_vec())
                    .unwrap_or_else(|| vec![0.0; frequencies.len()]),
                early_reflections: curves
                    .get("Early Reflections")
                    .map(|c| c.spl.to_vec())
                    .unwrap_or_else(|| vec![0.0; frequencies.len()]),
                sound_power: curves
                    .get("Sound Power")
                    .map(|c| c.spl.to_vec())
                    .unwrap_or_else(|| vec![0.0; frequencies.len()]),
                early_reflections_di: curves
                    .get("Early Reflections DI")
                    .map(|c| c.spl.to_vec())
                    .unwrap_or_else(|| vec![0.0; frequencies.len()]),
                sound_power_di: curves
                    .get("Sound Power DI")
                    .map(|c| c.spl.to_vec())
                    .unwrap_or_else(|| vec![0.0; frequencies.len()]),
                estimated_in_room,
                horizontal_directivity,
                vertical_directivity,
            };

            Ok::<crate::app::types::SpinoramaCurves, String>(spinorama_curves)
        });

        match &result {
            Ok(curves) => {
                log::info!(
                    "Spinorama curves loaded: {} frequencies, {} horizontal, {} vertical",
                    curves.frequencies.len(),
                    curves.horizontal_directivity.len(),
                    curves.vertical_directivity.len()
                );
            }
            Err(e) => {
                log::error!("Failed to load spinorama curves: {}", e);
            }
        }
        let _ = tx.send_blocking(result);
    });
    rx
}

/// Spawn a background thread to check phase data availability for a
/// speaker/version/measurement, returning a oneshot receiver that yields
/// the `has_phase` boolean once the check completes.
pub(super) fn spawn_phase_data_check_thread(
    speaker: String,
    version: String,
    measurement: String,
) -> smol::channel::Receiver<bool> {
    let (tx, rx) = smol::channel::bounded::<bool>(1);
    let curve_name = "Estimated In-Room Response".to_string();

    std::thread::spawn(move || {
        let has_phase = spinorama_runtime().block_on(async {
            match autoeq::read::read_spinorama(&speaker, &version, &measurement, &curve_name).await
            {
                Ok(curve) => curve.phase.is_some(),
                Err(e) => {
                    log::warn!("Failed to fetch curve for phase check: {}", e);
                    false
                }
            }
        });
        let _ = tx.send_blocking(has_phase);
    });
    rx
}
