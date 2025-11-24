//! CLI tool for computing headphone loss from frequency response files
//!
//! This tool computes the headphone preference loss score based on the model from
//! 'A Statistical Model that Predicts Listeners' Preference Ratings of In-Ear Headphones'
//! by Sean Olive et al. Lower scores indicate better predicted preference.
//!
//! Output plots are automatically saved to $AUTOEQ_DIR/data_generated/headphone_loss_plots.html
//!
//! Usage:
//!   cargo run --example headphone_loss_demo -- --spl <file> --target <file> [--smooth] [--smooth-n <n>]

use autoeq::Curve;
use autoeq::loss::headphone_loss;
use autoeq::read::{
    create_log_frequency_grid, normalize_and_interpolate_response, read_curve_from_csv,
    smooth_one_over_n_octave,
};
use autoeq_env::get_data_generated_dir;

use clap::Parser;
use plotly::common::Mode;
use plotly::{Plot, Scatter};
use serde_json;
use std::path::PathBuf;

#[derive(Parser, Debug)]
#[command(
    name = "headphone_loss_demo",
    about = "Compute headphone preference score from frequency response measurements",
    long_about = "Computes the headphone preference loss score based on the model from \n'A Statistical Model that Predicts Listeners' Preference Ratings of In-Ear Headphones' \nby Sean Olive et al. Lower scores indicate better predicted preference."
)]
struct Args {
    /// Path to SPL (frequency response) file (CSV or text with freq,spl columns)
    #[arg(long)]
    spl: PathBuf,

    /// Path to target frequency response file (CSV or text with freq,spl columns)
    #[arg(long)]
    target: PathBuf,

    /// Enable smoothing (regularization) of the inverted target curve
    #[arg(long, default_value_t = true)]
    pub smooth: bool,

    /// Smoothing level as 1/N octave (N in [1..24]). Example: N=6 => 1/6 octave smoothing
    #[arg(long, default_value_t = 2)]
    pub smooth_n: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // freqs on which we normalize every curve: 12 points per octave between 20 and 20kHz
    let freqs = create_log_frequency_grid(10 * 12, 20.0, 20000.0);

    // Load SPL data
    println!("Loading SPL data from: {:?}", args.spl);
    let input_curve_raw = read_curve_from_csv(&args.spl)?;
    println!(
        "  Loaded headphone response #{} data points from {:.1} Hz to {:.1} Hz SPL from {:.1} to {:.1} dB",
        input_curve_raw.freq.len(),
        input_curve_raw.freq[0],
        input_curve_raw.freq[input_curve_raw.freq.len() - 1],
        input_curve_raw.spl.fold(f64::INFINITY, |a, &b| a.min(b)),
        input_curve_raw
            .spl
            .fold(f64::NEG_INFINITY, |a, &b| a.max(b)),
    );

    let input_curve = normalize_and_interpolate_response(&freqs, &input_curve_raw);
    println!(
        " Normalized headphone response #{} data points from {:.1} Hz to {:.1} Hz SPL from {:.1} to {:.1} dB",
        input_curve.freq.len(),
        input_curve.freq[0],
        input_curve.freq[input_curve.freq.len() - 1],
        input_curve.spl.fold(f64::INFINITY, |a, &b| a.min(b)),
        input_curve.spl.fold(f64::NEG_INFINITY, |a, &b| a.max(b)),
    );

    let target_curve_raw = read_curve_from_csv(&args.target)?;
    println!(
        "  Loaded {} targets from {:.1} Hz to {:.1} Hz SPL from {:.1} to {:.1} dB",
        target_curve_raw.freq.len(),
        target_curve_raw.freq[0],
        target_curve_raw.freq[target_curve_raw.freq.len() - 1],
        target_curve_raw.spl.fold(f64::INFINITY, |a, &b| a.min(b)),
        target_curve_raw
            .spl
            .fold(f64::NEG_INFINITY, |a, &b| a.max(b)),
    );

    let target_curve = normalize_and_interpolate_response(&freqs, &target_curve_raw);

    // compute deviation and potentially smooth it
    let deviation_spl = &target_curve.spl - &input_curve.spl;
    let deviation = Curve {
        freq: freqs.clone(),
        spl: deviation_spl,
    };
    let smooth_deviation = if args.smooth {
        smooth_one_over_n_octave(&deviation, args.smooth_n)
    } else {
        deviation.clone()
    };

    // Compute headphone loss and create plots
    let score = headphone_loss(&smooth_deviation);

    // Print results
    println!("\n{}", "=".repeat(50));
    println!("Headphone Loss Score: {:.3}", -score);
    println!("{}", "=".repeat(50));

    // Get data_generated directory and create output path
    let data_generated_dir = get_data_generated_dir()
        .map_err(|e| format!("Failed to get data_generated directory: {}", e))?;
    let output_path = data_generated_dir.join("headphone_loss_plots.html");

    // Generate plots
    generate_plots(
        &input_curve,
        &target_curve,
        &deviation,
        &smooth_deviation,
        &output_path,
    )?;

    Ok(())
}

/// Generate plots for the input curve and target curve (if provided)
/// and their normalized versions
fn generate_plots(
    input_curve: &Curve,
    target_curve: &Curve,
    deviation: &Curve,
    smooth_deviation: &Curve,
    output_path: &PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    // Create plot 1: Input curve and target curve
    let mut plot1 = Plot::new();

    // Add input curve
    let input_trace = Scatter::new(input_curve.freq.to_vec(), input_curve.spl.to_vec())
        .mode(Mode::Lines)
        .name("Input Curve");
    plot1.add_trace(input_trace);

    let target_trace = Scatter::new(target_curve.freq.to_vec(), target_curve.spl.to_vec())
        .mode(Mode::Lines)
        .name("Harmann Target Curve");
    plot1.add_trace(target_trace);

    // Configure layout for plot 1
    let layout1 = plotly::layout::Layout::new()
        .title(plotly::common::Title::with_text(
            "Input Curve vs Target Curve",
        ))
        .legend(plotly::layout::Legend::new().x(0.05).y(0.1))
        .x_axis(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("Frequency (Hz)"))
                .type_(plotly::layout::AxisType::Log),
        )
        .y_axis(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("SPL (dB)"))
                .range(vec![-10.0, 10.0]),
        );
    plot1.set_layout(layout1);

    // Create plot 2: Normalized curves
    let mut plot2 = Plot::new();

    // Add normalized input curve
    plot2.add_trace(
        Scatter::new(deviation.freq.to_vec(), deviation.spl.to_vec())
            .mode(Mode::Lines)
            .name("Normalized Deviation"),
    );
    plot2.add_trace(
        Scatter::new(
            smooth_deviation.freq.to_vec(),
            smooth_deviation.spl.to_vec(),
        )
        .mode(Mode::Lines)
        .name("Smooth Normalized Deviation"),
    );

    // Configure layout for plot 2
    let layout2 = plotly::layout::Layout::new()
        .title(plotly::common::Title::with_text("Normalized Curves"))
        .legend(plotly::layout::Legend::new().x(0.05).y(0.9))
        .x_axis(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("Frequency (Hz)"))
                .type_(plotly::layout::AxisType::Log),
        )
        .y_axis(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("Normalized SPL (dB)"))
                .range(vec![-10.0, 10.0]),
        );
    plot2.set_layout(layout2);

    // Create HTML output
    let html_content = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Headphone Loss Analysis Plots</title>
    <script src="https://cdn.plot.ly/plotly-3.2.0.min.js"></script>
</head>
<body>
    <h1>Headphone Loss Analysis Plots</h1>
    <div id="plot1"></div>
    <div id="plot2"></div>
    <script>
        var plot1 = {};
        Plotly.newPlot('plot1', plot1.data, plot1.layout);

        var plot2 = {};
        Plotly.newPlot('plot2', plot2.data, plot2.layout);
    </script>
</body>
</html>"#,
        serde_json::to_string(&plot1).unwrap(),
        serde_json::to_string(&plot2).unwrap()
    );

    // Write HTML file
    std::fs::write(output_path, html_content)?;
    println!("\nPlots saved to: {:?}", output_path);

    Ok(())
}
