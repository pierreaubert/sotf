//! Voronoi Stippling — <https://observablehq.com/@mbostock/voronoi-stippling>
//!
//! Weighted Lloyd's relaxation: iteratively moves points toward the
//! density-weighted centroids of their Voronoi cells, producing a
//! stipple pattern that approximates an image's tonal values.

use crate::delaunay::Delaunay;
use crate::shape::path::{Path, PathBuilder};

#[derive(Debug)]
pub struct StipplingResult {
    pub width: f64,
    pub height: f64,
    pub points: Vec<(f64, f64)>,
    pub dot_paths: Vec<Path>,
    pub point_count: usize,
    pub iterations: usize,
}

/// Run weighted Voronoi stippling on a density map.
///
/// `density`: row-major grayscale density values in [0, 1] where 1 = black (place dots).
/// `width`, `height`: dimensions of the density grid.
/// `n`: number of stipple points.
/// `iterations`: number of Lloyd relaxation iterations.
pub fn compute(
    density: &[f64],
    width: usize,
    height: usize,
    n: usize,
    iterations: usize,
) -> StipplingResult {
    let w = width as f64;
    let h = height as f64;

    // Seed points via rejection sampling (deterministic using sine-based PRNG)
    let mut points = vec![0.0f64; n * 2];
    let mut seed = 42u64;
    let mut next_rand = || -> f64 {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        (seed >> 33) as f64 / (1u64 << 31) as f64
    };

    for i in 0..n {
        for _ in 0..30 {
            let x = (next_rand() * w).floor();
            let y = (next_rand() * h).floor();
            points[i * 2] = x;
            points[i * 2 + 1] = y;
            let ix = x as usize;
            let iy = y as usize;
            if ix < width && iy < height && next_rand() < density[iy * width + ix] {
                break;
            }
        }
    }

    // Lloyd's relaxation
    let mut c = vec![0.0f64; n * 2]; // weighted centroid accumulators
    let mut s = vec![0.0f64; n]; // weight sums

    for k in 0..iterations {
        // Build Delaunay from current points
        let flat_points = points.clone();
        let delaunay = Delaunay::new(&flat_to_tuples(&flat_points));
        let _voronoi = delaunay.voronoi(Some([0.0, 0.0, w, h]));

        // Accumulate weighted centroids by scanning every pixel
        c.fill(0.0);
        s.fill(0.0);

        let mut last_found = 0usize;
        for y in 0..height {
            for x in 0..width {
                let weight = density[y * width + x];
                if weight < 0.001 {
                    continue; // skip white pixels
                }
                let px = x as f64 + 0.5;
                let py = y as f64 + 0.5;
                // Find which Voronoi cell this pixel belongs to
                if let Some(i) = delaunay.find(px, py, Some(last_found)) {
                    last_found = i;
                    if i < n {
                        s[i] += weight;
                        c[i * 2] += weight * px;
                        c[i * 2 + 1] += weight * py;
                    }
                }
            }
        }

        // Move points toward weighted centroids
        let jitter = (k as f64 + 1.0).powf(-0.8) * 10.0;
        for i in 0..n {
            let x0 = points[i * 2];
            let y0 = points[i * 2 + 1];
            let (x1, y1) = if s[i] > 0.0 {
                (c[i * 2] / s[i], c[i * 2 + 1] / s[i])
            } else {
                (x0, y0)
            };
            // Overrelaxation factor 1.8 + diminishing jitter
            let jx = (next_rand() - 0.5) * jitter;
            let jy = (next_rand() - 0.5) * jitter;
            points[i * 2] = (x0 + (x1 - x0) * 1.8 + jx).clamp(0.0, w - 1.0);
            points[i * 2 + 1] = (y0 + (y1 - y0) * 1.8 + jy).clamp(0.0, h - 1.0);
        }
    }

    // Build dot paths (small circles)
    let dot_r = 1.5;
    let n_sides = 8;
    let dot_paths: Vec<Path> = (0..n)
        .map(|i| {
            let px = points[i * 2];
            let py = points[i * 2 + 1];
            let mut builder = PathBuilder::new();
            for v in 0..n_sides {
                let angle = std::f64::consts::TAU * v as f64 / n_sides as f64;
                let x = px + dot_r * angle.cos();
                let y = py + dot_r * angle.sin();
                if v == 0 {
                    builder = builder.move_to(x, y);
                } else {
                    builder = builder.line_to(x, y);
                }
            }
            builder = builder.close_path();
            builder.build()
        })
        .collect();

    let final_points: Vec<(f64, f64)> = (0..n).map(|i| (points[i * 2], points[i * 2 + 1])).collect();

    StipplingResult {
        width: w,
        height: h,
        points: final_points,
        dot_paths,
        point_count: n,
        iterations,
    }
}

fn flat_to_tuples(flat: &[f64]) -> Vec<(f64, f64)> {
    (0..flat.len() / 2)
        .map(|i| (flat[i * 2], flat[i * 2 + 1]))
        .collect()
}
