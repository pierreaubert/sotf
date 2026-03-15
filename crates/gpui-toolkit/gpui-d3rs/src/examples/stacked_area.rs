//! Stacked Area Chart — <https://observablehq.com/@d3/stacked-area-chart>
//!
//! Demonstrates: `Stack` with none offset, `LinearScale`, area path generation.

use crate::scale::{LinearScale, Scale};
use crate::shape::curve::Curve;
use crate::shape::path::PathBuilder;
use crate::shape::stack::{Stack, StackOffset, StackOrder, StackSeries};

#[derive(Debug)]
pub struct StackedAreaResult {
    pub width: f64,
    pub height: f64,
    pub categories: Vec<String>,
    pub series: Vec<StackSeries>,
    pub area_paths: Vec<(String, String)>, // (key, svg_path)
    pub y_domain: [f64; 2],
}

/// Default time series (4 categories, 12 months).
pub fn default_data() -> (Vec<String>, Vec<Vec<f64>>) {
    let categories: Vec<String> = ["Electronics", "Clothing", "Food", "Transport"]
        .iter()
        .map(|s| s.to_string())
        .collect();
    let months = 12;
    let mut matrix = Vec::with_capacity(months);
    for m in 0..months {
        let mut row = Vec::with_capacity(categories.len());
        for ci in 0..categories.len() {
            let base = 50.0 + ci as f64 * 20.0;
            let val = base
                + 15.0 * (m as f64 * 0.5 + ci as f64 * 1.2).sin()
                + 5.0 * (m as f64 * 0.8 + ci as f64 * 0.5).cos();
            row.push((val * 100.0).round() / 100.0);
        }
        matrix.push(row);
    }
    (categories, matrix)
}

pub fn compute(categories: &[String], matrix: &[Vec<f64>]) -> StackedAreaResult {
    let width = 928.0;
    let height = 500.0;
    let margin_top = 20.0;
    let margin_right = 20.0;
    let margin_bottom = 30.0;
    let margin_left = 40.0;
    let n = matrix.len();

    let stack = Stack::new()
        .keys(categories.to_vec())
        .order(StackOrder::None)
        .offset(StackOffset::None);
    let series = stack.generate(matrix);

    let x_scale = LinearScale::new()
        .domain(0.0, (n - 1) as f64)
        .range(margin_left, width - margin_right);

    // Y extent
    let y_max = series
        .iter()
        .flat_map(|s| (0..n).filter_map(|i| s.get(i).map(|v| v[1])))
        .fold(0.0f64, f64::max);
    let y_scale = LinearScale::new()
        .domain(0.0, y_max)
        .range(height - margin_bottom, margin_top);

    let curve = Curve::monotone_x();

    // Generate area paths for each series
    let area_paths: Vec<(String, String)> = series
        .iter()
        .map(|s| {
            // Top line (y1)
            let top_points: Vec<crate::shape::path::Point> = (0..n)
                .map(|i| {
                    let v = s.get(i).unwrap_or([0.0, 0.0]);
                    crate::shape::path::Point::new(x_scale.scale(i as f64), y_scale.scale(v[1]))
                })
                .collect();
            // Bottom line (y0), reversed
            let bot_points: Vec<crate::shape::path::Point> = (0..n)
                .rev()
                .map(|i| {
                    let v = s.get(i).unwrap_or([0.0, 0.0]);
                    crate::shape::path::Point::new(x_scale.scale(i as f64), y_scale.scale(v[0]))
                })
                .collect();

            let top_interp = curve.interpolate(&top_points);
            let bot_interp = curve.interpolate(&bot_points);

            let mut builder = PathBuilder::new();
            for (i, p) in top_interp.iter().enumerate() {
                if i == 0 {
                    builder = builder.move_to(p.x, p.y);
                } else {
                    builder = builder.line_to(p.x, p.y);
                }
            }
            for p in &bot_interp {
                builder = builder.line_to(p.x, p.y);
            }
            builder = builder.close_path();

            (s.key.clone(), builder.build().to_svg_string())
        })
        .collect();

    StackedAreaResult {
        width,
        height,
        categories: categories.to_vec(),
        series,
        area_paths,
        y_domain: [0.0, y_max],
    }
}
