use plotly::Scatter;
use plotly::common::Mode;

// Create two horizontal reference lines at y=1 and y=-1 spanning x=100..10000 for a given subplot axes
pub fn make_ref_lines(x_axis: &str, y_axis: &str) -> Vec<Scatter<f64, f64>> {
    let x_ref = vec![100.0_f64, 10000.0_f64];
    let y_pos = vec![1.0_f64, 1.0_f64];
    let y_neg = vec![-1.0_f64, -1.0_f64];

    let ref_pos = Scatter::new(x_ref.clone(), y_pos)
        .mode(Mode::Lines)
        .name("+1 dB ref")
        .x_axis(x_axis)
        .y_axis(y_axis)
        .line(plotly::common::Line::new().color("#000000").width(1.0));
    let ref_neg = Scatter::new(x_ref, y_neg)
        .mode(Mode::Lines)
        .name("-1 dB ref")
        .x_axis(x_axis)
        .y_axis(y_axis)
        .line(plotly::common::Line::new().color("#000000").width(1.0));

    vec![*ref_pos, *ref_neg]
}
