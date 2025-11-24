use plotly::common::Mode;
use plotly::layout::{Axis, AxisType};
use plotly::{Layout, Plot, Scatter};

use crate::loss::{DriversLossData, compute_drivers_combined_response};

/// Create a plot showing individual driver responses and the combined response
///
/// # Arguments
/// * `drivers_data` - Multi-driver measurement data
/// * `gains` - Optimized gain values for each driver (in dB)
/// * `crossover_freqs` - Optimized crossover frequencies (in Hz)
/// * `sample_rate` - Sample rate for filter design
///
/// # Returns
/// * Plot object showing all drivers and their combined response
pub fn plot_drivers(
    drivers_data: &DriversLossData,
    gains: &[f64],
    crossover_freqs: &[f64],
    sample_rate: f64,
) -> Plot {
    let mut plot = Plot::new();

    let freq_grid = &drivers_data.freq_grid;

    // First, compute the combined response to get a reference normalization
    let combined_response =
        compute_drivers_combined_response(drivers_data, gains, crossover_freqs, sample_rate);
    let combined_mean = combined_response.mean().unwrap_or(0.0);

    // Plot individual drivers (raw responses)
    for (i, driver) in drivers_data.drivers.iter().enumerate() {
        // Interpolate driver response to common frequency grid
        let interpolated = crate::read::normalize_and_interpolate_response(
            freq_grid,
            &crate::Curve {
                freq: driver.freq.clone(),
                spl: driver.spl.clone(),
            },
        );

        let color = match i {
            0 => "rgb(31, 119, 180)",  // Blue (woofer)
            1 => "rgb(255, 127, 14)",  // Orange (tweeter)
            2 => "rgb(44, 160, 44)",   // Green (midrange)
            3 => "rgb(214, 39, 40)",   // Red (super tweeter)
            _ => "rgb(128, 128, 128)", // Gray (fallback)
        };

        let trace = Scatter::new(freq_grid.to_vec(), interpolated.spl.to_vec())
            .mode(Mode::Lines)
            .name(format!("Driver {} (raw)", i + 1))
            .line(
                plotly::common::Line::new()
                    .color(color)
                    .width(1.5)
                    .dash(plotly::common::DashType::Dash),
            );

        plot.add_trace(trace);
    }

    // Plot individual drivers with gains and crossovers applied
    // We need to compute the response for each driver on THEIR frequency range only
    for (i, driver) in drivers_data.drivers.iter().enumerate() {
        // Use the driver's own frequency range
        let driver_freq_grid = &driver.freq;

        // Normalize the driver's SPL first (subtract mean to center around 0)
        let driver_spl_mean = driver.spl.mean().unwrap_or(0.0);
        let driver_spl_normalized = &driver.spl - driver_spl_mean;

        // Start with the normalized driver's SPL + gain
        let mut response = &driver_spl_normalized + gains[i];

        // Apply crossover filters on the driver's frequency grid
        if i > 0 {
            // Apply highpass from crossover with previous driver
            let xover_freq = crossover_freqs[i - 1];
            let hp_filter = match drivers_data.crossover_type {
                crate::loss::CrossoverType::Butterworth2 => {
                    crate::iir::peq_butterworth_highpass(2, xover_freq, sample_rate)
                }
                crate::loss::CrossoverType::LinkwitzRiley2 => {
                    crate::iir::peq_linkwitzriley_highpass(2, xover_freq, sample_rate)
                }
                crate::loss::CrossoverType::LinkwitzRiley4 => {
                    crate::iir::peq_linkwitzriley_highpass(4, xover_freq, sample_rate)
                }
            };
            let hp_response =
                crate::iir::compute_peq_response(driver_freq_grid, &hp_filter, sample_rate);
            response = response + hp_response;
        }

        if i < drivers_data.drivers.len() - 1 {
            // Apply lowpass from crossover with next driver
            let xover_freq = crossover_freqs[i];
            let lp_filter = match drivers_data.crossover_type {
                crate::loss::CrossoverType::Butterworth2 => {
                    crate::iir::peq_butterworth_lowpass(2, xover_freq, sample_rate)
                }
                crate::loss::CrossoverType::LinkwitzRiley2 => {
                    crate::iir::peq_linkwitzriley_lowpass(2, xover_freq, sample_rate)
                }
                crate::loss::CrossoverType::LinkwitzRiley4 => {
                    crate::iir::peq_linkwitzriley_lowpass(4, xover_freq, sample_rate)
                }
            };
            let lp_response =
                crate::iir::compute_peq_response(driver_freq_grid, &lp_filter, sample_rate);
            response = response + lp_response;
        }

        // No need to normalize again - we already normalized the driver SPL at the start
        // The response is already centered around the gain value

        let color = match i {
            0 => "rgb(31, 119, 180)",  // Blue (woofer)
            1 => "rgb(255, 127, 14)",  // Orange (tweeter)
            2 => "rgb(44, 160, 44)",   // Green (midrange)
            3 => "rgb(214, 39, 40)",   // Red (super tweeter)
            _ => "rgb(128, 128, 128)", // Gray (fallback)
        };

        let trace = Scatter::new(driver_freq_grid.to_vec(), response.to_vec())
            .mode(Mode::Lines)
            .name(format!("Driver {} ({:+.1} dB)", i + 1, gains[i]))
            .line(plotly::common::Line::new().color(color).width(2.0));

        plot.add_trace(trace);
    }

    // Plot combined response (already computed above, now normalize it)
    let combined_response_normalized = &combined_response - combined_mean;

    let trace_combined = Scatter::new(freq_grid.to_vec(), combined_response_normalized.to_vec())
        .mode(Mode::Lines)
        .name("Combined Response")
        .line(plotly::common::Line::new().color("rgb(0, 0, 0)").width(3.0));

    plot.add_trace(trace_combined);

    // Add vertical lines for crossover frequencies
    let mut shapes = Vec::new();
    let mut annotations = Vec::new();
    for (i, &xover_freq) in crossover_freqs.iter().enumerate() {
        let shape = plotly::layout::Shape::new()
            .shape_type(plotly::layout::ShapeType::Line)
            .x_ref("x")
            .y_ref("paper")
            .x0(xover_freq)
            .x1(xover_freq)
            .y0(0.0)
            .y1(1.0)
            .line(
                plotly::layout::ShapeLine::new()
                    .color("rgba(150, 150, 150, 0.6)")
                    .width(2.0)
                    .dash(plotly::common::DashType::Dot),
            );
        shapes.push(shape);

        // Add annotation for crossover frequency
        let annotation = plotly::layout::Annotation::new()
            .x(xover_freq)
            .y(1.02)
            .x_ref("x")
            .y_ref("paper")
            .text(format!("Crossover {}: {:.0} Hz", i + 1, xover_freq))
            .show_arrow(false)
            .font(
                plotly::common::Font::new()
                    .size(10)
                    .color("rgb(100, 100, 100)"),
            )
            .x_anchor(plotly::common::Anchor::Center)
            .y_anchor(plotly::common::Anchor::Bottom);
        annotations.push(annotation);
    }

    // Create layout
    let crossover_type_str = match drivers_data.crossover_type {
        crate::loss::CrossoverType::Butterworth2 => "2nd order Butterworth",
        crate::loss::CrossoverType::LinkwitzRiley2 => "2nd order Linkwitz-Riley",
        crate::loss::CrossoverType::LinkwitzRiley4 => "4th order Linkwitz-Riley",
    };

    let layout = Layout::new()
        .title(format!(
            "Multi-Driver Crossover Optimization ({})",
            crossover_type_str
        ))
        .x_axis(
            Axis::new()
                .title("Frequency (Hz)".to_string())
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .grid_color("rgba(128, 128, 128, 0.2)"),
        )
        .y_axis(
            Axis::new()
                .title("SPL (dB)".to_string())
                .range(vec![-30.0, 30.0])
                .grid_color("rgba(128, 128, 128, 0.2)"),
        )
        .shapes(shapes)
        .annotations(annotations)
        .height(600)
        .hover_mode(plotly::layout::HoverMode::X);

    plot.set_layout(layout);

    plot
}

/// Generate and save an HTML plot for multi-driver optimization results
///
/// # Arguments
/// * `drivers_data` - Multi-driver measurement data
/// * `gains` - Optimized gain values for each driver (in dB)
/// * `crossover_freqs` - Optimized crossover frequencies (in Hz)
/// * `sample_rate` - Sample rate for filter design
/// * `output_path` - Path to save the HTML file
///
/// # Returns
/// * Result indicating success or failure
pub fn plot_drivers_results(
    drivers_data: &DriversLossData,
    gains: &[f64],
    crossover_freqs: &[f64],
    sample_rate: f64,
    output_path: &std::path::Path,
) -> Result<(), Box<dyn std::error::Error>> {
    use build_html::*;
    use std::fs::File;
    use std::io::Write;

    let plot = plot_drivers(drivers_data, gains, crossover_freqs, sample_rate);

    let title_text = format!(
        "{}-Way Speaker Crossover Optimization",
        drivers_data.drivers.len()
    );

    let html = HtmlPage::new()
        .with_title(&title_text)
        .with_script_link("https://cdn.plot.ly/plotly-3.2.0.min.js")
        .with_raw(plot.to_inline_html(Some("drivers")))
        .to_html_string();

    // Ensure parent directory exists before writing files
    let html_output_path = output_path.with_extension("html");
    if let Some(parent) = html_output_path.parent() {
        std::fs::create_dir_all(parent)
            .unwrap_or_else(|_| panic!("Failed to create output directory: {:?}", parent));
    }

    let mut file = File::create(&html_output_path)?;
    file.write_all(html.as_bytes())?;
    file.flush()?;

    Ok(())
}
