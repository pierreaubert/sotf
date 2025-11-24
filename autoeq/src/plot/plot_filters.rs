use ndarray::Array1;
use plotly::common::{Anchor, Mode};
use plotly::layout::{
    Annotation, AxisType, GridPattern, LayoutGrid, RowOrder, Shape, ShapeLayer, ShapeLine,
    ShapeType,
};
use plotly::{Layout, Plot, Scatter};

use crate::iir::{Biquad, BiquadFilterType};
use crate::param_utils::determine_filter_type;
use crate::plot::filter_color::filter_color;
use crate::plot::ref_lines::make_ref_lines;

/// Create semi-transparent green rectangles to highlight frequency ranges
/// outside the optimization bounds (min_freq to max_freq)
fn make_freq_range_shapes(min_freq: f64, max_freq: f64) -> Vec<Shape> {
    let mut shapes = Vec::new();

    // Y-axis ranges for each subplot
    let y_ranges = [
        (-10.0, 10.0), // Subplot 1 (x, y)
        (-10.0, 10.0), // Subplot 2 (x2, y2)
        (-5.0, 5.0),   // Subplot 3 (x3, y3)
        (-10.0, 10.0), // Subplot 4 (x4, y4)
    ];

    // Axis references for each subplot
    let axis_refs = [("x", "y"), ("x2", "y2"), ("x3", "y3"), ("x4", "y4")];

    for ((x_ref, y_ref), (y_min, y_max)) in axis_refs.iter().zip(y_ranges.iter()) {
        // Left rectangle: 20 Hz to min_freq
        if min_freq > 20.0 {
            let shape = Shape::new()
                .shape_type(ShapeType::Rect)
                .x_ref(x_ref)
                .y_ref(y_ref)
                .x0(20.0_f64.log10())
                .x1(min_freq.log10())
                .y0(*y_min)
                .y1(*y_max)
                .fill_color("rgba(144, 238, 144, 0.3)")
                .layer(ShapeLayer::Below)
                .line(ShapeLine::new().width(0.0));
            shapes.push(shape);
        }

        // Right rectangle: max_freq to 20 kHz
        if max_freq < 20000.0 {
            let shape = Shape::new()
                .shape_type(ShapeType::Rect)
                .x_ref(x_ref)
                .y_ref(y_ref)
                .x0(max_freq.log10())
                .x1(20000.0_f64.log10())
                .y0(*y_min)
                .y1(*y_max)
                .fill_color("rgba(144, 238, 144, 0.3)")
                .layer(ShapeLayer::Below)
                .line(ShapeLine::new().width(0.0));
            shapes.push(shape);
        }
    }

    shapes
}

pub fn plot_filters(
    args: &crate::cli::Args,
    input_curve: &crate::Curve,
    target_curve: &crate::Curve,
    deviation_curve: &crate::Curve,
    optimized_params: &[f64],
) -> plotly::Plot {
    let freqs = input_curve.freq.clone();
    let mut plot = Plot::new();

    // Compute combined response on the same frequency grid as input_curve for the new subplots
    let mut filters: Vec<(usize, f64, f64, f64)> = (0..args.num_filters)
        .map(|i| {
            (
                i,
                10f64.powf(optimized_params[i * 3]),
                optimized_params[i * 3 + 1],
                optimized_params[i * 3 + 2],
            )
        })
        .collect();
    filters.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal));

    // For the first subplot (individual filters), compute responses on plot_freqs
    let mut combined_response: Array1<f64> = Array1::zeros(freqs.len());
    let peq_model = args.effective_peq_model();
    for (display_idx, (orig_i, f0, q, gain)) in filters.iter().enumerate() {
        let ftype = determine_filter_type(*orig_i, args.num_filters, peq_model, None);
        let filter = Biquad::new(ftype, *f0, args.sample_rate, *q, *gain);
        // Compute filter response on plot_freqs for the first subplot
        let filter_response = filter.np_log_result(&freqs);
        combined_response += &filter_response;

        let label = match ftype {
            BiquadFilterType::Highpass | BiquadFilterType::HighpassVariableQ => "HPQ",
            BiquadFilterType::Lowpass => "LP",
            BiquadFilterType::Lowshelf => "LS",
            BiquadFilterType::Highshelf => "HS",
            BiquadFilterType::Bandpass => "BP",
            BiquadFilterType::Notch => "NO",
            _ => "PK",
        };
        let individual_trace = Scatter::new(freqs.to_vec(), filter_response.to_vec())
            .mode(Mode::Lines)
            .name(format!("{} {} at {:5.0}Hz", label, orig_i + 1, f0))
            .marker(
                plotly::common::Marker::new()
                    .color(filter_color(display_idx))
                    .size(1),
            );
        plot.add_trace(individual_trace);
    }

    // Add total combined response on the first and second subplots
    let iir_trace1 = Scatter::new(freqs.to_vec(), combined_response.to_vec())
        .mode(Mode::Lines)
        .name("autoEQ")
        .line(plotly::common::Line::new().color("#000000").width(2.0));
    plot.add_trace(iir_trace1);

    let iir_trace2 = Scatter::new(freqs.to_vec(), combined_response.to_vec())
        .mode(Mode::Lines)
        .name("autoEQ")
        .show_legend(false)
        .x_axis("x2")
        .y_axis("y2")
        .line(plotly::common::Line::new().color("000000").width(2.0));
    plot.add_trace(iir_trace2);

    // Interpolate deviation_curve to plot_freqs for the second subplot
    let target_trace2 = Scatter::new(freqs.to_vec(), deviation_curve.spl.to_vec())
        .mode(Mode::Lines)
        .name("Deviation")
        .x_axis("x2")
        .y_axis("y2")
        .line(plotly::common::Line::new().color(filter_color(0)));
    plot.add_trace(target_trace2);

    let error = &deviation_curve.spl - &combined_response;
    let target_trace4 = Scatter::new(freqs.to_vec(), error.to_vec())
        .mode(Mode::Lines)
        .name("Error")
        .x_axis("x3")
        .y_axis("y3")
        .line(plotly::common::Line::new().color(filter_color(1)));
    plot.add_trace(target_trace4);

    // Add input curve and target curve subplot (new subplot)
    let target_trace4 = Scatter::new(freqs.to_vec(), target_curve.spl.to_vec())
        .mode(Mode::Lines)
        .name("Target")
        .show_legend(false)
        .x_axis("x4")
        .y_axis("y4")
        .line(
            plotly::common::Line::new()
                .color(filter_color(0))
                .width(2.0),
        );
    plot.add_trace(target_trace4);

    let input_trace = Scatter::new(input_curve.freq.to_vec(), input_curve.spl.to_vec())
        .mode(Mode::Lines)
        .name("Input")
        .x_axis("x4")
        .y_axis("y4")
        .line(plotly::common::Line::new().color(filter_color(1)));
    plot.add_trace(input_trace);

    // Add input curve + EQ and target curve subplot (new subplot)
    let input_plus_eq_trace = Scatter::new(
        input_curve.freq.to_vec(),
        (&input_curve.spl + &combined_response).to_vec(),
    )
    .mode(Mode::Lines)
    .name("Input + EQ")
    .x_axis("x4")
    .y_axis("y4")
    .line(
        plotly::common::Line::new()
            .color(filter_color(2))
            .width(3.0),
    );
    plot.add_trace(input_plus_eq_trace);

    // Add reference lines
    let ref_lines3 = make_ref_lines("x3", "y3");
    for ref_line in ref_lines3 {
        plot.add_trace(Box::new(ref_line));
    }

    // Configure layout with subplots
    let mut layout = Layout::new()
        .grid(
            LayoutGrid::new()
                .rows(2)
                .columns(2)
                .pattern(GridPattern::Independent)
                .row_order(RowOrder::TopToBottom),
        )
        .width(1024)
        .height(800)
        .x_axis(
            plotly::layout::Axis::new()
                .title("Frequency (Hz)".to_string())
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0., 0.45]),
        ) // log10(20) to log10(20000)
        .y_axis(
            plotly::layout::Axis::new()
                .title("SPL (dB)".to_string())
                .dtick(1.0)
                .range(vec![-10.0, 10.0]),
        )
        .x_axis2(
            plotly::layout::Axis::new()
                .title("Frequency (Hz)".to_string())
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0.55, 1.0]),
        ) // log10(20) to log10(20000)
        .y_axis2(
            plotly::layout::Axis::new()
                .title("SPL (dB)".to_string())
                .dtick(1.0)
                .range(vec![-10.0, 10.0]),
        )
        .x_axis3(
            plotly::layout::Axis::new()
                .title("Frequency (Hz)".to_string())
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0., 0.45]),
        )
        .y_axis3(
            plotly::layout::Axis::new()
                .title("SPL (dB)".to_string())
                .dtick(1.0)
                .range(vec![-5.0, 5.0]),
        )
        .x_axis4(
            plotly::layout::Axis::new()
                .title("Frequency (Hz)".to_string())
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0.55, 1.0]),
        )
        .y_axis4(
            plotly::layout::Axis::new()
                .title("SPL (dB)".to_string())
                .dtick(1.0)
                .range(vec![-10.0, 10.0]),
        );

    layout.add_annotation(
        Annotation::new()
            .y_ref("y domain")
            .y_anchor(Anchor::Bottom)
            .y(1)
            .text("IIR filters and Sum of filters")
            .x_ref("x domain")
            .x_anchor(Anchor::Center)
            .x(0.5)
            .show_arrow(false),
    );

    layout.add_annotation(
        Annotation::new()
            .y_ref("y2 domain")
            .y_anchor(Anchor::Bottom)
            .y(1)
            .text("Autoeq v.s. Deviation from target")
            .x_ref("x2 domain")
            .x_anchor(Anchor::Center)
            .x(0.5)
            .show_arrow(false),
    );

    layout.add_annotation(
        Annotation::new()
            .y_ref("y3 domain")
            .y_anchor(Anchor::Bottom)
            .y(1)
            .text("Error = Autoeq-Deviation (zoomed)")
            .x_ref("x3 domain")
            .x_anchor(Anchor::Center)
            .x(0.5)
            .show_arrow(false),
    );

    layout.add_annotation(
        Annotation::new()
            .y_ref("y4 domain")
            .y_anchor(Anchor::Bottom)
            .y(1)
            .text("Response w/ autoEQ")
            .x_ref("x4 domain")
            .x_anchor(Anchor::Center)
            .x(0.5)
            .show_arrow(false),
    );

    // Add frequency range shapes to highlight regions outside optimization bounds
    let freq_shapes = make_freq_range_shapes(args.min_freq, args.max_freq);
    for shape in freq_shapes {
        layout.add_shape(shape);
    }

    plot.set_layout(layout);

    plot
}
