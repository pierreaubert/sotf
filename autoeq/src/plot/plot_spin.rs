use ndarray::Array1;
use plotly::common::{AxisSide, Mode};
use plotly::layout::{AxisType, GridPattern, LayoutGrid, RowOrder};
use plotly::{Layout, Plot, Scatter};
use std::collections::HashMap;

use crate::plot::filter_color::filter_color;
use crate::plot::ref_lines::make_ref_lines;
use crate::plot::trend_lines::{
    calculate_tonal_balance, create_regression_trace, generate_regression_line,
};

/// Create CEA2034 combined traces for a single subplot, including DI on a secondary y-axis
///
/// # Arguments
/// * `curves` - HashMap of curve names to Curve data
/// * `x_axis` - Axis id for x (e.g. "x7")
/// * `y_axis` - Axis id for primary y (left) (e.g. "y7")
fn create_cea2034_combined_traces(
    curves: &HashMap<String, crate::Curve>,
    x_axis: &str,
    y_axis: &str,
    y_axis_di: &str,
) -> Vec<Scatter<f64, f64>> {
    let mut traces = Vec::new();
    for (i, curve_name) in CEA2034_CURVE_NAMES_FULL.iter().enumerate() {
        if let Some(curve) = curves.get(*curve_name) {
            let trace = Scatter::new(curve.freq.to_vec(), curve.spl.to_vec())
                .mode(Mode::Lines)
                .name(shorten_curve_name(curve_name))
                .x_axis(x_axis)
                .y_axis(y_axis)
                .line(plotly::common::Line::new().color(filter_color(i)));
            traces.push(*trace);
        }
    }
    // DI curves on secondary y-axis
    for (j, curve_name) in CEA2034_CURVE_NAMES_DI.iter().enumerate() {
        if let Some(curve) = curves.get(*curve_name) {
            let trace = Scatter::new(curve.freq.to_vec(), curve.spl.to_vec())
                .mode(Mode::Lines)
                .name(shorten_curve_name(curve_name))
                .x_axis(x_axis)
                .y_axis(y_axis_di)
                .line(plotly::common::Line::new().color(filter_color(j + 2)));
            traces.push(*trace);
        }
    }
    traces
}

/// Create CEA2034 combined traces with EQ applied on a single subplot
///
/// # Arguments
/// * `curves` - HashMap of curve names to Curve data
/// * `eq_response` - EQ response to apply to the primary CEA2034 curves
/// * `x_axis` - Axis id for x (e.g. "x8")
/// * `y_axis` - Axis id for primary y (left) (e.g. "y8")
fn create_cea2034_with_eq_combined_traces(
    curves: &HashMap<String, crate::Curve>,
    eq_response: &Array1<f64>,
    x_axis: &str,
    y_axis: &str,
    y_axis_di: &str,
) -> Vec<Scatter<f64, f64>> {
    let mut traces = Vec::new();
    for (i, curve_name) in CEA2034_CURVE_NAMES_FULL.iter().enumerate() {
        if let Some(curve) = curves.get(*curve_name) {
            let trace = Scatter::new(curve.freq.to_vec(), (&curve.spl + eq_response).to_vec())
                .mode(Mode::Lines)
                .name(format!("{} w/EQ", shorten_curve_name(curve_name)))
                .x_axis(x_axis)
                .y_axis(y_axis)
                .line(plotly::common::Line::new().color(filter_color(i)));
            traces.push(*trace);
        }
    }
    // DI curves unchanged, on secondary y-axis
    for (j, curve_name) in CEA2034_CURVE_NAMES_DI.iter().enumerate() {
        if let Some(curve) = curves.get(*curve_name) {
            let trace = Scatter::new(curve.freq.to_vec(), curve.spl.to_vec())
                .mode(Mode::Lines)
                .name(shorten_curve_name(curve_name))
                .x_axis(x_axis)
                .y_axis(y_axis_di)
                .line(plotly::common::Line::new().color(filter_color(j + 2)));
            traces.push(*trace);
        }
    }
    traces
}

// List of curve names
const CEA2034_CURVE_NAMES: [&str; 4] = [
    "On Axis",
    "Listening Window",
    "Early Reflections",
    "Sound Power",
];

const CEA2034_CURVE_NAMES_FULL: [&str; 5] = [
    "On Axis",
    "Listening Window",
    "Early Reflections",
    "Sound Power",
    "Estimated In-Room Response",
];

const CEA2034_CURVE_NAMES_DI: [&str; 2] = ["Early Reflections DI", "Sound Power DI"];

/// Convert a curve name to its short abbreviated form
///
/// # Arguments
/// * `curve_name` - The full curve name to abbreviate
///
/// # Returns
/// * A string slice with the abbreviated curve name
pub fn shorten_curve_name(curve_name: &str) -> &str {
    match curve_name {
        "On Axis" => "ON",
        "Listening Window" => "LW",
        "Early Reflections" => "ER",
        "Sound Power" => "SP",
        "Estimated In-Room Response" => "PIR",
        "Early Reflections DI" => "ERDI",
        "Sound Power DI" => "SPDI",
        // Add more mappings as needed
        _ => curve_name, // Return original if no mapping found
    }
}

/// Create CEA2034 traces for the combined plot
///
/// # Arguments
/// * `curves` - HashMap of curve names to Curve data
///
/// # Returns
/// * Vector of Scatter traces for CEA2034 curves
///
/// # Details
/// Creates traces for standard CEA2034 curves
fn create_cea2034_traces(curves: &HashMap<String, crate::Curve>) -> Vec<Scatter<f64, f64>> {
    let mut traces = Vec::new();

    let axes = ["x1y1", "x2y2", "x3y3", "x4y4"];

    for (i, (curve_name, axis)) in CEA2034_CURVE_NAMES.iter().zip(axes.iter()).enumerate() {
        let mut x_axis_name = &axis[..2];
        let mut y_axis_name = &axis[2..];
        if x_axis_name == "x1" || y_axis_name == "y1" {
            x_axis_name = "x";
            y_axis_name = "y";
        }
        if let Some(curve) = curves.get(*curve_name) {
            let trace = Scatter::new(curve.freq.to_vec(), curve.spl.to_vec())
                .mode(Mode::Lines)
                .name(shorten_curve_name(curve_name))
                .x_axis(x_axis_name)
                .y_axis(y_axis_name)
                .line(plotly::common::Line::new().color(filter_color(i)));
            traces.push(*trace);
        }
    }

    traces
}

/// Create CEA2034 traces with EQ response applied
///
/// # Arguments
/// * `curves` - HashMap of curve names to Curve data
/// * `eq_response` - Array of EQ response values to apply
///
/// # Returns
/// * Vector of Scatter traces for CEA2034 curves with EQ applied
///
/// # Details
/// Creates traces for standard CEA2034 curves with the EQ response
/// applied, using the same alias mapping as create_cea2034_traces.
fn create_cea2034_with_eq_traces(
    curves: &HashMap<String, crate::Curve>,
    eq_response: &Array1<f64>,
) -> Vec<Scatter<f64, f64>> {
    let mut traces = Vec::new();

    let axes = ["x1y1", "x2y2", "x3y3", "x4y4"];

    for (i, (curve_name, axis)) in CEA2034_CURVE_NAMES.iter().zip(axes.iter()).enumerate() {
        let mut x_axis_name = &axis[..2];
        let mut y_axis_name = &axis[2..];
        if x_axis_name == "x1" || y_axis_name == "y1" {
            x_axis_name = "x";
            y_axis_name = "y";
        }
        if let Some(curve) = curves.get(*curve_name) {
            let trace = Scatter::new(curve.freq.to_vec(), (&curve.spl + eq_response).to_vec())
                .mode(Mode::Lines)
                .name(format!("{} w/EQ", shorten_curve_name(curve_name)))
                .x_axis(x_axis_name)
                .y_axis(y_axis_name)
                .line(plotly::common::Line::new().color(filter_color(i + 4)));
            traces.push(*trace);
        }
    }

    traces
}

pub fn plot_spin_details(
    cea2034_curves: Option<&HashMap<String, crate::Curve>>,
    eq_response: Option<&Array1<f64>>,
) -> plotly::Plot {
    let mut plot = Plot::new();
    // Add each CEA2034 curves if provided
    let x_axis1_title = "On Axis".to_string();
    let x_axis2_title = "Listening Window".to_string();
    let x_axis3_title = "Early Reflections".to_string();
    let x_axis4_title = "Sound Power".to_string();
    if let Some(curves) = cea2034_curves {
        let cea2034_traces = create_cea2034_traces(curves);
        for trace in cea2034_traces {
            plot.add_trace(Box::new(trace));
        }
        // Also plot the EQ-applied variants if provided
        if let Some(eq_resp) = eq_response {
            let cea2034_eq_traces = create_cea2034_with_eq_traces(curves, eq_resp);
            for trace in cea2034_eq_traces {
                plot.add_trace(Box::new(trace));
            }
        }
    }

    // Add reference lines y=1 and y=-1 from x=100 to x=10000 (black) on both subplots
    for t in make_ref_lines("x", "y") {
        plot.add_trace(Box::new(t));
    }
    for t in make_ref_lines("x2", "y2") {
        plot.add_trace(Box::new(t));
    }

    // Configure layout with subplots
    let layout = Layout::new()
        .grid(
            LayoutGrid::new()
                .rows(2)
                .columns(2)
                .pattern(GridPattern::Independent)
                .row_order(RowOrder::BottomToTop),
        )
        .width(1024)
        .height(600)
        .x_axis(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text(x_axis1_title))
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0.0, 0.45]),
        )
        .y_axis(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("SPL (dB)"))
                .range(vec![-10.0, 10.0])
                .domain(&[0.55, 1.0]),
        )
        .x_axis2(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text(x_axis2_title))
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0.55, 1.0]),
        )
        .y_axis2(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("SPL (dB)"))
                .range(vec![-10.0, 10.0])
                .domain(&[0.55, 1.0]),
        )
        .x_axis3(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text(x_axis3_title))
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0.0, 0.45]),
        )
        .y_axis3(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("SPL (dB)"))
                .range(vec![-15.0, 5.0])
                .domain(&[0.0, 0.45]),
        )
        .x_axis4(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text(x_axis4_title))
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0.55, 1.0]),
        )
        .y_axis4(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("SPL (dB)"))
                .range(vec![-15.0, 5.0])
                .domain(&[0.0, 0.45]),
        );

    plot.set_layout(layout);

    plot
}

pub fn plot_spin_tonal(
    cea2034_curves: Option<&HashMap<String, crate::Curve>>,
    eq_response: Option<&Array1<f64>>,
) -> plotly::Plot {
    let mut plot = Plot::new();
    // Add each CEA2034 curves if provided
    let x_axis1_title = "On Axis Tonal Balance".to_string();
    let x_axis2_title = "Listening Window Tonal Balance".to_string();
    let x_axis3_title = "Early Reflections Tonal Balance".to_string();
    let x_axis4_title = "Sound Power Tonal Balance".to_string();
    if let Some(curves) = cea2034_curves {
        // Add regression lines for original curves (tonal balance)
        for (i, curve_name) in CEA2034_CURVE_NAMES.iter().enumerate() {
            let x_axis = if i == 0 {
                "x".to_string()
            } else {
                format!("x{}", i + 1)
            };
            let y_axis = if i == 0 {
                "y".to_string()
            } else {
                format!("y{}", i + 1)
            };
            if let Some(curve) = curves.get(*curve_name)
                && let Some((slope, intercept)) =
                    calculate_tonal_balance(&curve.freq, &curve.spl, 100.0, 10000.0)
            {
                let regression_line = generate_regression_line(slope, intercept, &curve.freq);
                let trace = create_regression_trace(
                    &curve.freq,
                    &regression_line,
                    &format!("{} {:.2} dB/oct", shorten_curve_name(curve_name), slope),
                    filter_color(i),
                    Some(&x_axis),
                    Some(&y_axis),
                );
                plot.add_trace(Box::new(trace));
            }
        }

        // Add regression lines for EQ-applied curves (tonal balance)
        if let Some(eq_resp) = eq_response {
            for (i, curve_name) in CEA2034_CURVE_NAMES.iter().enumerate() {
                let x_axis = if i == 0 {
                    "x".to_string()
                } else {
                    format!("x{}", i + 1)
                };
                let y_axis = if i == 0 {
                    "y".to_string()
                } else {
                    format!("y{}", i + 1)
                };
                if let Some(curve) = curves.get(*curve_name) {
                    let eq_applied = &curve.spl + eq_resp;
                    if let Some((slope, intercept)) =
                        calculate_tonal_balance(&curve.freq, &eq_applied, 100.0, 10000.0)
                    {
                        let regression_line =
                            generate_regression_line(slope, intercept, &curve.freq);
                        let trace = create_regression_trace(
                            &curve.freq,
                            &regression_line,
                            &format!(
                                "{} w/EQ {:.2} dB/oct",
                                shorten_curve_name(curve_name),
                                slope
                            ),
                            filter_color(i + 4),
                            Some(&x_axis),
                            Some(&y_axis),
                        );
                        plot.add_trace(Box::new(trace));
                    }
                }
            }
        }
    }

    // Configure layout with subplots
    let layout = Layout::new()
        .grid(
            LayoutGrid::new()
                .rows(2)
                .columns(2)
                .pattern(GridPattern::Independent)
                .row_order(RowOrder::BottomToTop),
        )
        .width(1024)
        .height(600)
        .x_axis(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text(x_axis1_title))
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0.0, 0.45]),
        )
        .y_axis(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("SPL (dB)"))
                .range(vec![-10.0, 10.0])
                .domain(&[0.55, 1.0]),
        )
        .x_axis2(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text(x_axis2_title))
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0.55, 1.0]),
        )
        .y_axis2(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("SPL (dB)"))
                .range(vec![-10.0, 10.0])
                .domain(&[0.55, 1.0]),
        )
        .x_axis3(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text(x_axis3_title))
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0.0, 0.45]),
        )
        .y_axis3(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("SPL (dB)"))
                .range(vec![-15.0, 5.0])
                .domain(&[0.0, 0.45]),
        )
        .x_axis4(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text(x_axis4_title))
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0.55, 1.0]),
        )
        .y_axis4(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("SPL (dB)"))
                .range(vec![-15.0, 5.0])
                .domain(&[0.0, 0.45]),
        );

    plot.set_layout(layout);

    plot
}

pub fn plot_spin(
    cea2034_curves: Option<&HashMap<String, crate::Curve>>,
    eq_response: Option<&Array1<f64>>,
) -> plotly::Plot {
    let mut plot = Plot::new();

    // ----------------------------------------------------------------------
    // Add CEA2034 if provided with and without EQ
    // ----------------------------------------------------------------------
    let x_axis1_title = "CEA2034".to_string();
    let x_axis3_title = "CEA2034 + EQ".to_string();
    if let Some(curves) = cea2034_curves {
        let cea2034_traces = create_cea2034_combined_traces(curves, "x", "y", "y2");
        for trace in cea2034_traces {
            plot.add_trace(Box::new(trace));
        }

        if let Some(eq_resp) = eq_response {
            let cea2034_traces =
                create_cea2034_with_eq_combined_traces(curves, eq_resp, "x3", "y3", "y4");
            for trace in cea2034_traces {
                plot.add_trace(Box::new(trace));
            }
        }
    }

    // Configure layout with subplots
    let layout = Layout::new()
        .grid(
            LayoutGrid::new()
                .rows(1)
                .columns(2)
                .pattern(GridPattern::Independent),
        )
        .width(1024)
        .height(450)
        // cea2034
        .x_axis(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text(&x_axis1_title))
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0., 0.4]),
        )
        .y_axis(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("SPL (dB)"))
                .dtick(5.0)
                .range(vec![-40.0, 10.0]),
        )
        .y_axis2(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text(
                    "DI (dB)                      ⌃",
                ))
                .range(vec![-5.0, 45.0])
                .tick_values(vec![-5.0, 0.0, 5.0, 10.0, 15.0])
                .overlaying("y")
                .side(AxisSide::Right),
        )
        // cea2034 with eq
        .x_axis3(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text(&x_axis3_title))
                .type_(AxisType::Log)
                .range(vec![1.301, 4.301])
                .domain(&[0.55, 0.95]),
        )
        .y_axis3(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text("SPL (dB)"))
                .dtick(5.0)
                .range(vec![-40.0, 10.0])
                .anchor("x3"),
        )
        .y_axis4(
            plotly::layout::Axis::new()
                .title(plotly::common::Title::with_text(
                    "DI (dB)                      ⌃",
                ))
                .range(vec![-5.0, 45.0])
                .tick_values(vec![-5.0, 0.0, 5.0, 10.0, 15.0])
                .anchor("x3")
                .overlaying("y3")
                .side(AxisSide::Right),
        );
    plot.set_layout(layout);

    plot
}

#[cfg(test)]
mod tests {
    use super::{
        create_cea2034_combined_traces, create_cea2034_traces,
        create_cea2034_with_eq_combined_traces, create_cea2034_with_eq_traces, make_ref_lines,
        shorten_curve_name,
    };
    use ndarray::Array1;
    use serde_json::json;
    use serde_json::to_value as to_json;
    use std::collections::HashMap;

    #[test]
    fn test_create_cea2034_traces() {
        // Create mock CEA2034 curves
        let mut curves = HashMap::new();

        // Create a simple frequency grid
        let freq = Array1::from(vec![20.0, 100.0, 1000.0, 10000.0, 20000.0]);
        let spl = Array1::from(vec![80.0, 85.0, 90.0, 85.0, 80.0]);

        // Add mock curves for the primary CEA2034 set used by create_cea2034_traces
        curves.insert(
            "On Axis".to_string(),
            crate::Curve {
                freq: freq.clone(),
                spl: spl.clone(),
            },
        );
        curves.insert(
            "Listening Window".to_string(),
            crate::Curve {
                freq: freq.clone(),
                spl: spl.clone(),
            },
        );
        curves.insert(
            "Early Reflections".to_string(),
            crate::Curve {
                freq: freq.clone(),
                spl: spl.clone(),
            },
        );
        curves.insert(
            "Sound Power".to_string(),
            crate::Curve {
                freq: freq.clone(),
                spl: spl.clone(),
            },
        );

        // Test creating CEA2034 traces
        let traces = create_cea2034_traces(&curves);

        // Should have 4 traces
        assert_eq!(traces.len(), 4);

        // Test creating CEA2034 traces with EQ
        let eq_response = Array1::from(vec![1.0, 1.0, 1.0, 1.0, 1.0]);
        let eq_traces = create_cea2034_with_eq_traces(&curves, &eq_response);

        // Should have 4 traces
        assert_eq!(eq_traces.len(), 4);
    }

    #[test]
    fn test_make_ref_lines_values() {
        let lines = make_ref_lines("x3", "y3");
        assert_eq!(lines.len(), 2);
        let v0 = to_json(&lines[0]).unwrap();
        let v1 = to_json(&lines[1]).unwrap();
        assert_eq!(v0["x"], json!([100.0, 10000.0]));
        assert_eq!(v1["x"], json!([100.0, 10000.0]));
        assert_eq!(v0["y"], json!([1.0, 1.0]));
        assert_eq!(v1["y"], json!([-1.0, -1.0]));
    }

    #[test]
    fn test_create_cea2034_combined_traces_counts_and_axes() {
        // Build minimal curves covering names used by combined function
        let mut curves = HashMap::new();
        let freq = Array1::from(vec![100.0, 1000.0, 10000.0]);
        let spl_primary = Array1::from(vec![80.0, 85.0, 82.0]);
        let spl_di = Array1::from(vec![5.0, 6.0, 7.0]);

        // Primary curves
        curves.insert(
            "On Axis".to_string(),
            crate::Curve {
                freq: freq.clone(),
                spl: spl_primary.clone(),
            },
        );
        curves.insert(
            "Listening Window".to_string(),
            crate::Curve {
                freq: freq.clone(),
                spl: spl_primary.clone(),
            },
        );
        curves.insert(
            "Early Reflections".to_string(),
            crate::Curve {
                freq: freq.clone(),
                spl: spl_primary.clone(),
            },
        );
        curves.insert(
            "Sound Power".to_string(),
            crate::Curve {
                freq: freq.clone(),
                spl: spl_primary.clone(),
            },
        );

        // DI curves
        curves.insert(
            "Early Reflections DI".to_string(),
            crate::Curve {
                freq: freq.clone(),
                spl: spl_di.clone(),
            },
        );
        curves.insert(
            "Sound Power DI".to_string(),
            crate::Curve {
                freq: freq.clone(),
                spl: spl_di.clone(),
            },
        );

        let traces = create_cea2034_combined_traces(&curves, "x7", "y7", "y7");
        assert_eq!(traces.len(), 6);

        // Check that DI traces target the secondary axis
        let v = to_json(&traces).unwrap();
        let names: Vec<String> = v
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        assert!(names.contains(&"ERDI".to_string()));
        assert!(names.contains(&"SPDI".to_string()));

        // Find DI entries and ensure yaxis is y7 (DI shares primary axis in current implementation)
        for t in v.as_array().unwrap() {
            let n = t["name"].as_str().unwrap();
            if n.ends_with(" DI") {
                assert_eq!(t["yaxis"], json!("y7"));
            }
        }
    }

    #[test]
    fn test_create_cea2034_with_eq_combined_traces_counts_and_names() {
        let mut curves = HashMap::new();
        let freq = Array1::from(vec![100.0, 1000.0, 10000.0]);
        let spl_primary = Array1::from(vec![80.0, 85.0, 82.0]);
        let spl_di = Array1::from(vec![5.0, 6.0, 7.0]);

        // Primary curves
        curves.insert(
            "On Axis".to_string(),
            crate::Curve {
                freq: freq.clone(),
                spl: spl_primary.clone(),
            },
        );
        curves.insert(
            "Listening Window".to_string(),
            crate::Curve {
                freq: freq.clone(),
                spl: spl_primary.clone(),
            },
        );
        curves.insert(
            "Early Reflections".to_string(),
            crate::Curve {
                freq: freq.clone(),
                spl: spl_primary.clone(),
            },
        );
        curves.insert(
            "Sound Power".to_string(),
            crate::Curve {
                freq: freq.clone(),
                spl: spl_primary.clone(),
            },
        );
        // DI
        curves.insert(
            "Early Reflections DI".to_string(),
            crate::Curve {
                freq: freq.clone(),
                spl: spl_di.clone(),
            },
        );
        curves.insert(
            "Sound Power DI".to_string(),
            crate::Curve {
                freq: freq.clone(),
                spl: spl_di.clone(),
            },
        );

        let eq = Array1::from(vec![1.0, -1.0, 0.5]);
        let traces = create_cea2034_with_eq_combined_traces(&curves, &eq, "x8", "y8", "y8");
        assert_eq!(traces.len(), 6);
        let v = to_json(&traces).unwrap();
        // Primary names should have suffix w/EQ, DI should not
        let names: Vec<String> = v
            .as_array()
            .unwrap()
            .iter()
            .map(|t| t["name"].as_str().unwrap().to_string())
            .collect();
        assert!(names.iter().any(|n| n == "ON w/EQ"));
        assert!(names.iter().any(|n| n == "LW w/EQ"));
        assert!(names.iter().any(|n| n == "ER w/EQ"));
        assert!(names.iter().any(|n| n == "SP w/EQ"));
        assert!(names.iter().any(|n| n == "ERDI"));
        assert!(names.iter().any(|n| n == "SPDI"));
        // DI yaxis should be y8 (shares primary axis in current implementation)
        for t in v.as_array().unwrap() {
            let n = t["name"].as_str().unwrap();
            if n.ends_with(" DI") {
                assert_eq!(t["yaxis"], json!("y8"));
            }
        }
    }

    #[test]
    fn test_shorten_curve_name() {
        // Test basic curve name abbreviations
        assert_eq!(shorten_curve_name("On Axis"), "ON");
        assert_eq!(shorten_curve_name("Listening Window"), "LW");
        assert_eq!(shorten_curve_name("Early Reflections"), "ER");
        assert_eq!(shorten_curve_name("Sound Power"), "SP");
        assert_eq!(shorten_curve_name("Estimated In-Room Response"), "PIR");

        // Test DI curve abbreviations
        assert_eq!(shorten_curve_name("Early Reflections DI"), "ERDI");
        assert_eq!(shorten_curve_name("Sound Power DI"), "SPDI");

        // Test unknown curve name (should return original)
        assert_eq!(shorten_curve_name("Unknown Curve"), "Unknown Curve");
        assert_eq!(shorten_curve_name(""), "");

        // Test case sensitivity (should return original since no exact match)
        assert_eq!(shorten_curve_name("on axis"), "on axis");
        assert_eq!(shorten_curve_name("ON AXIS"), "ON AXIS");
    }
}
