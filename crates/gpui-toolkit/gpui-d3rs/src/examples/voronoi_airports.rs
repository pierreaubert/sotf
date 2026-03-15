//! World Airports Voronoi — <https://observablehq.com/@d3/world-airports-voronoi>
//!
//! Demonstrates: `Delaunay` triangulation + `Voronoi` diagram on projected
//! geographic coordinates. Uses airports.csv (lon, lat).

use crate::delaunay::Delaunay;
use crate::shape::path::{Path, PathBuilder};

#[derive(Debug, Clone)]
pub struct AirportPoint {
    pub lon: f64,
    pub lat: f64,
    pub px: f64, // projected x
    pub py: f64, // projected y
}

#[derive(Debug)]
pub struct VoronoiAirportsResult {
    pub width: f64,
    pub height: f64,
    pub points: Vec<AirportPoint>,
    pub voronoi_paths: Vec<Path>,
    pub point_count: usize,
}

/// Parse airports CSV: lon,lat (no header).
pub fn load_csv(csv_str: &str) -> Vec<(f64, f64)> {
    csv_str
        .lines()
        .filter_map(|line| {
            let cols: Vec<&str> = line.split(',').collect();
            if cols.len() < 2 {
                return None;
            }
            let lon: f64 = cols[0].trim().parse().ok()?;
            let lat: f64 = cols[1].trim().parse().ok()?;
            // Filter to valid ranges
            if !(-180.0..=180.0).contains(&lon) || !(-90.0..=90.0).contains(&lat) {
                return None;
            }
            Some((lon, lat))
        })
        .collect()
}

/// Simple equirectangular projection: lon → x, lat → y.
fn project(lon: f64, lat: f64, width: f64, height: f64) -> (f64, f64) {
    let x = (lon + 180.0) / 360.0 * width;
    let y = (90.0 - lat) / 180.0 * height;
    (x, y)
}

/// Compute Voronoi diagram of projected airport locations.
pub fn compute(coords: &[(f64, f64)]) -> VoronoiAirportsResult {
    let width: f64 = 928.0;
    let height: f64 = 500.0;

    // Project all points
    let points: Vec<AirportPoint> = coords
        .iter()
        .map(|&(lon, lat)| {
            let (px, py) = project(lon, lat, width, height);
            AirportPoint { lon, lat, px, py }
        })
        .collect();

    let projected: Vec<(f64, f64)> = points.iter().map(|p| (p.px, p.py)).collect();

    // Build Delaunay triangulation and Voronoi diagram
    let delaunay = Delaunay::new(&projected);
    let voronoi = delaunay.voronoi(Some([0.0, 0.0, width, height]));

    // Generate Voronoi cell paths
    let voronoi_paths: Vec<Path> = (0..points.len())
        .filter_map(|i| {
            let cell = voronoi.cell_polygon(i)?;
            if cell.is_empty() {
                return None;
            }
            let mut builder = PathBuilder::new();
            for (vi, &(x, y)) in cell.iter().enumerate() {
                if vi == 0 {
                    builder = builder.move_to(x, y);
                } else {
                    builder = builder.line_to(x, y);
                }
            }
            builder = builder.close_path();
            Some(builder.build())
        })
        .collect();

    let point_count = points.len();
    VoronoiAirportsResult {
        width,
        height,
        points,
        voronoi_paths,
        point_count,
    }
}
