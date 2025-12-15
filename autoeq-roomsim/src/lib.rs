//! WASM bindings for Room Acoustics Simulator
//!
//! This module provides WebAssembly bindings to run room acoustics
//! simulations in the browser. It supports:
//! - Rectangular and L-shaped room geometries
//! - Multiple sources with directivity and crossover filters
//! - Multiple solver methods:
//!   - Direct field computation (free-field propagation)
//!   - Image Source Method (ISM) with 1st, 2nd, 3rd order reflections
//!   - Modal analysis for low frequencies
//!   - Hybrid modal/ISM with Schroeder frequency crossover
//!   - BEM (Boundary Element Method) for accurate wave-based simulation
//! - Frequency response and spatial slice visualization
//! - Impulse response generation
//! - Binaural rendering
//!
//! The BEM solver uses pure Rust linear algebra (no BLAS) for WASM compatibility.

use ndarray::Array2;
use num_complex::Complex64;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
use wasm_bindgen::prelude::*;

// Re-export thread pool initialization for WASM
pub use wasm_bindgen_rayon::init_thread_pool;

// BEM solver from math-bem (with pure Rust fallbacks for WASM)
mod bem_solver;
mod scattering_objects;

// Re-export BEM types for external use
pub use bem_solver::{BemAssemblyMethod, BemConfig, BemResult, BemSolverMethod, FmmConfig};
pub use scattering_objects::{BoxObject, CylinderObject, ScatteringObjectConfig, SphereObject};

// ============================================================================
// Wall Materials and Absorption Coefficients
// ============================================================================

/// Standard octave band center frequencies for absorption coefficients (Hz)
pub const ABSORPTION_FREQUENCIES: [f64; 6] = [125.0, 250.0, 500.0, 1000.0, 2000.0, 4000.0];

/// Wall material with frequency-dependent absorption coefficients
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallMaterial {
    pub name: String,
    /// Absorption coefficients at 125, 250, 500, 1000, 2000, 4000 Hz
    pub absorption: [f64; 6],
}

impl WallMaterial {
    /// Create a new wall material with given absorption coefficients
    pub fn new(name: &str, absorption: [f64; 6]) -> Self {
        Self {
            name: name.to_string(),
            absorption,
        }
    }

    /// Get absorption coefficient at a given frequency (interpolated)
    pub fn absorption_at_frequency(&self, frequency: f64) -> f64 {
        // Clamp to frequency range
        if frequency <= ABSORPTION_FREQUENCIES[0] {
            return self.absorption[0];
        }
        if frequency >= ABSORPTION_FREQUENCIES[5] {
            return self.absorption[5];
        }

        // Find bracketing frequencies and interpolate logarithmically
        for i in 0..5 {
            if frequency >= ABSORPTION_FREQUENCIES[i] && frequency < ABSORPTION_FREQUENCIES[i + 1] {
                let log_f = frequency.ln();
                let log_f1 = ABSORPTION_FREQUENCIES[i].ln();
                let log_f2 = ABSORPTION_FREQUENCIES[i + 1].ln();
                let t = (log_f - log_f1) / (log_f2 - log_f1);
                return self.absorption[i] * (1.0 - t) + self.absorption[i + 1] * t;
            }
        }

        self.absorption[5]
    }

    /// Convert absorption coefficient to reflection coefficient: R = sqrt(1 - α)
    pub fn reflection_at_frequency(&self, frequency: f64) -> f64 {
        let alpha = self.absorption_at_frequency(frequency).clamp(0.0, 0.99);
        (1.0 - alpha).sqrt()
    }

    // ========== Material Presets ==========

    /// Concrete or brick (painted)
    pub fn concrete() -> Self {
        Self::new("Concrete", [0.01, 0.01, 0.02, 0.02, 0.02, 0.03])
    }

    /// Unpainted brick
    pub fn brick() -> Self {
        Self::new("Brick", [0.03, 0.03, 0.03, 0.04, 0.05, 0.07])
    }

    /// Gypsum board (drywall) on studs
    pub fn drywall() -> Self {
        Self::new("Drywall", [0.29, 0.10, 0.05, 0.04, 0.07, 0.09])
    }

    /// Plaster on brick
    pub fn plaster() -> Self {
        Self::new("Plaster", [0.01, 0.02, 0.02, 0.03, 0.04, 0.05])
    }

    /// Glass (large pane)
    pub fn glass() -> Self {
        Self::new("Glass", [0.18, 0.06, 0.04, 0.03, 0.02, 0.02])
    }

    /// Wood paneling (thin)
    pub fn wood_thin() -> Self {
        Self::new("Wood (thin)", [0.42, 0.21, 0.10, 0.08, 0.06, 0.06])
    }

    /// Heavy wooden door
    pub fn wood_thick() -> Self {
        Self::new("Wood (thick)", [0.14, 0.10, 0.06, 0.08, 0.10, 0.10])
    }

    /// Carpet on concrete
    pub fn carpet_thin() -> Self {
        Self::new("Carpet (thin)", [0.02, 0.06, 0.14, 0.37, 0.60, 0.65])
    }

    /// Heavy carpet on underlay
    pub fn carpet_thick() -> Self {
        Self::new("Carpet (thick)", [0.08, 0.24, 0.57, 0.69, 0.71, 0.73])
    }

    /// Acoustic ceiling tiles
    pub fn acoustic_tile() -> Self {
        Self::new("Acoustic Tile", [0.50, 0.70, 0.60, 0.70, 0.70, 0.50])
    }

    /// Curtains (medium weight, draped)
    pub fn curtains() -> Self {
        Self::new("Curtains", [0.07, 0.31, 0.49, 0.75, 0.70, 0.60])
    }

    /// Acoustic foam panels
    pub fn acoustic_foam() -> Self {
        Self::new("Acoustic Foam", [0.08, 0.25, 0.60, 0.90, 0.95, 0.90])
    }

    /// Hardwood floor
    pub fn hardwood() -> Self {
        Self::new("Hardwood", [0.15, 0.11, 0.10, 0.07, 0.06, 0.07])
    }

    /// Concrete floor (sealed)
    pub fn concrete_floor() -> Self {
        Self::new("Concrete Floor", [0.01, 0.01, 0.02, 0.02, 0.02, 0.02])
    }

    /// Default material (moderate absorption, like plaster walls)
    #[allow(clippy::should_implement_trait)]
    pub fn default() -> Self {
        Self::plaster()
    }
}

/// Wall surfaces in a rectangular room
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WallSurface {
    /// Left wall (x = 0)
    Left,
    /// Right wall (x = width)
    Right,
    /// Front wall (y = 0)
    Front,
    /// Back wall (y = depth)
    Back,
    /// Floor (z = 0)
    Floor,
    /// Ceiling (z = height)
    Ceiling,
}

impl WallSurface {
    pub fn all() -> [WallSurface; 6] {
        [
            WallSurface::Left,
            WallSurface::Right,
            WallSurface::Front,
            WallSurface::Back,
            WallSurface::Floor,
            WallSurface::Ceiling,
        ]
    }
}

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = console)]
    fn log(s: &str);
}

macro_rules! console_log {
    ($($t:tt)*) => (log(&format_args!($($t)*).to_string()))
}

/// Initialize panic hook for better error messages in browser console
#[wasm_bindgen]
pub fn init_panic_hook() {
    console_error_panic_hook::set_once();
}

// ============================================================================
// Room Geometry Types (local implementation for WASM)
// ============================================================================

/// 3D point in space
#[derive(Debug, Clone, Copy)]
pub struct Point3D {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl Point3D {
    pub fn new(x: f64, y: f64, z: f64) -> Self {
        Self { x, y, z }
    }

    pub fn distance_to(&self, other: &Point3D) -> f64 {
        let dx = self.x - other.x;
        let dy = self.y - other.y;
        let dz = self.z - other.z;
        (dx * dx + dy * dy + dz * dz).sqrt()
    }
}

/// Rectangular room
pub struct RectangularRoom {
    pub width: f64,
    pub depth: f64,
    pub height: f64,
}

impl RectangularRoom {
    pub fn new(width: f64, depth: f64, height: f64) -> Self {
        Self {
            width,
            depth,
            height,
        }
    }

    pub fn get_edges(&self) -> Vec<(Point3D, Point3D)> {
        vec![
            // Floor edges
            (
                Point3D::new(0.0, 0.0, 0.0),
                Point3D::new(self.width, 0.0, 0.0),
            ),
            (
                Point3D::new(self.width, 0.0, 0.0),
                Point3D::new(self.width, self.depth, 0.0),
            ),
            (
                Point3D::new(self.width, self.depth, 0.0),
                Point3D::new(0.0, self.depth, 0.0),
            ),
            (
                Point3D::new(0.0, self.depth, 0.0),
                Point3D::new(0.0, 0.0, 0.0),
            ),
            // Ceiling edges
            (
                Point3D::new(0.0, 0.0, self.height),
                Point3D::new(self.width, 0.0, self.height),
            ),
            (
                Point3D::new(self.width, 0.0, self.height),
                Point3D::new(self.width, self.depth, self.height),
            ),
            (
                Point3D::new(self.width, self.depth, self.height),
                Point3D::new(0.0, self.depth, self.height),
            ),
            (
                Point3D::new(0.0, self.depth, self.height),
                Point3D::new(0.0, 0.0, self.height),
            ),
            // Vertical edges
            (
                Point3D::new(0.0, 0.0, 0.0),
                Point3D::new(0.0, 0.0, self.height),
            ),
            (
                Point3D::new(self.width, 0.0, 0.0),
                Point3D::new(self.width, 0.0, self.height),
            ),
            (
                Point3D::new(self.width, self.depth, 0.0),
                Point3D::new(self.width, self.depth, self.height),
            ),
            (
                Point3D::new(0.0, self.depth, 0.0),
                Point3D::new(0.0, self.depth, self.height),
            ),
        ]
    }
}

/// L-shaped room
pub struct LShapedRoom {
    pub width1: f64,
    pub depth1: f64,
    pub width2: f64,
    pub depth2: f64,
    pub height: f64,
}

impl LShapedRoom {
    pub fn new(width1: f64, depth1: f64, width2: f64, depth2: f64, height: f64) -> Self {
        Self {
            width1,
            depth1,
            width2,
            depth2,
            height,
        }
    }

    /// Check if a 2D point (x, y) is inside the L-shaped room footprint
    /// The L-shape consists of two rectangular regions:
    /// - Region 1: x in [0, width1], y in [0, depth1]
    /// - Region 2: x in [0, width2], y in [depth1, depth1 + depth2]
    pub fn contains_xy(&self, x: f64, y: f64) -> bool {
        let total_depth = self.depth1 + self.depth2;

        // Check region 1 (front section, wider)
        if x >= 0.0 && x <= self.width1 && y >= 0.0 && y <= self.depth1 {
            return true;
        }

        // Check region 2 (back section, narrower)
        if x >= 0.0 && x <= self.width2 && y >= self.depth1 && y <= total_depth {
            return true;
        }

        false
    }

    /// Check if a 3D point is inside the L-shaped room
    pub fn contains(&self, p: &Point3D) -> bool {
        self.contains_xy(p.x, p.y) && p.z >= 0.0 && p.z <= self.height
    }

    /// Validate if an image source produces a valid reflection path
    /// The reflection path from source to listener via the image source must:
    /// 1. Cross the intended wall
    /// 2. Not pass through any internal walls or outside the room
    pub fn is_valid_image_source(
        &self,
        image: &Point3D,
        listener: &Point3D,
        _source: &Point3D,
    ) -> bool {
        // The image source should be outside the room (on the other side of the wall)
        // For an L-shaped room, we also need to check that the path doesn't pass
        // through the internal corner or outside the room

        // Find the midpoint (approximate reflection point)
        let _midpoint = Point3D::new(
            (image.x + listener.x) / 2.0,
            (image.y + listener.y) / 2.0,
            (image.z + listener.z) / 2.0,
        );

        // The reflection point should be on or near a wall
        // For simplicity, check if the midpoint is approximately on the room boundary
        // This is a heuristic - for more accuracy, we'd need to intersect the line
        // with each wall plane

        // Check a few points along the path from listener to image
        let segments = 10;
        for i in 1..segments {
            let t = i as f64 / segments as f64;
            let test_point = Point3D::new(
                listener.x + t * (image.x - listener.x),
                listener.y + t * (image.y - listener.y),
                listener.z + t * (image.z - listener.z),
            );

            // If the test point is outside the room but not the image source position,
            // this could be an invalid path
            if !self.contains(&test_point) {
                // Allow the path to go outside only at the end (where image source is)
                if i < segments - 1 {
                    // Check if this is a legitimate boundary crossing
                    // The path should cross only one wall
                    let prev_t = (i - 1) as f64 / segments as f64;
                    let prev_point = Point3D::new(
                        listener.x + prev_t * (image.x - listener.x),
                        listener.y + prev_t * (image.y - listener.y),
                        listener.z + prev_t * (image.z - listener.z),
                    );

                    if self.contains(&prev_point) {
                        // This is the crossing point - valid so far
                        continue;
                    } else {
                        // Path was already outside - might be going through corner
                        // This needs more sophisticated checking
                    }
                }
            }
        }

        // For now, use a simpler heuristic:
        // Reject image sources that would require reflection off internal corners
        let total_depth = self.depth1 + self.depth2;

        // Check if image source is in the "forbidden" region
        // The forbidden region is where width1 < x < infinity AND depth1 < y < total_depth
        // This is the area cut out of the L-shape
        if image.x > self.width2 && image.y > self.depth1 && image.y < total_depth {
            return false;
        }

        true
    }

    /// Generate first-order image sources for L-shaped room
    /// L-shaped room has 8 walls: 6 exterior + 2 interior (at the step)
    ///
    /// Walls:
    /// - Left: x = 0 (full depth)
    /// - Right (section 1): x = width1, y in [0, depth1]
    /// - Right (section 2): x = width2, y in [depth1, total_depth]
    /// - Front: y = 0, x in [0, width1]
    /// - Back: y = total_depth, x in [0, width2]
    /// - Floor: z = 0
    /// - Ceiling: z = height
    /// - Interior wall 1 (step): x in [width2, width1], y = depth1
    /// - Interior wall 2 (step): y in [depth1, depth1], x = width2
    ///
    /// Returns: Vec of (image_position, wall_name)
    pub fn get_first_order_images(&self, source: &Point3D) -> Vec<(Point3D, &'static str)> {
        let total_depth = self.depth1 + self.depth2;
        let mut images = Vec::new();

        // Left wall (x = 0) - always present for full depth
        images.push((Point3D::new(-source.x, source.y, source.z), "left"));

        // Right wall handling depends on source y position
        if source.y <= self.depth1 {
            // Source is in section 1 (front), reflect off x = width1
            images.push((
                Point3D::new(2.0 * self.width1 - source.x, source.y, source.z),
                "right",
            ));
        } else {
            // Source is in section 2 (back), reflect off x = width2
            images.push((
                Point3D::new(2.0 * self.width2 - source.x, source.y, source.z),
                "right",
            ));
        }

        // Front wall (y = 0)
        images.push((Point3D::new(source.x, -source.y, source.z), "front"));

        // Back wall (y = total_depth)
        images.push((
            Point3D::new(source.x, 2.0 * total_depth - source.y, source.z),
            "back",
        ));

        // Floor (z = 0)
        images.push((Point3D::new(source.x, source.y, -source.z), "floor"));

        // Ceiling (z = height)
        images.push((
            Point3D::new(source.x, source.y, 2.0 * self.height - source.z),
            "ceiling",
        ));

        // Interior walls (the step) - L-shape has two interior walls:
        // 1. Horizontal interior wall: y = depth1, x in [width2, width1]
        // 2. Vertical interior wall: x = width2, y in [depth1, total_depth] (but only visible from section 1)

        // Horizontal interior wall (y = depth1)
        // Visible to sources in section 1 (y < depth1) when x > width2
        if source.y < self.depth1 && source.x > self.width2 {
            // Source is in the wider front section, can see the horizontal step
            images.push((
                Point3D::new(source.x, 2.0 * self.depth1 - source.y, source.z),
                "step_horizontal",
            ));
        }

        // Vertical interior wall (x = width2)
        // Visible to sources in section 1 (y < depth1) when looking toward section 2
        // The wall runs from y = depth1 to y = total_depth at x = width2
        if source.y < self.depth1 && source.x > self.width2 {
            // Source in overhang area can see vertical interior wall
            images.push((
                Point3D::new(2.0 * self.width2 - source.x, source.y, source.z),
                "step_vertical",
            ));
        }

        // Also check: sources in section 2 looking back at the vertical step
        // (from the narrow back section toward the wider front)
        if source.y > self.depth1 && source.x < self.width2 {
            // Source in section 2 can see vertical interior wall from behind
            // Reflection off x = width2 going toward section 1
            // But this would put the image in the "cut-out" area, which we validate later
            images.push((
                Point3D::new(2.0 * self.width2 - source.x, source.y, source.z),
                "step_vertical",
            ));
        }

        images
    }

    pub fn get_edges(&self) -> Vec<(Point3D, Point3D)> {
        let total_depth = self.depth1 + self.depth2;
        vec![
            // Floor edges - Main section
            (
                Point3D::new(0.0, 0.0, 0.0),
                Point3D::new(self.width1, 0.0, 0.0),
            ),
            (
                Point3D::new(self.width1, 0.0, 0.0),
                Point3D::new(self.width1, self.depth1, 0.0),
            ),
            (
                Point3D::new(self.width1, self.depth1, 0.0),
                Point3D::new(self.width2, self.depth1, 0.0),
            ),
            (
                Point3D::new(self.width2, self.depth1, 0.0),
                Point3D::new(self.width2, total_depth, 0.0),
            ),
            (
                Point3D::new(self.width2, total_depth, 0.0),
                Point3D::new(0.0, total_depth, 0.0),
            ),
            (
                Point3D::new(0.0, total_depth, 0.0),
                Point3D::new(0.0, 0.0, 0.0),
            ),
            // Ceiling edges
            (
                Point3D::new(0.0, 0.0, self.height),
                Point3D::new(self.width1, 0.0, self.height),
            ),
            (
                Point3D::new(self.width1, 0.0, self.height),
                Point3D::new(self.width1, self.depth1, self.height),
            ),
            (
                Point3D::new(self.width1, self.depth1, self.height),
                Point3D::new(self.width2, self.depth1, self.height),
            ),
            (
                Point3D::new(self.width2, self.depth1, self.height),
                Point3D::new(self.width2, total_depth, self.height),
            ),
            (
                Point3D::new(self.width2, total_depth, self.height),
                Point3D::new(0.0, total_depth, self.height),
            ),
            (
                Point3D::new(0.0, total_depth, self.height),
                Point3D::new(0.0, 0.0, self.height),
            ),
            // Vertical edges
            (
                Point3D::new(0.0, 0.0, 0.0),
                Point3D::new(0.0, 0.0, self.height),
            ),
            (
                Point3D::new(self.width1, 0.0, 0.0),
                Point3D::new(self.width1, 0.0, self.height),
            ),
            (
                Point3D::new(self.width1, self.depth1, 0.0),
                Point3D::new(self.width1, self.depth1, self.height),
            ),
            (
                Point3D::new(self.width2, self.depth1, 0.0),
                Point3D::new(self.width2, self.depth1, self.height),
            ),
            (
                Point3D::new(self.width2, total_depth, 0.0),
                Point3D::new(self.width2, total_depth, self.height),
            ),
            (
                Point3D::new(0.0, total_depth, 0.0),
                Point3D::new(0.0, total_depth, self.height),
            ),
        ]
    }
}

/// Room geometry enum
pub enum RoomGeometry {
    Rectangular(RectangularRoom),
    LShaped(LShapedRoom),
}

impl RoomGeometry {
    pub fn get_edges(&self) -> Vec<(Point3D, Point3D)> {
        match self {
            RoomGeometry::Rectangular(r) => r.get_edges(),
            RoomGeometry::LShaped(r) => r.get_edges(),
        }
    }
}

/// Crossover filter
pub enum CrossoverFilter {
    FullRange,
    Lowpass {
        cutoff_freq: f64,
        order: u32,
    },
    Highpass {
        cutoff_freq: f64,
        order: u32,
    },
    Bandpass {
        low_cutoff: f64,
        high_cutoff: f64,
        order: u32,
    },
}

impl CrossoverFilter {
    pub fn amplitude_at_frequency(&self, frequency: f64) -> f64 {
        match self {
            CrossoverFilter::FullRange => 1.0,
            CrossoverFilter::Lowpass { cutoff_freq, order } => {
                let ratio = frequency / cutoff_freq;
                1.0 / (1.0 + ratio.powi(*order as i32 * 2)).sqrt()
            }
            CrossoverFilter::Highpass { cutoff_freq, order } => {
                let ratio = cutoff_freq / frequency;
                1.0 / (1.0 + ratio.powi(*order as i32 * 2)).sqrt()
            }
            CrossoverFilter::Bandpass {
                low_cutoff,
                high_cutoff,
                order,
            } => {
                let hp = 1.0 / (1.0 + (low_cutoff / frequency).powi(*order as i32 * 2)).sqrt();
                let lp = 1.0 / (1.0 + (frequency / high_cutoff).powi(*order as i32 * 2)).sqrt();
                hp * lp
            }
        }
    }
}

/// Directivity pattern
pub struct DirectivityPattern {
    pub horizontal_angles: Vec<f64>,
    pub vertical_angles: Vec<f64>,
    pub magnitude: Array2<f64>,
}

impl DirectivityPattern {
    pub fn omnidirectional() -> Self {
        let horizontal_angles: Vec<f64> = (0..36).map(|i| i as f64 * 10.0).collect();
        let vertical_angles: Vec<f64> = (0..19).map(|i| i as f64 * 10.0).collect();
        let magnitude = Array2::ones((vertical_angles.len(), horizontal_angles.len()));

        Self {
            horizontal_angles,
            vertical_angles,
            magnitude,
        }
    }

    pub fn interpolate(&self, theta: f64, phi: f64) -> f64 {
        let theta_deg = theta.to_degrees();
        let mut phi_deg = phi.to_degrees();

        while phi_deg < 0.0 {
            phi_deg += 360.0;
        }
        while phi_deg >= 360.0 {
            phi_deg -= 360.0;
        }

        let h_idx = (phi_deg / 10.0).floor() as usize;
        let v_idx = (theta_deg / 10.0).floor() as usize;

        let h_idx = h_idx.min(self.horizontal_angles.len() - 1);
        let v_idx = v_idx.min(self.vertical_angles.len() - 1);

        let h_next = (h_idx + 1) % self.horizontal_angles.len();
        let v_next = (v_idx + 1).min(self.vertical_angles.len() - 1);

        let h_frac = (phi_deg / 10.0) - h_idx as f64;
        let v_frac = (theta_deg / 10.0) - v_idx as f64;

        let m00 = self.magnitude[[v_idx, h_idx]];
        let m01 = self.magnitude[[v_idx, h_next]];
        let m10 = self.magnitude[[v_next, h_idx]];
        let m11 = self.magnitude[[v_next, h_next]];

        let m0 = m00 * (1.0 - h_frac) + m01 * h_frac;
        let m1 = m10 * (1.0 - h_frac) + m11 * h_frac;

        m0 * (1.0 - v_frac) + m1 * v_frac
    }
}

/// Sound source
pub struct Source {
    pub position: Point3D,
    pub directivity: DirectivityPattern,
    pub amplitude: f64,
    pub crossover: CrossoverFilter,
    pub name: String,
    /// Time delay in seconds
    pub delay_sec: f64,
    /// Phase inversion flag (true = 180 degree phase shift)
    pub invert_phase: bool,
}

impl Source {
    pub fn new(position: Point3D, directivity: DirectivityPattern, amplitude: f64) -> Self {
        Self {
            position,
            directivity,
            amplitude,
            crossover: CrossoverFilter::FullRange,
            name: String::from("Source"),
            delay_sec: 0.0,
            invert_phase: false,
        }
    }

    pub fn with_crossover(mut self, crossover: CrossoverFilter) -> Self {
        self.crossover = crossover;
        self
    }

    pub fn with_name(mut self, name: String) -> Self {
        self.name = name;
        self
    }

    pub fn with_delay_ms(mut self, delay_ms: f64) -> Self {
        self.delay_sec = delay_ms / 1000.0;
        self
    }

    pub fn with_phase_inversion(mut self, invert: bool) -> Self {
        self.invert_phase = invert;
        self
    }

    /// Get the phase factor for this source at a given frequency
    /// Includes both time delay and phase inversion
    pub fn phase_factor(&self, frequency: f64) -> Complex64 {
        let omega = 2.0 * PI * frequency;
        // Phase shift from delay: e^(-i * omega * delay)
        let delay_phase = Complex64::new(0.0, -omega * self.delay_sec).exp();
        // Phase inversion: multiply by -1
        if self.invert_phase {
            -delay_phase
        } else {
            delay_phase
        }
    }

    pub fn amplitude_towards(&self, point: &Point3D, frequency: f64) -> f64 {
        let dx = point.x - self.position.x;
        let dy = point.y - self.position.y;
        let dz = point.z - self.position.z;

        let r = (dx * dx + dy * dy + dz * dz).sqrt();
        if r < 1e-10 {
            return self.amplitude * self.crossover.amplitude_at_frequency(frequency);
        }

        let theta = (dz / r).acos();
        let phi = dy.atan2(dx);

        let directivity_factor = self.directivity.interpolate(theta, phi);
        let crossover_factor = self.crossover.amplitude_at_frequency(frequency);
        self.amplitude * directivity_factor * crossover_factor
    }
}

// ============================================================================
// Configuration types (JSON-serializable)
// ============================================================================

/// Room geometry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RoomGeometryConfig {
    #[serde(rename = "rectangular")]
    Rectangular { width: f64, depth: f64, height: f64 },
    #[serde(rename = "lshaped")]
    LShaped {
        width1: f64,
        depth1: f64,
        width2: f64,
        depth2: f64,
        height: f64,
    },
}

/// 3D point configuration
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Point3DConfig {
    pub x: f64,
    pub y: f64,
    pub z: f64,
}

impl From<Point3DConfig> for Point3D {
    fn from(p: Point3DConfig) -> Self {
        Point3D::new(p.x, p.y, p.z)
    }
}

impl From<Point3DConfig> for bem::room_acoustics::Point3D {
    fn from(p: Point3DConfig) -> Self {
        bem::room_acoustics::Point3D::new(p.x, p.y, p.z)
    }
}

/// Crossover filter configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CrossoverConfig {
    #[serde(rename = "fullrange")]
    #[default]
    FullRange,
    #[serde(rename = "lowpass")]
    Lowpass { cutoff_freq: f64, order: u32 },
    #[serde(rename = "highpass")]
    Highpass { cutoff_freq: f64, order: u32 },
    #[serde(rename = "bandpass")]
    Bandpass {
        low_cutoff: f64,
        high_cutoff: f64,
        order: u32,
    },
}

impl CrossoverConfig {
    fn to_filter(&self) -> CrossoverFilter {
        match self {
            CrossoverConfig::FullRange => CrossoverFilter::FullRange,
            CrossoverConfig::Lowpass { cutoff_freq, order } => CrossoverFilter::Lowpass {
                cutoff_freq: *cutoff_freq,
                order: *order,
            },
            CrossoverConfig::Highpass { cutoff_freq, order } => CrossoverFilter::Highpass {
                cutoff_freq: *cutoff_freq,
                order: *order,
            },
            CrossoverConfig::Bandpass {
                low_cutoff,
                high_cutoff,
                order,
            } => CrossoverFilter::Bandpass {
                low_cutoff: *low_cutoff,
                high_cutoff: *high_cutoff,
                order: *order,
            },
        }
    }
}

/// Directivity pattern configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DirectivityConfig {
    #[serde(rename = "omnidirectional")]
    #[default]
    Omnidirectional,
    #[serde(rename = "cardioid")]
    Cardioid { front_back_ratio: f64 },
    /// Spinorama directivity data with horizontal and vertical SPL curves at different angles
    #[serde(rename = "spinorama")]
    Spinorama {
        /// Horizontal SPL data at different angles
        horizontal: Vec<SpinoramaCurve>,
        /// Vertical SPL data at different angles
        vertical: Vec<SpinoramaCurve>,
    },
}

/// A single SPL curve at a specific angle (for spinorama directivity data)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpinoramaCurve {
    /// Angle in degrees (e.g., -60, -50, ..., 0, ..., 50, 60)
    pub angle: f64,
    /// Frequency points in Hz
    pub freq: Vec<f64>,
    /// SPL values in dB
    pub spl: Vec<f64>,
}

/// Wall material configuration (JSON-serializable)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum WallMaterialConfig {
    /// Use a preset material by name
    #[serde(rename = "preset")]
    Preset { name: String },
    /// Custom absorption coefficients at 125, 250, 500, 1000, 2000, 4000 Hz
    #[serde(rename = "custom")]
    Custom {
        name: String,
        #[serde(default = "default_absorption")]
        absorption: [f64; 6],
    },
}

fn default_absorption() -> [f64; 6] {
    [0.02, 0.02, 0.03, 0.03, 0.04, 0.05] // Default plaster-like
}

impl Default for WallMaterialConfig {
    fn default() -> Self {
        WallMaterialConfig::Preset {
            name: "plaster".to_string(),
        }
    }
}

impl WallMaterialConfig {
    /// Convert config to WallMaterial
    pub fn to_material(&self) -> WallMaterial {
        match self {
            WallMaterialConfig::Preset { name } => {
                match name.to_lowercase().as_str() {
                    "concrete" => WallMaterial::concrete(),
                    "brick" => WallMaterial::brick(),
                    "drywall" | "gypsum" => WallMaterial::drywall(),
                    "plaster" => WallMaterial::plaster(),
                    "glass" => WallMaterial::glass(),
                    "wood_thin" | "wood-thin" | "thin_wood" => WallMaterial::wood_thin(),
                    "wood_thick" | "wood-thick" | "thick_wood" | "wood" => {
                        WallMaterial::wood_thick()
                    }
                    "carpet_thin" | "carpet-thin" | "thin_carpet" => WallMaterial::carpet_thin(),
                    "carpet_thick" | "carpet-thick" | "thick_carpet" | "carpet" => {
                        WallMaterial::carpet_thick()
                    }
                    "acoustic_tile" | "acoustic-tile" | "ceiling_tile" => {
                        WallMaterial::acoustic_tile()
                    }
                    "curtains" | "drapes" => WallMaterial::curtains(),
                    "acoustic_foam" | "acoustic-foam" | "foam" => WallMaterial::acoustic_foam(),
                    "hardwood" | "wood_floor" | "wood-floor" => WallMaterial::hardwood(),
                    "concrete_floor" | "concrete-floor" => WallMaterial::concrete_floor(),
                    _ => WallMaterial::plaster(), // Default fallback
                }
            }
            WallMaterialConfig::Custom { name, absorption } => WallMaterial::new(name, *absorption),
        }
    }

    /// Get absorption coefficient at a given frequency
    pub fn absorption_at_frequency(&self, frequency: f64) -> f64 {
        self.to_material().absorption_at_frequency(frequency)
    }
}

/// Wall materials configuration for all 6 surfaces
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WallMaterialsConfig {
    /// Left wall (x = 0)
    #[serde(default)]
    pub left: WallMaterialConfig,
    /// Right wall (x = width)
    #[serde(default)]
    pub right: WallMaterialConfig,
    /// Front wall (y = 0)
    #[serde(default)]
    pub front: WallMaterialConfig,
    /// Back wall (y = depth)
    #[serde(default)]
    pub back: WallMaterialConfig,
    /// Floor (z = 0)
    #[serde(default = "default_floor_material")]
    pub floor: WallMaterialConfig,
    /// Ceiling (z = height)
    #[serde(default = "default_ceiling_material")]
    pub ceiling: WallMaterialConfig,
}

fn default_floor_material() -> WallMaterialConfig {
    WallMaterialConfig::Preset {
        name: "hardwood".to_string(),
    }
}

fn default_ceiling_material() -> WallMaterialConfig {
    WallMaterialConfig::Preset {
        name: "plaster".to_string(),
    }
}

impl Default for WallMaterialsConfig {
    fn default() -> Self {
        Self {
            left: WallMaterialConfig::default(),
            right: WallMaterialConfig::default(),
            front: WallMaterialConfig::default(),
            back: WallMaterialConfig::default(),
            floor: default_floor_material(),
            ceiling: default_ceiling_material(),
        }
    }
}

impl WallMaterialsConfig {
    /// Get the material for a specific wall surface
    pub fn get_material(&self, surface: WallSurface) -> WallMaterial {
        match surface {
            WallSurface::Left => self.left.to_material(),
            WallSurface::Right => self.right.to_material(),
            WallSurface::Front => self.front.to_material(),
            WallSurface::Back => self.back.to_material(),
            WallSurface::Floor => self.floor.to_material(),
            WallSurface::Ceiling => self.ceiling.to_material(),
        }
    }

    /// Get reflection coefficient for a wall at a given frequency
    pub fn reflection_at(&self, surface: WallSurface, frequency: f64) -> f64 {
        self.get_material(surface)
            .reflection_at_frequency(frequency)
    }

    /// Get average absorption coefficient across all walls at a given frequency
    pub fn average_absorption_at(&self, frequency: f64) -> f64 {
        let surfaces = WallSurface::all();
        let total: f64 = surfaces
            .iter()
            .map(|&s| self.get_material(s).absorption_at_frequency(frequency))
            .sum();
        total / surfaces.len() as f64
    }
}

/// Source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfig {
    pub name: String,
    pub position: Point3DConfig,
    #[serde(default = "default_amplitude")]
    pub amplitude: f64,
    #[serde(default)]
    pub directivity: DirectivityConfig,
    #[serde(default)]
    pub crossover: CrossoverConfig,
    /// Time delay in milliseconds (for driver alignment)
    #[serde(default)]
    pub delay_ms: f64,
    /// Phase inversion (180 degree phase shift)
    #[serde(default)]
    pub invert_phase: bool,
}

fn default_amplitude() -> f64 {
    1.0
}

/// Frequency range configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrequencyConfig {
    pub min_freq: f64,
    pub max_freq: f64,
    pub num_points: usize,
    #[serde(default = "default_spacing")]
    pub spacing: String,
}

fn default_spacing() -> String {
    "logarithmic".to_string()
}

/// Solver configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverConfig {
    /// Solver method: "direct", "image-source-1/2/3", "modal", "hybrid", "bem", or "hybrid-bem"
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default = "default_mesh_resolution")]
    pub mesh_resolution: usize,
    #[serde(default = "default_speed_of_sound")]
    pub speed_of_sound: f64,
    /// Temperature in Celsius (affects speed of sound and air absorption)
    #[serde(default = "default_temperature")]
    pub temperature: f64,
    /// Relative humidity in percent (0-100, affects air absorption)
    #[serde(default = "default_humidity")]
    pub humidity: f64,
    /// Enable air absorption modeling
    #[serde(default = "default_air_absorption")]
    pub air_absorption: bool,
    /// Enable edge diffraction modeling (adds diffraction contributions at room corners)
    #[serde(default = "default_edge_diffraction")]
    pub edge_diffraction: bool,
    /// Crossover width for hybrid solver (in octaves, default 0.5)
    #[serde(default = "default_hybrid_crossover_width")]
    pub hybrid_crossover_width: f64,
    /// Maximum mode order for modal analysis (default 20)
    #[serde(default = "default_max_mode_order")]
    pub max_mode_order: u32,
    /// Modal damping factor (Q factor, default 50)
    #[serde(default = "default_modal_damping")]
    pub modal_damping: f64,
    /// BEM solver configuration (used when method is "bem" or "hybrid-bem")
    #[serde(default)]
    pub bem_config: BemConfig,
}

fn default_method() -> String {
    "direct".to_string()
}
fn default_mesh_resolution() -> usize {
    2
}
fn default_speed_of_sound() -> f64 {
    343.0
}
fn default_temperature() -> f64 {
    20.0
} // 20°C
fn default_humidity() -> f64 {
    50.0
} // 50% relative humidity
fn default_air_absorption() -> bool {
    true
}
fn default_edge_diffraction() -> bool {
    false
} // Disabled by default (computationally expensive)
fn default_hybrid_crossover_width() -> f64 {
    0.5
} // 0.5 octaves
fn default_max_mode_order() -> u32 {
    20
}
fn default_modal_damping() -> f64 {
    10.0
} // Q factor (typical for residential rooms)

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            method: default_method(),
            mesh_resolution: default_mesh_resolution(),
            speed_of_sound: default_speed_of_sound(),
            temperature: default_temperature(),
            humidity: default_humidity(),
            air_absorption: default_air_absorption(),
            edge_diffraction: default_edge_diffraction(),
            hybrid_crossover_width: default_hybrid_crossover_width(),
            max_mode_order: default_max_mode_order(),
            modal_damping: default_modal_damping(),
            bem_config: BemConfig::default(),
        }
    }
}

/// Calculate air absorption coefficient (Np/m) using ISO 9613-1 approximation
///
/// This is a simplified model based on ISO 9613-1 that provides reasonable
/// accuracy for typical room acoustics conditions (15-30°C, 20-80% RH).
///
/// Returns the absorption coefficient in Nepers per meter (Np/m).
/// To convert to dB/m: multiply by 8.686
///
/// # Arguments
/// * `frequency` - Sound frequency in Hz
/// * `temperature` - Temperature in Celsius
/// * `humidity` - Relative humidity in percent (0-100)
///
/// # Reference
/// ISO 9613-1:1993 - Acoustics -- Attenuation of sound during propagation outdoors
pub fn calculate_air_absorption(frequency: f64, temperature: f64, humidity: f64) -> f64 {
    // Simplified air absorption model based on empirical data (Np/m)
    // This provides reasonable accuracy for typical room acoustics conditions.
    //
    // Reference values at 20°C, 50% RH:
    // ~0.0001 Np/m at 500 Hz
    // ~0.001 Np/m at 1 kHz
    // ~0.004 Np/m at 4 kHz
    // ~0.010 Np/m at 8 kHz
    //
    // For more accurate results, implement full ISO 9613-1 calculation.

    let base_absorption = match frequency {
        f if f < 500.0 => 0.0001 * (f / 500.0).powi(2),
        f if f < 2000.0 => 0.0001 + 0.0009 * ((f - 500.0) / 1500.0),
        f if f < 8000.0 => 0.001 + 0.009 * ((f - 2000.0) / 6000.0),
        _ => 0.01 + 0.005 * ((frequency - 8000.0) / 8000.0),
    };

    // Temperature correction (absorption increases ~2%/°C deviation from 20°C)
    let temp_factor = 1.0 + 0.02 * (temperature - 20.0).abs();

    // Humidity correction (absorption decreases with humidity up to ~40%, then increases)
    let humidity_factor = if humidity < 40.0 {
        1.0 + 0.01 * (40.0 - humidity)
    } else {
        1.0 + 0.005 * (humidity - 40.0)
    };

    base_absorption * temp_factor * humidity_factor
}

/// Visualization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationConfig {
    #[serde(default = "default_generate_slices")]
    pub generate_slices: bool,
    #[serde(default = "default_slice_resolution")]
    pub slice_resolution: usize,
    #[serde(default)]
    pub slice_frequency_indices: Vec<usize>,
    /// Generate impulse response output
    #[serde(default = "default_generate_impulse_response")]
    pub generate_impulse_response: bool,
    /// Impulse response configuration
    #[serde(default)]
    pub impulse_response: ImpulseResponseConfig,
    /// Binaural rendering configuration
    #[serde(default)]
    pub binaural: BinauralConfig,
}

fn default_generate_slices() -> bool {
    true
}
fn default_slice_resolution() -> usize {
    50
}
fn default_generate_impulse_response() -> bool {
    false
}

impl Default for VisualizationConfig {
    fn default() -> Self {
        Self {
            generate_slices: true,
            slice_resolution: 50,
            slice_frequency_indices: Vec::new(),
            generate_impulse_response: false,
            impulse_response: ImpulseResponseConfig::default(),
            binaural: BinauralConfig::default(),
        }
    }
}

/// Complete simulation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulationConfig {
    pub room: RoomGeometryConfig,
    pub sources: Vec<SourceConfig>,
    pub listening_positions: Vec<Point3DConfig>,
    pub frequencies: FrequencyConfig,
    #[serde(default)]
    pub solver: SolverConfig,
    #[serde(default)]
    pub visualization: VisualizationConfig,
    #[serde(default)]
    pub wall_materials: WallMaterialsConfig,
    #[serde(default)]
    pub metadata: MetadataConfig,
    /// Scattering objects inside the room (furniture, equipment, etc.)
    /// Only used with BEM solver methods ("bem" or "hybrid-bem")
    #[serde(default)]
    pub scattering_objects: Vec<ScatteringObjectConfig>,
}

/// Simulation metadata
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct MetadataConfig {
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub author: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub notes: String,
}

// ============================================================================
// Output types
// ============================================================================

#[derive(Debug, Clone, Serialize)]
pub struct SourceResponse {
    pub source_name: String,
    pub spl: Vec<f64>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SliceOutput {
    pub frequency: f64,
    pub x: Vec<f64>,
    pub y: Vec<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub z: Option<Vec<f64>>,
    pub spl: Vec<f64>,
    pub shape: [usize; 2],
}

#[derive(Debug, Clone, Serialize)]
pub struct RoomOutput {
    pub width: f64,
    pub depth: f64,
    pub height: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_type: Option<String>,
    pub edges: Vec<[[f64; 3]; 2]>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SimulationResults {
    pub room: RoomOutput,
    pub sources: Vec<SourceOutputInfo>,
    pub listening_position: [f64; 3],
    pub frequencies: Vec<f64>,
    pub frequency_response: Vec<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source_responses: Option<Vec<SourceResponse>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub horizontal_slices: Option<Vec<SliceOutput>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vertical_slices: Option<Vec<SliceOutput>>,
    pub solver: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mesh_nodes: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mesh_elements: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<MetadataConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_modes: Option<Vec<RoomMode>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub room_acoustics: Option<RoomAcoustics>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub impulse_response: Option<ImpulseResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binaural_response: Option<BinauralResponse>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceOutputInfo {
    pub name: String,
    pub position: [f64; 3],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crossover: Option<String>,
}

// ============================================================================
// Room Modes
// ============================================================================

/// Represents a room resonant mode
#[derive(Debug, Clone, Serialize)]
pub struct RoomMode {
    /// Resonant frequency in Hz
    pub frequency: f64,
    /// Mode indices (n, m, p) for x, y, z dimensions
    pub indices: [u32; 3],
    /// Mode type: "axial", "tangential", or "oblique"
    pub mode_type: String,
    /// Description of the mode (e.g., "1,0,0 - Length mode")
    pub description: String,
}

/// Calculate room modes for a rectangular room
///
/// Room modes occur at frequencies where standing waves form between parallel surfaces.
/// - Axial modes: Standing waves between one pair of parallel walls (n,0,0), (0,m,0), or (0,0,p)
/// - Tangential modes: Standing waves between two pairs of walls (n,m,0), (n,0,p), or (0,m,p)
/// - Oblique modes: Standing waves between all three pairs of walls (n,m,p)
///
/// Formula: f = (c/2) * sqrt((n/Lx)² + (m/Ly)² + (p/Lz)²)
pub fn calculate_room_modes(
    length_x: f64, // Room width (x dimension)
    length_y: f64, // Room depth (y dimension)
    length_z: f64, // Room height (z dimension)
    speed_of_sound: f64,
    max_frequency: f64,
    max_order: u32,
) -> Vec<RoomMode> {
    let mut modes = Vec::new();
    let c = speed_of_sound;

    for n in 0..=max_order {
        for m in 0..=max_order {
            for p in 0..=max_order {
                // Skip the (0,0,0) mode
                if n == 0 && m == 0 && p == 0 {
                    continue;
                }

                // Calculate mode frequency
                let nx = n as f64 / length_x;
                let my = m as f64 / length_y;
                let pz = p as f64 / length_z;
                let freq = (c / 2.0) * (nx * nx + my * my + pz * pz).sqrt();

                if freq > max_frequency {
                    continue;
                }

                // Determine mode type
                let zero_count = [n, m, p].iter().filter(|&&x| x == 0).count();
                let mode_type = match zero_count {
                    2 => "axial",
                    1 => "tangential",
                    0 => "oblique",
                    _ => continue, // Shouldn't happen
                };

                // Generate description
                let description = match (n, m, p) {
                    (n, 0, 0) if n > 0 => format!("{},0,0 - Length mode (X)", n),
                    (0, m, 0) if m > 0 => format!("0,{},0 - Width mode (Y)", m),
                    (0, 0, p) if p > 0 => format!("0,0,{} - Height mode (Z)", p),
                    (n, m, 0) => format!("{},{},0 - Floor tangential", n, m),
                    (n, 0, p) => format!("{},0,{} - Side tangential", n, p),
                    (0, m, p) => format!("0,{},{} - Front tangential", m, p),
                    (n, m, p) => format!("{},{},{} - Oblique", n, m, p),
                };

                modes.push(RoomMode {
                    frequency: freq,
                    indices: [n, m, p],
                    mode_type: mode_type.to_string(),
                    description,
                });
            }
        }
    }

    // Sort by frequency
    modes.sort_by(|a, b| a.frequency.partial_cmp(&b.frequency).unwrap());

    modes
}

/// Calculate modal pressure at a point from room mode superposition
///
/// The pressure field is represented as a sum of standing wave modes:
/// p(x,y,z,f) = Σ A_nmp * cos(n*π*x/Lx) * cos(m*π*y/Ly) * cos(p*π*z/Lz) * H(f, f_nmp)
///
/// Where:
/// - A_nmp is the mode amplitude (depends on source position)
/// - H(f, f_nmp) is the modal transfer function (resonant response)
/// - f_nmp is the mode frequency
///
/// The transfer function H(f, f_nmp) = 1 / (1 - (f/f_nmp)² + j*f/(f_nmp*Q))
/// where Q is the modal damping factor.
///
/// Reference: Kuttruff, "Room Acoustics", Chapter 3 - Modal expansion of room Green's function
///
/// G_modal = (c²/V) Σ [ε * Ψ(rs) * Ψ(r) / (ω² - ωₙ² + j*δₙ*ω)]
///
/// This has units of 1/m, matching the free-field Green's function G = e^(ikr)/(4πr).
#[allow(clippy::too_many_arguments)]
pub fn calculate_modal_pressure(
    source: &Point3D,
    listener: &Point3D,
    frequency: f64,
    room_width: f64,  // Lx
    room_depth: f64,  // Ly
    room_height: f64, // Lz
    speed_of_sound: f64,
    max_mode_order: u32,
    modal_damping: f64, // Q factor
) -> Complex64 {
    let volume = room_width * room_depth * room_height;

    // Direct source-listener distance for phase
    let r = source.distance_to(listener).max(0.1);
    let omega = 2.0 * PI * frequency;
    let omega_sq = omega * omega;
    let k = omega / speed_of_sound;
    let c_sq = speed_of_sound * speed_of_sound;

    // Modal Green's function prefactor: c²/V (units: m²/s² / m³ = m⁻¹ when divided by ω²)
    let prefactor = c_sq / volume;

    let mut modal_sum = Complex64::new(0.0, 0.0);

    for n in 0..=max_mode_order {
        for m in 0..=max_mode_order {
            for p in 0..=max_mode_order {
                // Skip DC mode (0,0,0)
                if n == 0 && m == 0 && p == 0 {
                    continue;
                }

                // Calculate mode angular frequency
                // ωₙ = c * π * sqrt((n/Lx)² + (m/Ly)² + (p/Lz)²)
                let nx = n as f64 / room_width;
                let my = m as f64 / room_depth;
                let pz = p as f64 / room_height;
                let omega_n = speed_of_sound * PI * (nx * nx + my * my + pz * pz).sqrt();
                let omega_n_sq = omega_n * omega_n;

                // Mode frequency for filtering
                let mode_freq = omega_n / (2.0 * PI);

                // Skip modes far from the frequency of interest (2 octaves)
                if mode_freq > frequency * 4.0 || mode_freq < frequency / 4.0 {
                    continue;
                }

                // Calculate mode shape at source position
                // Ψ(r) = cos(nπx/Lx) * cos(mπy/Ly) * cos(pπz/Lz)
                let source_mode = (n as f64 * PI * source.x / room_width).cos()
                    * (m as f64 * PI * source.y / room_depth).cos()
                    * (p as f64 * PI * source.z / room_height).cos();

                // Calculate mode shape at listener position
                let listener_mode = (n as f64 * PI * listener.x / room_width).cos()
                    * (m as f64 * PI * listener.y / room_depth).cos()
                    * (p as f64 * PI * listener.z / room_height).cos();

                // Neumann factor: ε = 1 if index is 0, else 2
                let epsilon = |i: u32| if i == 0 { 1.0 } else { 2.0 };
                let mode_norm = epsilon(n) * epsilon(m) * epsilon(p);

                // Modal transfer function (Kuttruff Eq. 3.27):
                // H(ω) = 1 / (ωₙ² - ω² - j*2*δₙ*ω)
                // where δₙ = ωₙ/(2*Q) is the damping coefficient
                // The factor of 2 comes from expressing damping in terms of Q
                //
                // Units: 1/(rad/s)² = s²/rad²
                let delta_n = omega_n / (2.0 * modal_damping);
                let denominator = Complex64::new(omega_n_sq - omega_sq, -2.0 * delta_n * omega);
                let transfer_function = Complex64::new(1.0, 0.0) / denominator;

                // Add mode contribution
                // Ψ(rs) * Ψ(r) is dimensionless (product of cosines)
                // mode_norm is dimensionless (Neumann factors)
                let mode_amplitude = mode_norm * source_mode * listener_mode;
                modal_sum += transfer_function * mode_amplitude;
            }
        }
    }

    // Apply prefactor c²/V
    // Final units: (m²/s²) / m³ * (s²/rad²) = m⁻¹/rad² = 1/m (treating radians as dimensionless)
    // This matches the Green's function units
    modal_sum *= prefactor;

    // Add phase term to match Green's function form e^(ikr)
    let phase = Complex64::new(0.0, k * r).exp();

    modal_sum * phase
}

/// Calculate hybrid crossover weight for blending modal and ISM responses
///
/// Returns a weight from 0 to 1 where:
/// - 0 = use only modal analysis
/// - 1 = use only ISM
///
/// The crossover uses a smooth cosine transition centered at Schroeder frequency.
pub fn hybrid_crossover_weight(
    frequency: f64,
    schroeder_frequency: f64,
    crossover_width_octaves: f64,
) -> f64 {
    if schroeder_frequency <= 0.0 {
        return 1.0; // Use ISM if no valid Schroeder frequency
    }

    // Calculate distance from Schroeder frequency in octaves
    let octaves_from_schroeder = (frequency / schroeder_frequency).log2();

    // Crossover region: +/- crossover_width_octaves around Schroeder
    let half_width = crossover_width_octaves / 2.0;

    if octaves_from_schroeder < -half_width {
        // Well below Schroeder: use modal only
        0.0
    } else if octaves_from_schroeder > half_width {
        // Well above Schroeder: use ISM only
        1.0
    } else {
        // In crossover region: smooth blend using cosine
        let t = (octaves_from_schroeder + half_width) / crossover_width_octaves;
        // Cosine interpolation for smooth transition
        (1.0 - (t * PI).cos()) / 2.0
    }
}

// ============================================================================
// RT60 and Reverberation Time Calculations
// ============================================================================

/// Room acoustics metrics including RT60
#[derive(Debug, Clone, Serialize)]
pub struct RoomAcoustics {
    /// Sabine RT60 in seconds
    pub rt60_sabine: f64,
    /// Eyring RT60 in seconds (more accurate for absorptive rooms)
    pub rt60_eyring: f64,
    /// Room volume in cubic meters
    pub volume: f64,
    /// Total surface area in square meters
    pub surface_area: f64,
    /// Average absorption coefficient
    pub average_alpha: f64,
    /// Total absorption in sabins (m²)
    pub total_absorption: f64,
    /// Schroeder frequency (transition from modal to statistical behavior)
    pub schroeder_frequency: f64,
    /// Critical distance (where direct and reverberant fields are equal)
    pub critical_distance: f64,
}

/// Calculate RT60 using Sabine's formula
///
/// RT60 = 0.161 * V / A
///
/// Where:
/// - V = room volume (m³)
/// - A = total absorption (sabins, m²) = Σ(αᵢ * Sᵢ)
///
/// Valid for rooms with average absorption < 0.2
pub fn rt60_sabine(volume: f64, total_absorption: f64) -> f64 {
    if total_absorption > 0.0 {
        0.161 * volume / total_absorption
    } else {
        f64::INFINITY // No absorption = infinite reverberation
    }
}

/// Calculate RT60 using Eyring's formula
///
/// RT60 = 0.161 * V / (-S * ln(1 - α_avg))
///
/// Where:
/// - V = room volume (m³)
/// - S = total surface area (m²)
/// - α_avg = average absorption coefficient
///
/// More accurate than Sabine for rooms with higher absorption (α > 0.2)
pub fn rt60_eyring(volume: f64, surface_area: f64, average_alpha: f64) -> f64 {
    if average_alpha > 0.0 && average_alpha < 1.0 {
        let clamped_alpha = average_alpha.min(0.99); // Prevent ln(0)
        0.161 * volume / (-surface_area * (1.0 - clamped_alpha).ln())
    } else if average_alpha >= 1.0 {
        0.0 // Perfect absorption = no reverberation
    } else {
        f64::INFINITY // No absorption = infinite reverberation
    }
}

/// Calculate critical distance (where direct and reverberant fields are equal)
///
/// r_c = 0.057 * sqrt(V / RT60)
///
/// At distances less than r_c, direct sound dominates.
/// At distances greater than r_c, reverberant sound dominates.
pub fn critical_distance(volume: f64, rt60: f64) -> f64 {
    if rt60 > 0.0 {
        0.057 * (volume / rt60).sqrt()
    } else {
        f64::INFINITY // No reverberation = direct sound always dominates
    }
}

/// Calculate room acoustics for a rectangular room with given wall materials
pub fn calculate_room_acoustics(
    width: f64,
    depth: f64,
    height: f64,
    materials: &WallMaterialsConfig,
    frequency: f64,
) -> RoomAcoustics {
    // Calculate room geometry
    let volume = width * depth * height;

    // Surface areas for each wall pair
    let area_left_right = depth * height; // Left and right walls (perpendicular to X)
    let area_front_back = width * height; // Front and back walls (perpendicular to Y)
    let area_floor_ceiling = width * depth; // Floor and ceiling (perpendicular to Z)

    let surface_area = 2.0 * (area_left_right + area_front_back + area_floor_ceiling);

    // Get absorption coefficients at the specified frequency
    let alpha_left = materials.left.absorption_at_frequency(frequency);
    let alpha_right = materials.right.absorption_at_frequency(frequency);
    let alpha_front = materials.front.absorption_at_frequency(frequency);
    let alpha_back = materials.back.absorption_at_frequency(frequency);
    let alpha_floor = materials.floor.absorption_at_frequency(frequency);
    let alpha_ceiling = materials.ceiling.absorption_at_frequency(frequency);

    // Calculate total absorption (sabins)
    let total_absorption = alpha_left * area_left_right
        + alpha_right * area_left_right
        + alpha_front * area_front_back
        + alpha_back * area_front_back
        + alpha_floor * area_floor_ceiling
        + alpha_ceiling * area_floor_ceiling;

    // Calculate average absorption coefficient
    let average_alpha = total_absorption / surface_area;

    // Calculate RT60 using both formulas
    let rt60_sab = rt60_sabine(volume, total_absorption);
    let rt60_eyr = rt60_eyring(volume, surface_area, average_alpha);

    // Use Eyring RT60 for Schroeder frequency (more accurate)
    let schroeder_freq = 2000.0 * (rt60_eyr / volume).sqrt();
    let crit_dist = critical_distance(volume, rt60_eyr);

    RoomAcoustics {
        rt60_sabine: rt60_sab,
        rt60_eyring: rt60_eyr,
        volume,
        surface_area,
        average_alpha,
        total_absorption,
        schroeder_frequency: schroeder_freq,
        critical_distance: crit_dist,
    }
}

// ============================================================================
// Impulse Response Calculation
// ============================================================================

/// Impulse response data structure
#[derive(Debug, Clone, Serialize)]
pub struct ImpulseResponse {
    /// Time samples in seconds
    pub time: Vec<f64>,
    /// Amplitude samples (normalized)
    pub amplitude: Vec<f64>,
    /// Sample rate in Hz
    pub sample_rate: f64,
    /// Duration in seconds
    pub duration: f64,
    /// Peak amplitude
    pub peak_amplitude: f64,
    /// Energy decay curve (ETC) in dB
    pub energy_decay: Vec<f64>,
}

/// Configuration for impulse response generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImpulseResponseConfig {
    /// Sample rate for output IR (default 48000 Hz)
    #[serde(default = "default_ir_sample_rate")]
    pub sample_rate: f64,
    /// IR duration in seconds (default: auto from RT60)
    #[serde(default)]
    pub duration: Option<f64>,
    /// Number of FFT points (default: power of 2 based on duration)
    #[serde(default)]
    pub fft_size: Option<usize>,
    /// Minimum frequency for spectrum (default: 20 Hz)
    #[serde(default = "default_ir_min_freq")]
    pub min_freq: f64,
    /// Maximum frequency for spectrum (default: Nyquist/2)
    #[serde(default)]
    pub max_freq: Option<f64>,
}

fn default_ir_sample_rate() -> f64 {
    48000.0
}
fn default_ir_min_freq() -> f64 {
    20.0
}

impl Default for ImpulseResponseConfig {
    fn default() -> Self {
        Self {
            sample_rate: default_ir_sample_rate(),
            duration: None,
            fft_size: None,
            min_freq: default_ir_min_freq(),
            max_freq: None,
        }
    }
}

/// Calculate impulse response from frequency response using IFFT
///
/// This function takes complex frequency response data and converts it to
/// a time-domain impulse response using the inverse FFT.
///
/// The process:
/// 1. Interpolate frequency response to uniform spacing (for FFT)
/// 2. Create conjugate-symmetric spectrum (for real output)
/// 3. Apply IFFT
/// 4. Window and normalize the result
pub fn calculate_impulse_response(
    frequencies: &[f64],
    complex_response: &[Complex64],
    config: &ImpulseResponseConfig,
) -> ImpulseResponse {
    let sample_rate = config.sample_rate;
    let nyquist = sample_rate / 2.0;

    // Determine FFT size (power of 2)
    let duration = config.duration.unwrap_or(0.5); // Default 500ms
    let min_fft_size = (duration * sample_rate).ceil() as usize;
    let fft_size = config.fft_size.unwrap_or_else(|| {
        // Round up to next power of 2
        let mut size = 1;
        while size < min_fft_size {
            size *= 2;
        }
        size.max(1024)
    });

    // Frequency resolution
    let freq_resolution = sample_rate / fft_size as f64;

    // Create uniform frequency bins for FFT
    let num_bins = fft_size / 2 + 1;
    let mut spectrum: Vec<Complex64> = vec![Complex64::new(0.0, 0.0); fft_size];

    // Interpolate frequency response to FFT bins
    #[allow(clippy::needless_range_loop)]
    for bin in 0..num_bins {
        let freq = bin as f64 * freq_resolution;

        // Skip DC and frequencies outside our range
        if freq < config.min_freq || freq > nyquist {
            spectrum[bin] = Complex64::new(0.0, 0.0);
            continue;
        }

        // Find bracketing frequencies and interpolate
        let value = interpolate_complex(frequencies, complex_response, freq);
        spectrum[bin] = value;
    }

    // Create conjugate-symmetric spectrum for real-valued output
    // spectrum[N-k] = conj(spectrum[k])
    for k in 1..fft_size / 2 {
        spectrum[fft_size - k] = spectrum[k].conj();
    }

    // Apply IFFT manually using DFT (simple implementation for WASM)
    // For production, use a proper FFT library
    let time_domain = ifft_real(&spectrum);

    // Find peak and normalize
    let peak = time_domain.iter().map(|&x| x.abs()).fold(0.0_f64, f64::max);
    let normalized: Vec<f64> = if peak > 1e-10 {
        time_domain.iter().map(|&x| x / peak).collect()
    } else {
        time_domain.clone()
    };

    // Calculate time vector
    let time: Vec<f64> = (0..fft_size).map(|i| i as f64 / sample_rate).collect();

    // Calculate energy decay curve (Schroeder integration)
    let energy_decay = calculate_energy_decay(&normalized);

    ImpulseResponse {
        time,
        amplitude: normalized,
        sample_rate,
        duration: fft_size as f64 / sample_rate,
        peak_amplitude: peak,
        energy_decay,
    }
}

/// Interpolate complex frequency response
fn interpolate_complex(frequencies: &[f64], values: &[Complex64], target_freq: f64) -> Complex64 {
    if frequencies.is_empty() || values.is_empty() {
        return Complex64::new(0.0, 0.0);
    }

    // Clamp to range
    if target_freq <= frequencies[0] {
        return values[0];
    }
    if target_freq >= frequencies[frequencies.len() - 1] {
        return values[values.len() - 1];
    }

    // Find bracketing indices using logarithmic interpolation
    for i in 0..frequencies.len() - 1 {
        if target_freq >= frequencies[i] && target_freq < frequencies[i + 1] {
            // Log-frequency interpolation
            let log_f = target_freq.ln();
            let log_f1 = frequencies[i].ln();
            let log_f2 = frequencies[i + 1].ln();
            let t = (log_f - log_f1) / (log_f2 - log_f1);

            // Interpolate magnitude and phase separately for better results
            let mag1 = values[i].norm();
            let mag2 = values[i + 1].norm();
            let phase1 = values[i].arg();
            let phase2 = values[i + 1].arg();

            // Unwrap phase if needed
            let mut phase_diff = phase2 - phase1;
            while phase_diff > PI {
                phase_diff -= 2.0 * PI;
            }
            while phase_diff < -PI {
                phase_diff += 2.0 * PI;
            }

            let mag = mag1 * (1.0 - t) + mag2 * t;
            let phase = phase1 + t * phase_diff;

            return Complex64::from_polar(mag, phase);
        }
    }

    values[values.len() - 1]
}

/// Simple IFFT implementation for real-valued output
/// Uses direct DFT computation (O(N²) but works without external FFT library)
fn ifft_real(spectrum: &[Complex64]) -> Vec<f64> {
    let n = spectrum.len();
    let mut output = vec![0.0; n];

    // For small sizes, use direct DFT
    // For larger sizes, this should be replaced with a proper FFT
    if n <= 4096 {
        #[allow(clippy::needless_range_loop)]
        for k in 0..n {
            let mut sum = Complex64::new(0.0, 0.0);
            for (m, &spec_val) in spectrum.iter().enumerate() {
                let angle = 2.0 * PI * (k as f64) * (m as f64) / (n as f64);
                let twiddle = Complex64::new(angle.cos(), angle.sin());
                sum += spec_val * twiddle;
            }
            output[k] = sum.re / n as f64;
        }
    } else {
        // Cooley-Tukey radix-2 IFFT
        output = cooley_tukey_ifft(spectrum);
    }

    output
}

/// Cooley-Tukey radix-2 IFFT
fn cooley_tukey_ifft(spectrum: &[Complex64]) -> Vec<f64> {
    let n = spectrum.len();

    // Bit-reversal permutation
    let mut data: Vec<Complex64> = spectrum.to_vec();
    let mut j = 0;
    for i in 0..n {
        if i < j {
            data.swap(i, j);
        }
        let mut m = n / 2;
        while m >= 1 && j >= m {
            j -= m;
            m /= 2;
        }
        j += m;
    }

    // Iterative FFT
    let mut len = 2;
    while len <= n {
        let half = len / 2;
        let angle_step = 2.0 * PI / len as f64; // Note: positive for IFFT

        for start in (0..n).step_by(len) {
            for k in 0..half {
                let angle = angle_step * k as f64;
                let twiddle = Complex64::new(angle.cos(), angle.sin());

                let even = data[start + k];
                let odd = data[start + k + half] * twiddle;

                data[start + k] = even + odd;
                data[start + k + half] = even - odd;
            }
        }

        len *= 2;
    }

    // Normalize and extract real part
    data.iter().map(|c| c.re / n as f64).collect()
}

/// Calculate energy decay curve (Schroeder integration)
/// EDT is calculated by reverse integration of squared impulse response
fn calculate_energy_decay(ir: &[f64]) -> Vec<f64> {
    let n = ir.len();
    if n == 0 {
        return vec![];
    }

    // Calculate squared amplitude (energy)
    let energy: Vec<f64> = ir.iter().map(|&x| x * x).collect();

    // Reverse cumulative sum (Schroeder integration)
    let mut decay = vec![0.0; n];
    let mut cumsum = 0.0;

    for i in (0..n).rev() {
        cumsum += energy[i];
        decay[i] = cumsum;
    }

    // Normalize and convert to dB
    let max_energy = decay[0].max(1e-10);
    decay
        .iter()
        .map(|&e| 10.0 * (e / max_energy).log10())
        .collect()
}

// ============================================================================
// Binaural Rendering
// ============================================================================

/// Default head radius (meters) - average human head
const DEFAULT_HEAD_RADIUS: f64 = 0.0875; // 8.75 cm

/// Default ear spacing (meters) - typical ear-to-ear distance
const DEFAULT_EAR_SPACING: f64 = 0.175; // 17.5 cm

/// Binaural output containing left and right ear responses
#[derive(Debug, Clone, Serialize)]
pub struct BinauralResponse {
    /// Left ear impulse response
    pub left: ImpulseResponse,
    /// Right ear impulse response
    pub right: ImpulseResponse,
    /// Interaural time difference (seconds) - positive means left ear leads
    pub itd: f64,
    /// Interaural level difference (dB) - positive means left ear is louder
    pub ild: f64,
    /// Head position used
    pub head_position: [f64; 3],
    /// Head orientation (yaw in degrees, 0 = facing +Y)
    pub head_yaw: f64,
}

/// Configuration for binaural rendering
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BinauralConfig {
    /// Enable binaural rendering
    #[serde(default)]
    pub enabled: bool,
    /// Head center position (defaults to listening position)
    #[serde(default)]
    pub head_position: Option<Point3DConfig>,
    /// Head yaw angle in degrees (0 = facing +Y, 90 = facing -X)
    #[serde(default)]
    pub head_yaw: f64,
    /// Head radius in meters (default 0.0875m)
    #[serde(default = "default_head_radius")]
    pub head_radius: f64,
    /// Ear spacing in meters (default 0.175m)
    #[serde(default = "default_ear_spacing")]
    pub ear_spacing: f64,
    /// Impulse response configuration for binaural output
    #[serde(default)]
    pub ir_config: ImpulseResponseConfig,
}

fn default_head_radius() -> f64 {
    DEFAULT_HEAD_RADIUS
}
fn default_ear_spacing() -> f64 {
    DEFAULT_EAR_SPACING
}

impl Default for BinauralConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            head_position: None,
            head_yaw: 0.0,
            head_radius: DEFAULT_HEAD_RADIUS,
            ear_spacing: DEFAULT_EAR_SPACING,
            ir_config: ImpulseResponseConfig::default(),
        }
    }
}

/// Calculate ear positions from head center and orientation
pub fn calculate_ear_positions(
    head_center: &Point3D,
    head_yaw_deg: f64,
    ear_spacing: f64,
) -> (Point3D, Point3D) {
    let yaw_rad = head_yaw_deg.to_radians();
    let half_spacing = ear_spacing / 2.0;

    // Ears are perpendicular to facing direction
    // If facing +Y (yaw=0), left ear is at -X, right ear is at +X
    let ear_dx = -yaw_rad.sin() * half_spacing;
    let ear_dy = yaw_rad.cos() * half_spacing;

    // Left ear (positive perpendicular direction when facing forward)
    let left_ear = Point3D::new(
        head_center.x - ear_dx,
        head_center.y - ear_dy,
        head_center.z,
    );

    // Right ear (negative perpendicular direction)
    let right_ear = Point3D::new(
        head_center.x + ear_dx,
        head_center.y + ear_dy,
        head_center.z,
    );

    (left_ear, right_ear)
}

/// Calculate interaural time difference (ITD) using Woodworth's formula
///
/// ITD = (r/c) * (theta + sin(theta))
///
/// Where:
/// - r = head radius
/// - c = speed of sound
/// - theta = azimuth angle from median plane
pub fn calculate_itd(
    source: &Point3D,
    head_center: &Point3D,
    head_yaw_deg: f64,
    head_radius: f64,
    speed_of_sound: f64,
) -> f64 {
    // Calculate azimuth angle relative to head orientation
    let dx = source.x - head_center.x;
    let dy = source.y - head_center.y;

    // Source azimuth in world coordinates
    let source_azimuth = dx.atan2(dy);

    // Head facing direction
    let head_facing = head_yaw_deg.to_radians();

    // Relative azimuth (positive = source to the left)
    let theta = source_azimuth - head_facing;

    // Woodworth's formula
    (head_radius / speed_of_sound) * (theta + theta.sin())
}

/// Simplified HRTF magnitude approximation
///
/// This uses a simplified head-shadowing model based on the angle to the source.
/// For more accurate results, measured HRTF databases should be used.
///
/// Returns (left_gain, right_gain) as linear multipliers
pub fn approximate_hrtf_magnitude(
    source: &Point3D,
    head_center: &Point3D,
    head_yaw_deg: f64,
    frequency: f64,
) -> (f64, f64) {
    // Calculate azimuth relative to head
    let dx = source.x - head_center.x;
    let dy = source.y - head_center.y;
    let source_azimuth = dx.atan2(dy);
    let head_facing = head_yaw_deg.to_radians();
    let theta = source_azimuth - head_facing;

    // Head shadowing is more pronounced at higher frequencies
    // Simple model: use sinusoidal ILD that increases with frequency
    let freq_factor = (frequency / 1000.0).clamp(0.1, 4.0);

    // Maximum ILD is about 10-20 dB at high frequencies for sources at 90 degrees
    let max_ild_db = 6.0 * freq_factor; // ~6dB at 1kHz, ~24dB at 4kHz+

    // ILD varies sinusoidally with azimuth
    let ild_db = max_ild_db * theta.sin();

    // Convert to linear gains
    let left_gain = 10.0_f64.powf(-ild_db / 20.0);
    let right_gain = 10.0_f64.powf(ild_db / 20.0);

    // Normalize so average is 1
    let avg = (left_gain + right_gain) / 2.0;
    (left_gain / avg, right_gain / avg)
}

/// Calculate binaural response for a room simulation
pub fn calculate_binaural_response(
    frequencies: &[f64],
    left_pressures: &[Complex64],
    right_pressures: &[Complex64],
    config: &BinauralConfig,
    _speed_of_sound: f64,
    head_position: Point3D,
) -> BinauralResponse {
    // Calculate impulse responses for both ears
    let left_ir = calculate_impulse_response(frequencies, left_pressures, &config.ir_config);
    let right_ir = calculate_impulse_response(frequencies, right_pressures, &config.ir_config);

    // Calculate average ITD from magnitude-weighted phases
    let mut itd_sum = 0.0;
    let mut weight_sum = 0.0;

    for i in 0..frequencies
        .len()
        .min(left_pressures.len())
        .min(right_pressures.len())
    {
        let left_phase = left_pressures[i].arg();
        let right_phase = right_pressures[i].arg();
        let freq = frequencies[i];

        if freq > 20.0 && freq < 1500.0 {
            // Phase-based ITD is most reliable below 1.5kHz
            let phase_diff = left_phase - right_phase;
            let itd_sample = phase_diff / (2.0 * PI * freq);
            let weight = (left_pressures[i].norm() + right_pressures[i].norm()) / 2.0;
            itd_sum += itd_sample * weight;
            weight_sum += weight;
        }
    }

    let itd = if weight_sum > 1e-10 {
        itd_sum / weight_sum
    } else {
        0.0
    };

    // Calculate ILD from average magnitude difference
    let left_energy: f64 = left_pressures.iter().map(|p| p.norm_sqr()).sum();
    let right_energy: f64 = right_pressures.iter().map(|p| p.norm_sqr()).sum();

    let ild = if right_energy > 1e-10 {
        10.0 * (left_energy / right_energy).log10()
    } else if left_energy > 1e-10 {
        20.0 // Left much louder
    } else {
        0.0
    };

    BinauralResponse {
        left: left_ir,
        right: right_ir,
        itd,
        ild,
        head_position: [head_position.x, head_position.y, head_position.z],
        head_yaw: config.head_yaw,
    }
}

// ============================================================================
// Edge Diffraction (Simplified Biot-Tolstoy model)
// ============================================================================

/// Calculate edge diffraction coefficient using simplified UTD (Uniform Theory of Diffraction)
///
/// This is a simplified approximation based on the Biot-Tolstoy-Medwin model,
/// commonly used in room acoustics for diffraction around edges.
///
/// The diffraction coefficient depends on:
/// - Wedge angle (for room corners, typically 90 degrees = pi/2)
/// - Source and receiver angles relative to the edge
/// - Frequency (through wavenumber k)
/// - Distances from source/receiver to edge
///
/// Reference: Svensson, U. P., Fred, R. I., & Vanderkooy, J. (1999).
/// "An analytic secondary source model of edge diffraction impulse responses."
pub fn edge_diffraction_coefficient(
    wedge_angle: f64,  // Interior wedge angle (radians), typically PI/2 for room corners
    r_source: f64,     // Distance from source to edge
    r_receiver: f64,   // Distance from receiver to edge
    theta_source: f64, // Angle from source to edge face (radians)
    theta_receiver: f64, // Angle from receiver to edge face (radians)
    k: f64,            // Wavenumber (2 * PI * f / c)
) -> Complex64 {
    // Wedge index (n = PI / wedge_angle)
    let n = PI / wedge_angle;

    // Total path length from source around edge to receiver
    let path_length = r_source + r_receiver;

    // Diffraction attenuation factor
    // This is a simplified version - full BTM involves integral over edge length
    let nu = n;

    // Cotangent diffraction functions for each term
    let beta_plus = theta_source + theta_receiver;
    let beta_minus = theta_source - theta_receiver;

    // Fresnel-like diffraction function (simplified)
    let d_plus = cot_diffraction_term(beta_plus, nu);
    let d_minus = cot_diffraction_term(beta_minus, nu);

    // Distance factor (geometric spreading from edge)
    let distance_factor = 1.0 / (4.0 * PI * (r_source * r_receiver).sqrt());

    // Frequency-dependent phase
    let phase = Complex64::new(0.0, k * path_length).exp();

    // Combined diffraction coefficient
    let diffraction_amplitude = distance_factor * (d_plus + d_minus).abs();

    phase * diffraction_amplitude
}

/// Helper function for cotangent diffraction term
fn cot_diffraction_term(beta: f64, nu: f64) -> f64 {
    // Simplified cotangent function for diffraction calculation
    // cot((PI + beta) / (2 * nu))
    let arg = (PI + beta) / (2.0 * nu);

    // Handle singularities
    if arg.sin().abs() < 1e-10 {
        return 0.0;
    }

    arg.cos() / arg.sin()
}

/// Represents an edge in the room for diffraction calculation
#[derive(Debug, Clone)]
pub struct DiffractionEdge {
    /// Start point of the edge
    pub start: Point3D,
    /// End point of the edge
    pub end: Point3D,
    /// Interior wedge angle (radians)
    pub wedge_angle: f64,
}

impl DiffractionEdge {
    pub fn new(start: Point3D, end: Point3D, wedge_angle: f64) -> Self {
        Self {
            start,
            end,
            wedge_angle,
        }
    }

    /// Get the edge direction vector
    pub fn direction(&self) -> Point3D {
        let dx = self.end.x - self.start.x;
        let dy = self.end.y - self.start.y;
        let dz = self.end.z - self.start.z;
        let len = (dx * dx + dy * dy + dz * dz).sqrt();
        Point3D::new(dx / len, dy / len, dz / len)
    }

    /// Get the edge length
    pub fn length(&self) -> f64 {
        self.start.distance_to(&self.end)
    }

    /// Find closest point on edge to a given point
    pub fn closest_point(&self, point: &Point3D) -> Point3D {
        let edge_vec = Point3D::new(
            self.end.x - self.start.x,
            self.end.y - self.start.y,
            self.end.z - self.start.z,
        );
        let point_vec = Point3D::new(
            point.x - self.start.x,
            point.y - self.start.y,
            point.z - self.start.z,
        );

        let edge_len_sq =
            edge_vec.x * edge_vec.x + edge_vec.y * edge_vec.y + edge_vec.z * edge_vec.z;
        if edge_len_sq < 1e-10 {
            return self.start;
        }

        let dot = point_vec.x * edge_vec.x + point_vec.y * edge_vec.y + point_vec.z * edge_vec.z;
        let t = (dot / edge_len_sq).clamp(0.0, 1.0);

        Point3D::new(
            self.start.x + t * edge_vec.x,
            self.start.y + t * edge_vec.y,
            self.start.z + t * edge_vec.z,
        )
    }

    /// Calculate diffraction contribution from this edge
    pub fn diffraction_contribution(
        &self,
        source: &Point3D,
        receiver: &Point3D,
        k: f64,
    ) -> Complex64 {
        // Find the diffraction point (closest point on edge to midpoint of source-receiver)
        let midpoint = Point3D::new(
            (source.x + receiver.x) / 2.0,
            (source.y + receiver.y) / 2.0,
            (source.z + receiver.z) / 2.0,
        );
        let diff_point = self.closest_point(&midpoint);

        // Distances from source/receiver to diffraction point
        let r_source = source.distance_to(&diff_point);
        let r_receiver = receiver.distance_to(&diff_point);

        // Skip if distances are too small
        if r_source < 0.01 || r_receiver < 0.01 {
            return Complex64::new(0.0, 0.0);
        }

        // Simplified angles (assume perpendicular to edge)
        // In a full implementation, we'd calculate the actual angles relative to edge faces
        let theta_source = PI / 4.0; // 45 degrees - typical value
        let theta_receiver = PI / 4.0;

        edge_diffraction_coefficient(
            self.wedge_angle,
            r_source,
            r_receiver,
            theta_source,
            theta_receiver,
            k,
        )
    }
}

/// Get diffraction edges for a rectangular room (12 edges at corners)
pub fn get_rectangular_room_edges(width: f64, depth: f64, height: f64) -> Vec<DiffractionEdge> {
    let wedge_90 = PI / 2.0; // 90-degree corners

    vec![
        // Floor edges
        DiffractionEdge::new(
            Point3D::new(0.0, 0.0, 0.0),
            Point3D::new(width, 0.0, 0.0),
            wedge_90,
        ),
        DiffractionEdge::new(
            Point3D::new(width, 0.0, 0.0),
            Point3D::new(width, depth, 0.0),
            wedge_90,
        ),
        DiffractionEdge::new(
            Point3D::new(width, depth, 0.0),
            Point3D::new(0.0, depth, 0.0),
            wedge_90,
        ),
        DiffractionEdge::new(
            Point3D::new(0.0, depth, 0.0),
            Point3D::new(0.0, 0.0, 0.0),
            wedge_90,
        ),
        // Ceiling edges
        DiffractionEdge::new(
            Point3D::new(0.0, 0.0, height),
            Point3D::new(width, 0.0, height),
            wedge_90,
        ),
        DiffractionEdge::new(
            Point3D::new(width, 0.0, height),
            Point3D::new(width, depth, height),
            wedge_90,
        ),
        DiffractionEdge::new(
            Point3D::new(width, depth, height),
            Point3D::new(0.0, depth, height),
            wedge_90,
        ),
        DiffractionEdge::new(
            Point3D::new(0.0, depth, height),
            Point3D::new(0.0, 0.0, height),
            wedge_90,
        ),
        // Vertical edges
        DiffractionEdge::new(
            Point3D::new(0.0, 0.0, 0.0),
            Point3D::new(0.0, 0.0, height),
            wedge_90,
        ),
        DiffractionEdge::new(
            Point3D::new(width, 0.0, 0.0),
            Point3D::new(width, 0.0, height),
            wedge_90,
        ),
        DiffractionEdge::new(
            Point3D::new(width, depth, 0.0),
            Point3D::new(width, depth, height),
            wedge_90,
        ),
        DiffractionEdge::new(
            Point3D::new(0.0, depth, 0.0),
            Point3D::new(0.0, depth, height),
            wedge_90,
        ),
    ]
}

// ============================================================================
// Core computation functions
// ============================================================================

/// Green's function for 3D Helmholtz equation: G(r) = exp(ikr) / (4 pi r)
fn greens_function_3d(r: f64, k: f64) -> Complex64 {
    if r < 1e-10 {
        return Complex64::new(0.0, 0.0);
    }
    let ikr = Complex64::new(0.0, k * r);
    ikr.exp() / (4.0 * PI * r)
}

/// Convert complex pressure to SPL (dB re 20 uPa)
fn pressure_to_spl(pressure: Complex64) -> f64 {
    let magnitude = pressure.norm();
    let p_ref = 20e-6;
    20.0 * (magnitude / p_ref).max(1e-10).log10()
}

/// Generate logarithmically spaced frequencies
fn log_space(start: f64, end: f64, num: usize) -> Vec<f64> {
    if num <= 1 {
        return vec![start];
    }
    let log_start = start.ln();
    let log_end = end.ln();
    (0..num)
        .map(|i| {
            let log_val = log_start + (log_end - log_start) * i as f64 / (num - 1) as f64;
            log_val.exp()
        })
        .collect()
}

/// Generate linearly spaced values
fn lin_space(start: f64, end: f64, num: usize) -> Vec<f64> {
    if num <= 1 {
        return vec![start];
    }
    (0..num)
        .map(|i| start + (end - start) * i as f64 / (num - 1) as f64)
        .collect()
}

/// Create a simple cardioid directivity pattern
fn create_cardioid_pattern(front_back_ratio: f64) -> DirectivityPattern {
    let horizontal_angles: Vec<f64> = (0..36).map(|i| i as f64 * 10.0).collect();
    let vertical_angles: Vec<f64> = (0..19).map(|i| i as f64 * 10.0).collect();

    let mut magnitude = Array2::zeros((vertical_angles.len(), horizontal_angles.len()));

    // Cardioid pattern: (a + b*cos(theta)) where a + b = 1 (front), a - b = 1/ratio (back)
    // Solving: a = (ratio + 1) / (2 * ratio), b = (ratio - 1) / (2 * ratio)
    let a = (front_back_ratio + 1.0) / (2.0 * front_back_ratio);
    let b = (front_back_ratio - 1.0) / (2.0 * front_back_ratio);

    for (v_idx, &theta_deg) in vertical_angles.iter().enumerate() {
        for (h_idx, &phi_deg) in horizontal_angles.iter().enumerate() {
            let theta = theta_deg.to_radians();
            let phi = phi_deg.to_radians();
            let cos_angle = theta.sin() * phi.cos();
            let response = (a + b * cos_angle).max(0.0);
            magnitude[[v_idx, h_idx]] = response;
        }
    }

    DirectivityPattern {
        horizontal_angles,
        vertical_angles,
        magnitude,
    }
}

/// Create a DirectivityPattern from spinorama.org directivity data
///
/// The spinorama data contains SPL curves at different angles for both horizontal
/// and vertical planes. This function:
/// 1. Normalizes all SPL values relative to the on-axis response
/// 2. Converts dB differences to linear magnitude ratios
/// 3. Creates a 2D interpolation grid for arbitrary angle lookups
///
/// # Arguments
/// * `horizontal` - Horizontal plane SPL curves at different angles
/// * `vertical` - Vertical plane SPL curves at different angles
/// * `frequency` - The frequency at which to sample the directivity
fn create_spinorama_pattern(
    horizontal: &[SpinoramaCurve],
    vertical: &[SpinoramaCurve],
    frequency: f64,
) -> DirectivityPattern {
    // Standard 10-degree resolution for the pattern
    let horizontal_angles: Vec<f64> = (0..36).map(|i| i as f64 * 10.0).collect();
    let vertical_angles: Vec<f64> = (0..19).map(|i| i as f64 * 10.0).collect();

    let mut magnitude = Array2::ones((vertical_angles.len(), horizontal_angles.len()));

    // Find the on-axis SPL for normalization (0° in horizontal data)
    let on_axis_spl = horizontal
        .iter()
        .find(|c| (c.angle - 0.0).abs() < 0.5)
        .map(|c| interpolate_spl_at_frequency(&c.freq, &c.spl, frequency))
        .unwrap_or(0.0);

    // Helper to get SPL at a specific angle from spinorama data
    let get_spl_at_angle = |curves: &[SpinoramaCurve], angle: f64| -> f64 {
        // Handle the fact that spinorama uses -60 to +60 range
        // while our pattern uses 0 to 360 for horizontal
        let search_angle = if angle > 180.0 { angle - 360.0 } else { angle };

        // Find the two closest angles and interpolate
        let mut closest_below: Option<&SpinoramaCurve> = None;
        let mut closest_above: Option<&SpinoramaCurve> = None;

        for curve in curves {
            if curve.angle <= search_angle
                && (closest_below.is_none() || curve.angle > closest_below.unwrap().angle)
            {
                closest_below = Some(curve);
            }
            if curve.angle >= search_angle
                && (closest_above.is_none() || curve.angle < closest_above.unwrap().angle)
            {
                closest_above = Some(curve);
            }
        }

        match (closest_below, closest_above) {
            (Some(below), Some(above)) if (below.angle - above.angle).abs() < 0.5 => {
                // Exact match
                interpolate_spl_at_frequency(&below.freq, &below.spl, frequency)
            }
            (Some(below), Some(above)) => {
                // Interpolate between two angles
                let spl_below = interpolate_spl_at_frequency(&below.freq, &below.spl, frequency);
                let spl_above = interpolate_spl_at_frequency(&above.freq, &above.spl, frequency);
                let t = (search_angle - below.angle) / (above.angle - below.angle);
                spl_below * (1.0 - t) + spl_above * t
            }
            (Some(only), None) | (None, Some(only)) => {
                interpolate_spl_at_frequency(&only.freq, &only.spl, frequency)
            }
            (None, None) => on_axis_spl, // Fallback to on-axis
        }
    };

    // Fill the magnitude array
    // Horizontal plane (theta = 90°, phi varies) - use horizontal data
    // Vertical plane (phi = 0°, theta varies) - use vertical data
    // Other angles: interpolate between horizontal and vertical

    for (v_idx, &theta_deg) in vertical_angles.iter().enumerate() {
        for (h_idx, &phi_deg) in horizontal_angles.iter().enumerate() {
            // Get SPL from horizontal and vertical data
            let h_spl = get_spl_at_angle(horizontal, phi_deg);
            let v_spl = get_spl_at_angle(vertical, theta_deg);

            // Blend based on angle: at theta=90° use horizontal, at theta=0° or 180° use vertical
            let theta_rad = theta_deg.to_radians();
            let blend = theta_rad.sin(); // 1 at 90°, 0 at 0° and 180°

            let combined_spl = h_spl * blend + v_spl * (1.0 - blend);

            // Convert dB difference to linear magnitude
            let db_diff = combined_spl - on_axis_spl;
            let linear_mag = 10.0_f64.powf(db_diff / 20.0);

            magnitude[[v_idx, h_idx]] = linear_mag.clamp(0.0, 10.0);
        }
    }

    DirectivityPattern {
        horizontal_angles,
        vertical_angles,
        magnitude,
    }
}

/// Interpolate SPL value at a specific frequency from spinorama frequency/SPL arrays
fn interpolate_spl_at_frequency(freq: &[f64], spl: &[f64], target_freq: f64) -> f64 {
    if freq.is_empty() || spl.is_empty() {
        return 0.0;
    }

    // Handle edge cases
    if target_freq <= freq[0] {
        return spl[0];
    }
    if target_freq >= freq[freq.len() - 1] {
        return spl[spl.len() - 1];
    }

    // Find bracketing indices and interpolate logarithmically
    for i in 0..freq.len() - 1 {
        if target_freq >= freq[i] && target_freq < freq[i + 1] {
            let log_f = target_freq.ln();
            let log_f1 = freq[i].ln();
            let log_f2 = freq[i + 1].ln();
            let t = (log_f - log_f1) / (log_f2 - log_f1);
            return spl[i] * (1.0 - t) + spl[i + 1] * t;
        }
    }

    spl[spl.len() - 1]
}

// ============================================================================
// WASM-exported Room Simulator
// ============================================================================

/// Room Acoustics Simulator - WASM interface
#[wasm_bindgen]
pub struct RoomSimulatorWasm {
    config: SimulationConfig,
    room_geometry: RoomGeometry,
    sources: Vec<Source>,
    listening_position: Point3D,
    frequencies: Vec<f64>,
    speed_of_sound: f64,
    wall_materials: WallMaterialsConfig,
    /// Temperature in Celsius
    temperature: f64,
    /// Relative humidity (0-100%)
    humidity: f64,
    /// Whether to model air absorption
    air_absorption_enabled: bool,
    /// Whether to model edge diffraction
    edge_diffraction_enabled: bool,
    /// Pre-computed diffraction edges for the room
    diffraction_edges: Vec<DiffractionEdge>,
}

#[wasm_bindgen]
impl RoomSimulatorWasm {
    /// Create a new simulator from JSON configuration
    #[wasm_bindgen(constructor)]
    pub fn new(config_json: &str) -> Result<RoomSimulatorWasm, JsValue> {
        init_panic_hook();

        let config: SimulationConfig = serde_json::from_str(config_json)
            .map_err(|e| JsValue::from_str(&format!("Config parse error: {}", e)))?;

        // Build room geometry
        let room_geometry = match &config.room {
            RoomGeometryConfig::Rectangular {
                width,
                depth,
                height,
            } => {
                console_log!(
                    "Creating rectangular room: {}x{}x{} m",
                    width,
                    depth,
                    height
                );
                RoomGeometry::Rectangular(RectangularRoom::new(*width, *depth, *height))
            }
            RoomGeometryConfig::LShaped {
                width1,
                depth1,
                width2,
                depth2,
                height,
            } => {
                console_log!(
                    "Creating L-shaped room: {}x{} + {}x{} x {} m",
                    width1,
                    depth1,
                    width2,
                    depth2,
                    height
                );
                RoomGeometry::LShaped(LShapedRoom::new(
                    *width1, *depth1, *width2, *depth2, *height,
                ))
            }
        };

        // Build sources
        let sources: Vec<Source> = config
            .sources
            .iter()
            .map(|s| {
                let directivity = match &s.directivity {
                    DirectivityConfig::Omnidirectional => DirectivityPattern::omnidirectional(),
                    DirectivityConfig::Cardioid { front_back_ratio } => {
                        create_cardioid_pattern(*front_back_ratio)
                    }
                    DirectivityConfig::Spinorama {
                        horizontal,
                        vertical,
                    } => {
                        // Use 1kHz as the representative frequency for the directivity pattern
                        // This is a common choice for speaker directivity visualization
                        create_spinorama_pattern(horizontal, vertical, 1000.0)
                    }
                };

                Source::new(s.position.into(), directivity, s.amplitude)
                    .with_name(s.name.clone())
                    .with_crossover(s.crossover.to_filter())
                    .with_delay_ms(s.delay_ms)
                    .with_phase_inversion(s.invert_phase)
            })
            .collect();

        console_log!("Created {} sources", sources.len());
        for source in &sources {
            if source.delay_sec > 0.0 || source.invert_phase {
                console_log!(
                    "  {}: delay={:.2}ms, invert={}",
                    source.name,
                    source.delay_sec * 1000.0,
                    source.invert_phase
                );
            }
        }

        let listening_position = config
            .listening_positions
            .first()
            .map(|p| (*p).into())
            .unwrap_or(Point3D::new(0.0, 0.0, 0.0));

        let frequencies = if config.frequencies.spacing == "linear" {
            lin_space(
                config.frequencies.min_freq,
                config.frequencies.max_freq,
                config.frequencies.num_points,
            )
        } else {
            log_space(
                config.frequencies.min_freq,
                config.frequencies.max_freq,
                config.frequencies.num_points,
            )
        };

        console_log!(
            "Frequency range: {:.1} - {:.1} Hz ({} points)",
            frequencies.first().unwrap_or(&0.0),
            frequencies.last().unwrap_or(&0.0),
            frequencies.len()
        );

        let speed_of_sound = config.solver.speed_of_sound;
        let wall_materials = config.wall_materials.clone();
        let temperature = config.solver.temperature;
        let humidity = config.solver.humidity;
        let air_absorption_enabled = config.solver.air_absorption;

        // Log wall materials
        console_log!(
            "Wall materials: Left={}, Right={}, Front={}, Back={}, Floor={}, Ceiling={}",
            wall_materials.get_material(WallSurface::Left).name,
            wall_materials.get_material(WallSurface::Right).name,
            wall_materials.get_material(WallSurface::Front).name,
            wall_materials.get_material(WallSurface::Back).name,
            wall_materials.get_material(WallSurface::Floor).name,
            wall_materials.get_material(WallSurface::Ceiling).name,
        );

        // Log environmental conditions
        if air_absorption_enabled {
            console_log!(
                "Air absorption enabled: T={:.1}°C, RH={:.0}%",
                temperature,
                humidity
            );
        }

        let edge_diffraction_enabled = config.solver.edge_diffraction;

        // Pre-compute diffraction edges for the room
        let diffraction_edges = if edge_diffraction_enabled {
            match &room_geometry {
                RoomGeometry::Rectangular(r) => {
                    console_log!("Edge diffraction enabled: computing 12 room edges");
                    get_rectangular_room_edges(r.width, r.depth, r.height)
                }
                RoomGeometry::LShaped(l) => {
                    // For L-shaped rooms, we have more edges (18 total)
                    // For now, use a simplified approximation with outer bounding box
                    console_log!("Edge diffraction enabled for L-shaped room (simplified)");
                    get_rectangular_room_edges(l.width1, l.depth1 + l.depth2, l.height)
                }
            }
        } else {
            Vec::new()
        };

        Ok(RoomSimulatorWasm {
            config,
            room_geometry,
            sources,
            listening_position,
            frequencies,
            speed_of_sound,
            wall_materials,
            temperature,
            humidity,
            air_absorption_enabled,
            edge_diffraction_enabled,
            diffraction_edges,
        })
    }

    fn wavenumber(&self, frequency: f64) -> f64 {
        2.0 * PI * frequency / self.speed_of_sound
    }

    /// Check if a point is inside the room geometry
    fn point_inside_room(&self, point: &Point3D) -> bool {
        match &self.room_geometry {
            RoomGeometry::Rectangular(r) => {
                point.x >= 0.0
                    && point.x <= r.width
                    && point.y >= 0.0
                    && point.y <= r.depth
                    && point.z >= 0.0
                    && point.z <= r.height
            }
            RoomGeometry::LShaped(l) => l.contains(point),
        }
    }

    /// Calculate air absorption attenuation factor for a given distance
    /// Returns a multiplier in the range (0, 1] where 1 = no attenuation
    fn air_absorption_factor(&self, distance: f64, frequency: f64) -> f64 {
        if !self.air_absorption_enabled || distance < 1e-6 {
            return 1.0;
        }

        let alpha = calculate_air_absorption(frequency, self.temperature, self.humidity);
        // Attenuation: exp(-alpha * distance)
        (-alpha * distance).exp()
    }

    fn calculate_direct_field(&self, point: &Point3D, frequency: f64) -> Complex64 {
        let (room_width, room_depth, room_height) = self.get_room_dimensions();

        // Handle different solver methods
        let method = self.config.solver.method.as_str();

        // BEM-based methods
        if method == "bem" || method == "hybrid-bem" {
            return self.calculate_bem_or_hybrid_bem(point, frequency, method);
        }

        // Modal and modal-ISM hybrid methods
        if method == "modal" || method == "hybrid" {
            // Modal analysis is only valid for rectangular rooms
            if let RoomGeometry::Rectangular(_) = &self.room_geometry {
                // Calculate modal pressure for all sources
                let mut modal_pressure = Complex64::new(0.0, 0.0);
                for source in &self.sources {
                    let amplitude = source.amplitude_towards(point, frequency);
                    let phase_factor = source.phase_factor(frequency);

                    let modal = calculate_modal_pressure(
                        &source.position,
                        point,
                        frequency,
                        room_width,
                        room_depth,
                        room_height,
                        self.config.solver.speed_of_sound,
                        self.config.solver.max_mode_order,
                        self.config.solver.modal_damping,
                    );

                    modal_pressure += modal * amplitude * phase_factor;
                }

                if method == "modal" {
                    // Pure modal: return modal pressure only
                    return modal_pressure;
                }

                // Hybrid: blend modal and ISM based on Schroeder frequency
                let volume = room_width * room_depth * room_height;
                let avg_absorption = self.wall_materials.average_absorption_at(frequency);
                let surface_area = 2.0
                    * (room_width * room_depth
                        + room_width * room_height
                        + room_depth * room_height);
                let total_abs = avg_absorption * surface_area;
                let rt60 = rt60_sabine(volume, total_abs);
                let schroeder_freq = 2000.0 * (rt60 / volume).sqrt();

                let ism_weight = hybrid_crossover_weight(
                    frequency,
                    schroeder_freq,
                    self.config.solver.hybrid_crossover_width,
                );

                // If fully in modal region, return modal pressure
                if ism_weight < 1e-6 {
                    return modal_pressure;
                }

                // Calculate ISM pressure and blend
                let ism_pressure = self.calculate_ism_field(point, frequency);

                // Blend: p = (1-w) * modal + w * ISM
                return modal_pressure * (1.0 - ism_weight) + ism_pressure * ism_weight;
            }
        }

        // Standard ISM calculation for non-hybrid modes
        self.calculate_ism_field(point, frequency)
    }

    /// Calculate field using BEM or hybrid BEM+ISM
    ///
    /// For "bem" mode: Uses modal analysis for rectangular rooms (captures room modes),
    /// falls back to BEM direct field + scattering for non-rectangular.
    ///
    /// For "hybrid-bem" mode: Blends modal/BEM at low frequencies with ISM at high
    /// frequencies using Schroeder frequency crossover.
    fn calculate_bem_or_hybrid_bem(
        &self,
        point: &Point3D,
        frequency: f64,
        method: &str,
    ) -> Complex64 {
        let (room_width, room_depth, room_height) = self.get_room_dimensions();

        // Calculate low-frequency pressure using modal analysis (rectangular) or BEM (other)
        let low_freq_pressure = if let RoomGeometry::Rectangular(_) = &self.room_geometry {
            // Modal analysis for rectangular rooms - captures room resonances accurately
            let mut modal_pressure = Complex64::new(0.0, 0.0);
            for source in &self.sources {
                let amplitude = source.amplitude_towards(point, frequency);
                let phase_factor = source.phase_factor(frequency);

                let modal = calculate_modal_pressure(
                    &source.position,
                    point,
                    frequency,
                    room_width,
                    room_depth,
                    room_height,
                    self.config.solver.speed_of_sound,
                    self.config.solver.max_mode_order,
                    self.config.solver.modal_damping,
                );

                modal_pressure += modal * amplitude * phase_factor;
            }

            // Add scattering object contributions if present
            if !self.config.scattering_objects.is_empty() {
                let scattering_result = bem_solver::solve_bem(
                    &self.room_geometry,
                    &self.sources,
                    &self.config.scattering_objects,
                    point,
                    frequency,
                    self.config.solver.speed_of_sound,
                    &self.config.solver.bem_config,
                );
                if let Ok(result) = scattering_result {
                    // Add scattered field contribution (subtract direct to avoid double counting)
                    let k =
                        2.0 * std::f64::consts::PI * frequency / self.config.solver.speed_of_sound;
                    let mut direct_field = Complex64::new(0.0, 0.0);
                    for source in &self.sources {
                        let amp = source.amplitude_towards(point, frequency);
                        let dist = source.position.distance_to(point);
                        direct_field += bem_solver::greens_function(dist, k)
                            * amp
                            * source.phase_factor(frequency);
                    }
                    // Scattered contribution = total BEM - direct
                    let scattered = result.pressure - direct_field;
                    modal_pressure += scattered;
                }
            }

            modal_pressure
        } else {
            // Non-rectangular rooms: use full BEM solver
            let bem_result = bem_solver::solve_bem(
                &self.room_geometry,
                &self.sources,
                &self.config.scattering_objects,
                point,
                frequency,
                self.config.solver.speed_of_sound,
                &self.config.solver.bem_config,
            );

            match bem_result {
                Ok(result) => result.pressure,
                Err(_) => return self.calculate_ism_field(point, frequency),
            }
        };

        if method == "bem" {
            // Pure BEM/modal: return low frequency pressure only
            return low_freq_pressure;
        }

        // Hybrid-BEM: blend modal/BEM at low frequencies with ISM at high frequencies
        // Use Schroeder frequency as the crossover point
        let volume = room_width * room_depth * room_height;
        let avg_absorption = self.wall_materials.average_absorption_at(frequency);
        let surface_area =
            2.0 * (room_width * room_depth + room_width * room_height + room_depth * room_height);
        let total_abs = avg_absorption * surface_area;
        let rt60 = rt60_sabine(volume, total_abs);
        let schroeder_freq = 2000.0 * (rt60 / volume).sqrt();

        // ISM weight increases above Schroeder frequency
        let ism_weight = hybrid_crossover_weight(
            frequency,
            schroeder_freq,
            self.config.solver.hybrid_crossover_width,
        );

        // If fully in modal/BEM region (below Schroeder), return low freq pressure
        if ism_weight < 1e-6 {
            return low_freq_pressure;
        }

        // If fully in ISM region (above Schroeder), return ISM pressure
        if ism_weight > 1.0 - 1e-6 {
            return self.calculate_ism_field(point, frequency);
        }

        // Blend: p = (1-w) * modal/BEM + w * ISM
        let ism_pressure = self.calculate_ism_field(point, frequency);
        low_freq_pressure * (1.0 - ism_weight) + ism_pressure * ism_weight
    }

    /// Calculate field using Image Source Method (ISM)
    fn calculate_ism_field(&self, point: &Point3D, frequency: f64) -> Complex64 {
        let k = self.wavenumber(frequency);
        let mut total_pressure = Complex64::new(0.0, 0.0);

        // Determine reflection order from solver method
        let reflection_order = match self.config.solver.method.as_str() {
            "direct" => 0,
            "image-source-1" => 1,
            "image-source-2" => 2,
            "image-source-3" => 3,
            "modal" => 0,      // Modal doesn't use ISM reflections
            "bem" => 0,        // BEM doesn't use ISM reflections
            "hybrid" => 2,     // Hybrid modal/ISM uses 2nd order
            "hybrid-bem" => 2, // Hybrid BEM/ISM uses 2nd order
            _ => 2,            // Default to 2nd order
        };

        // Get room dimensions for image source calculation
        let (room_width, room_depth, room_height) = self.get_room_dimensions();

        // Get frequency-dependent reflection coefficients for each wall
        let r_left = self
            .wall_materials
            .reflection_at(WallSurface::Left, frequency);
        let r_right = self
            .wall_materials
            .reflection_at(WallSurface::Right, frequency);
        let r_front = self
            .wall_materials
            .reflection_at(WallSurface::Front, frequency);
        let r_back = self
            .wall_materials
            .reflection_at(WallSurface::Back, frequency);
        let r_floor = self
            .wall_materials
            .reflection_at(WallSurface::Floor, frequency);
        let r_ceiling = self
            .wall_materials
            .reflection_at(WallSurface::Ceiling, frequency);

        for source in &self.sources {
            let amplitude = source.amplitude_towards(point, frequency);
            // Get phase factor for delay and phase inversion
            let phase_factor = source.phase_factor(frequency);

            // Direct sound (always included) with air absorption and phase
            let r_direct = source.position.distance_to(point);
            let air_atten_direct = self.air_absorption_factor(r_direct, frequency);
            total_pressure +=
                greens_function_3d(r_direct, k) * amplitude * air_atten_direct * phase_factor;

            if reflection_order >= 1 {
                // First-order image sources - handle L-shaped rooms specially
                let first_order_images: Vec<(Point3D, f64)> = match &self.room_geometry {
                    RoomGeometry::LShaped(l_room) => {
                        // Use L-shaped room's proper image source generation
                        l_room
                            .get_first_order_images(&source.position)
                            .into_iter()
                            .filter_map(|(image, wall_name)| {
                                // Validate the image source path
                                if !l_room.is_valid_image_source(&image, point, &source.position) {
                                    return None;
                                }
                                // Map wall name to reflection coefficient
                                let refl = match wall_name {
                                    "left" => r_left,
                                    "right" => r_right,
                                    "front" => r_front,
                                    "back" => r_back,
                                    "floor" => r_floor,
                                    "ceiling" => r_ceiling,
                                    "step_horizontal" | "step_vertical" => {
                                        // Interior step walls - use average of nearby walls
                                        (r_right + r_back) / 2.0
                                    }
                                    _ => r_right, // Default fallback
                                };
                                Some((image, refl))
                            })
                            .collect()
                    }
                    RoomGeometry::Rectangular(_) => {
                        // Standard rectangular room image sources (6 walls)
                        vec![
                            // Left wall (x=0)
                            (
                                Point3D::new(
                                    -source.position.x,
                                    source.position.y,
                                    source.position.z,
                                ),
                                r_left,
                            ),
                            // Right wall (x=width)
                            (
                                Point3D::new(
                                    2.0 * room_width - source.position.x,
                                    source.position.y,
                                    source.position.z,
                                ),
                                r_right,
                            ),
                            // Front wall (y=0)
                            (
                                Point3D::new(
                                    source.position.x,
                                    -source.position.y,
                                    source.position.z,
                                ),
                                r_front,
                            ),
                            // Back wall (y=depth)
                            (
                                Point3D::new(
                                    source.position.x,
                                    2.0 * room_depth - source.position.y,
                                    source.position.z,
                                ),
                                r_back,
                            ),
                            // Floor (z=0)
                            (
                                Point3D::new(
                                    source.position.x,
                                    source.position.y,
                                    -source.position.z,
                                ),
                                r_floor,
                            ),
                            // Ceiling (z=height)
                            (
                                Point3D::new(
                                    source.position.x,
                                    source.position.y,
                                    2.0 * room_height - source.position.z,
                                ),
                                r_ceiling,
                            ),
                        ]
                    }
                };

                for (image_src, refl_coeff) in &first_order_images {
                    let r_image = image_src.distance_to(point);
                    if r_image > 1e-6 {
                        let air_atten = self.air_absorption_factor(r_image, frequency);
                        total_pressure += greens_function_3d(r_image, k)
                            * amplitude
                            * refl_coeff
                            * air_atten
                            * phase_factor;
                    }
                }
            }

            // Higher-order reflections only for rectangular rooms
            // L-shaped room higher-order ISM is complex and would need proper path validation
            if let RoomGeometry::Rectangular(_) = &self.room_geometry {
                if reflection_order >= 2 {
                    // Second-order image sources (edges - 12 combinations)
                    // Each involves reflection off two walls, so multiply their coefficients
                    let second_order_images = [
                        // Left + Front (x=0, y=0)
                        (
                            Point3D::new(-source.position.x, -source.position.y, source.position.z),
                            r_left * r_front,
                        ),
                        // Left + Back (x=0, y=depth)
                        (
                            Point3D::new(
                                -source.position.x,
                                2.0 * room_depth - source.position.y,
                                source.position.z,
                            ),
                            r_left * r_back,
                        ),
                        // Right + Front (x=width, y=0)
                        (
                            Point3D::new(
                                2.0 * room_width - source.position.x,
                                -source.position.y,
                                source.position.z,
                            ),
                            r_right * r_front,
                        ),
                        // Right + Back (x=width, y=depth)
                        (
                            Point3D::new(
                                2.0 * room_width - source.position.x,
                                2.0 * room_depth - source.position.y,
                                source.position.z,
                            ),
                            r_right * r_back,
                        ),
                        // Left + Floor (x=0, z=0)
                        (
                            Point3D::new(-source.position.x, source.position.y, -source.position.z),
                            r_left * r_floor,
                        ),
                        // Left + Ceiling (x=0, z=height)
                        (
                            Point3D::new(
                                -source.position.x,
                                source.position.y,
                                2.0 * room_height - source.position.z,
                            ),
                            r_left * r_ceiling,
                        ),
                        // Right + Floor (x=width, z=0)
                        (
                            Point3D::new(
                                2.0 * room_width - source.position.x,
                                source.position.y,
                                -source.position.z,
                            ),
                            r_right * r_floor,
                        ),
                        // Right + Ceiling (x=width, z=height)
                        (
                            Point3D::new(
                                2.0 * room_width - source.position.x,
                                source.position.y,
                                2.0 * room_height - source.position.z,
                            ),
                            r_right * r_ceiling,
                        ),
                        // Front + Floor (y=0, z=0)
                        (
                            Point3D::new(source.position.x, -source.position.y, -source.position.z),
                            r_front * r_floor,
                        ),
                        // Front + Ceiling (y=0, z=height)
                        (
                            Point3D::new(
                                source.position.x,
                                -source.position.y,
                                2.0 * room_height - source.position.z,
                            ),
                            r_front * r_ceiling,
                        ),
                        // Back + Floor (y=depth, z=0)
                        (
                            Point3D::new(
                                source.position.x,
                                2.0 * room_depth - source.position.y,
                                -source.position.z,
                            ),
                            r_back * r_floor,
                        ),
                        // Back + Ceiling (y=depth, z=height)
                        (
                            Point3D::new(
                                source.position.x,
                                2.0 * room_depth - source.position.y,
                                2.0 * room_height - source.position.z,
                            ),
                            r_back * r_ceiling,
                        ),
                    ];

                    for (image_src, refl_coeff) in &second_order_images {
                        let r_image = image_src.distance_to(point);
                        if r_image > 1e-6 {
                            let air_atten = self.air_absorption_factor(r_image, frequency);
                            total_pressure += greens_function_3d(r_image, k)
                                * amplitude
                                * refl_coeff
                                * air_atten
                                * phase_factor;
                        }
                    }
                }

                if reflection_order >= 3 {
                    // Third-order image sources (corners - 8 combinations)
                    // Each involves reflection off three walls
                    let third_order_images = [
                        // Left + Front + Floor
                        (
                            Point3D::new(
                                -source.position.x,
                                -source.position.y,
                                -source.position.z,
                            ),
                            r_left * r_front * r_floor,
                        ),
                        // Left + Front + Ceiling
                        (
                            Point3D::new(
                                -source.position.x,
                                -source.position.y,
                                2.0 * room_height - source.position.z,
                            ),
                            r_left * r_front * r_ceiling,
                        ),
                        // Left + Back + Floor
                        (
                            Point3D::new(
                                -source.position.x,
                                2.0 * room_depth - source.position.y,
                                -source.position.z,
                            ),
                            r_left * r_back * r_floor,
                        ),
                        // Left + Back + Ceiling
                        (
                            Point3D::new(
                                -source.position.x,
                                2.0 * room_depth - source.position.y,
                                2.0 * room_height - source.position.z,
                            ),
                            r_left * r_back * r_ceiling,
                        ),
                        // Right + Front + Floor
                        (
                            Point3D::new(
                                2.0 * room_width - source.position.x,
                                -source.position.y,
                                -source.position.z,
                            ),
                            r_right * r_front * r_floor,
                        ),
                        // Right + Front + Ceiling
                        (
                            Point3D::new(
                                2.0 * room_width - source.position.x,
                                -source.position.y,
                                2.0 * room_height - source.position.z,
                            ),
                            r_right * r_front * r_ceiling,
                        ),
                        // Right + Back + Floor
                        (
                            Point3D::new(
                                2.0 * room_width - source.position.x,
                                2.0 * room_depth - source.position.y,
                                -source.position.z,
                            ),
                            r_right * r_back * r_floor,
                        ),
                        // Right + Back + Ceiling
                        (
                            Point3D::new(
                                2.0 * room_width - source.position.x,
                                2.0 * room_depth - source.position.y,
                                2.0 * room_height - source.position.z,
                            ),
                            r_right * r_back * r_ceiling,
                        ),
                    ];

                    for (image_src, refl_coeff) in &third_order_images {
                        let r_image = image_src.distance_to(point);
                        if r_image > 1e-6 {
                            let air_atten = self.air_absorption_factor(r_image, frequency);
                            total_pressure += greens_function_3d(r_image, k)
                                * amplitude
                                * refl_coeff
                                * air_atten
                                * phase_factor;
                        }
                    }
                }
            }

            // Add edge diffraction contributions
            if self.edge_diffraction_enabled && !self.diffraction_edges.is_empty() {
                for edge in &self.diffraction_edges {
                    let diff_contrib = edge.diffraction_contribution(&source.position, point, k);
                    // Scale diffraction by source amplitude and phase
                    total_pressure += diff_contrib * amplitude * phase_factor;
                }
            }
        }

        total_pressure
    }

    /// Calculate field from a single source using the configured solver method
    /// This mirrors calculate_direct_field but for a single source only
    fn calculate_source_field(
        &self,
        source_idx: usize,
        point: &Point3D,
        frequency: f64,
    ) -> Complex64 {
        if source_idx >= self.sources.len() {
            return Complex64::new(0.0, 0.0);
        }

        let (room_width, room_depth, room_height) = self.get_room_dimensions();
        let method = self.config.solver.method.as_str();
        let source = &self.sources[source_idx];

        // BEM-based methods - use modal for single source
        if method == "bem" || method == "hybrid-bem" {
            return self
                .calculate_single_source_bem_or_hybrid(source_idx, point, frequency, method);
        }

        // Modal and modal-ISM hybrid methods
        if method == "modal" || method == "hybrid" {
            if let RoomGeometry::Rectangular(_) = &self.room_geometry {
                let amplitude = source.amplitude_towards(point, frequency);
                let phase_factor = source.phase_factor(frequency);

                let modal_pressure = calculate_modal_pressure(
                    &source.position,
                    point,
                    frequency,
                    room_width,
                    room_depth,
                    room_height,
                    self.config.solver.speed_of_sound,
                    self.config.solver.max_mode_order,
                    self.config.solver.modal_damping,
                ) * amplitude
                    * phase_factor;

                if method == "modal" {
                    return modal_pressure;
                }

                // Hybrid: blend modal and ISM based on Schroeder frequency
                let volume = room_width * room_depth * room_height;
                let avg_absorption = self.wall_materials.average_absorption_at(frequency);
                let surface_area = 2.0
                    * (room_width * room_depth
                        + room_width * room_height
                        + room_depth * room_height);
                let total_abs = avg_absorption * surface_area;
                let rt60 = rt60_sabine(volume, total_abs);
                let schroeder_freq = 2000.0 * (rt60 / volume).sqrt();

                let ism_weight = hybrid_crossover_weight(
                    frequency,
                    schroeder_freq,
                    self.config.solver.hybrid_crossover_width,
                );

                if ism_weight < 1e-6 {
                    return modal_pressure;
                }

                let ism_pressure = self.calculate_single_source_ism(source_idx, point, frequency);
                return modal_pressure * (1.0 - ism_weight) + ism_pressure * ism_weight;
            }
        }

        // Standard ISM calculation
        self.calculate_single_source_ism(source_idx, point, frequency)
    }

    /// Calculate field from a single source using BEM or hybrid BEM+ISM
    fn calculate_single_source_bem_or_hybrid(
        &self,
        source_idx: usize,
        point: &Point3D,
        frequency: f64,
        method: &str,
    ) -> Complex64 {
        let (room_width, room_depth, room_height) = self.get_room_dimensions();
        let source = &self.sources[source_idx];

        // For rectangular rooms, use modal analysis for the single source
        let low_freq_pressure = if let RoomGeometry::Rectangular(_) = &self.room_geometry {
            let amplitude = source.amplitude_towards(point, frequency);
            let phase_factor = source.phase_factor(frequency);

            calculate_modal_pressure(
                &source.position,
                point,
                frequency,
                room_width,
                room_depth,
                room_height,
                self.config.solver.speed_of_sound,
                self.config.solver.max_mode_order,
                self.config.solver.modal_damping,
            ) * amplitude
                * phase_factor
        } else {
            // Non-rectangular: use direct field approximation
            let k = self.wavenumber(frequency);
            let amplitude = source.amplitude_towards(point, frequency);
            let dist = source.position.distance_to(point);
            let phase_factor = source.phase_factor(frequency);
            greens_function_3d(dist, k) * amplitude * phase_factor
        };

        if method == "bem" {
            return low_freq_pressure;
        }

        // Hybrid-BEM: blend with ISM at high frequencies
        let volume = room_width * room_depth * room_height;
        let avg_absorption = self.wall_materials.average_absorption_at(frequency);
        let surface_area =
            2.0 * (room_width * room_depth + room_width * room_height + room_depth * room_height);
        let total_abs = avg_absorption * surface_area;
        let rt60 = rt60_sabine(volume, total_abs);
        let schroeder_freq = 2000.0 * (rt60 / volume).sqrt();

        let ism_weight = hybrid_crossover_weight(
            frequency,
            schroeder_freq,
            self.config.solver.hybrid_crossover_width,
        );

        if ism_weight < 1e-6 {
            return low_freq_pressure;
        }

        if ism_weight > 1.0 - 1e-6 {
            return self.calculate_single_source_ism(source_idx, point, frequency);
        }

        let ism_pressure = self.calculate_single_source_ism(source_idx, point, frequency);
        low_freq_pressure * (1.0 - ism_weight) + ism_pressure * ism_weight
    }

    /// Calculate field from a single source using ISM
    fn calculate_single_source_ism(
        &self,
        source_idx: usize,
        point: &Point3D,
        frequency: f64,
    ) -> Complex64 {
        let k = self.wavenumber(frequency);
        let source = &self.sources[source_idx];
        let amplitude = source.amplitude_towards(point, frequency);
        let phase_factor = source.phase_factor(frequency);

        // Determine reflection order from solver method
        let reflection_order = match self.config.solver.method.as_str() {
            "direct" => 0,
            "image-source-1" => 1,
            "image-source-2" => 2,
            "image-source-3" => 3,
            "modal" => 0,
            "bem" => 0,
            "hybrid" => 2,
            "hybrid-bem" => 2,
            _ => 2,
        };

        let (room_width, room_depth, room_height) = self.get_room_dimensions();

        // Get frequency-dependent reflection coefficients
        let r_left = self
            .wall_materials
            .reflection_at(WallSurface::Left, frequency);
        let r_right = self
            .wall_materials
            .reflection_at(WallSurface::Right, frequency);
        let r_front = self
            .wall_materials
            .reflection_at(WallSurface::Front, frequency);
        let r_back = self
            .wall_materials
            .reflection_at(WallSurface::Back, frequency);
        let r_floor = self
            .wall_materials
            .reflection_at(WallSurface::Floor, frequency);
        let r_ceiling = self
            .wall_materials
            .reflection_at(WallSurface::Ceiling, frequency);

        let mut total_pressure = Complex64::new(0.0, 0.0);

        // Direct sound
        let r_direct = source.position.distance_to(point);
        let air_atten_direct = self.air_absorption_factor(r_direct, frequency);
        total_pressure +=
            greens_function_3d(r_direct, k) * amplitude * air_atten_direct * phase_factor;

        if reflection_order >= 1 {
            // First-order image sources - handle L-shaped rooms
            let first_order_images: Vec<(Point3D, f64)> = match &self.room_geometry {
                RoomGeometry::LShaped(l_room) => l_room
                    .get_first_order_images(&source.position)
                    .into_iter()
                    .filter_map(|(image, wall_name)| {
                        if !l_room.is_valid_image_source(&image, point, &source.position) {
                            return None;
                        }
                        let refl = match wall_name {
                            "left" => r_left,
                            "right" => r_right,
                            "front" => r_front,
                            "back" => r_back,
                            "floor" => r_floor,
                            "ceiling" => r_ceiling,
                            "step_horizontal" | "step_vertical" => (r_right + r_back) / 2.0,
                            _ => r_right,
                        };
                        Some((image, refl))
                    })
                    .collect(),
                RoomGeometry::Rectangular(_) => {
                    vec![
                        (
                            Point3D::new(-source.position.x, source.position.y, source.position.z),
                            r_left,
                        ),
                        (
                            Point3D::new(
                                2.0 * room_width - source.position.x,
                                source.position.y,
                                source.position.z,
                            ),
                            r_right,
                        ),
                        (
                            Point3D::new(source.position.x, -source.position.y, source.position.z),
                            r_front,
                        ),
                        (
                            Point3D::new(
                                source.position.x,
                                2.0 * room_depth - source.position.y,
                                source.position.z,
                            ),
                            r_back,
                        ),
                        (
                            Point3D::new(source.position.x, source.position.y, -source.position.z),
                            r_floor,
                        ),
                        (
                            Point3D::new(
                                source.position.x,
                                source.position.y,
                                2.0 * room_height - source.position.z,
                            ),
                            r_ceiling,
                        ),
                    ]
                }
            };

            for (image_src, refl_coeff) in first_order_images {
                let r_image = image_src.distance_to(point);
                if r_image > 1e-6 {
                    let air_atten = self.air_absorption_factor(r_image, frequency);
                    total_pressure += greens_function_3d(r_image, k)
                        * amplitude
                        * refl_coeff
                        * air_atten
                        * phase_factor;
                }
            }

            // Higher-order reflections for rectangular rooms only
            if reflection_order >= 2 {
                if let RoomGeometry::Rectangular(_) = &self.room_geometry {
                    for nx in -2i32..=2 {
                        for ny in -2i32..=2 {
                            for nz in -2i32..=2 {
                                let order = nx.abs() + ny.abs() + nz.abs();
                                if order < 2 || order > reflection_order {
                                    continue;
                                }

                                let image_x = if nx % 2 == 0 {
                                    source.position.x + (nx as f64) * room_width
                                } else {
                                    -source.position.x + (nx as f64 + 1.0) * room_width
                                };
                                let image_y = if ny % 2 == 0 {
                                    source.position.y + (ny as f64) * room_depth
                                } else {
                                    -source.position.y + (ny as f64 + 1.0) * room_depth
                                };
                                let image_z = if nz % 2 == 0 {
                                    source.position.z + (nz as f64) * room_height
                                } else {
                                    -source.position.z + (nz as f64 + 1.0) * room_height
                                };

                                let image_pos = Point3D::new(image_x, image_y, image_z);
                                let r_image = image_pos.distance_to(point);

                                if r_image > 1e-6 {
                                    let refl_coeff = r_left.powi((nx.abs() + 1) / 2)
                                        * r_right.powi(nx.abs() / 2)
                                        * r_front.powi((ny.abs() + 1) / 2)
                                        * r_back.powi(ny.abs() / 2)
                                        * r_floor.powi((nz.abs() + 1) / 2)
                                        * r_ceiling.powi(nz.abs() / 2);

                                    let air_atten = self.air_absorption_factor(r_image, frequency);
                                    total_pressure += greens_function_3d(r_image, k)
                                        * amplitude
                                        * refl_coeff
                                        * air_atten
                                        * phase_factor;
                                }
                            }
                        }
                    }
                }
            }
        }

        total_pressure
    }

    /// Run the full simulation and return JSON results
    #[wasm_bindgen]
    pub fn run_simulation(&self) -> Result<String, JsValue> {
        console_log!("Starting simulation...");

        let compute_ir = self.config.visualization.generate_impulse_response;
        let compute_binaural = self.config.visualization.binaural.enabled;

        // Precompute ear positions for binaural
        let (left_ear_pos, right_ear_pos) = if compute_binaural {
            let binaural_config = &self.config.visualization.binaural;
            let head_center = binaural_config
                .head_position
                .as_ref()
                .map(|p| Point3D::new(p.x, p.y, p.z))
                .unwrap_or(self.listening_position);
            calculate_ear_positions(
                &head_center,
                binaural_config.head_yaw,
                binaural_config.ear_spacing,
            )
        } else {
            (self.listening_position, self.listening_position)
        };

        // Precompute head center and yaw for HRTF
        let head_center_for_hrtf = self
            .config
            .visualization
            .binaural
            .head_position
            .as_ref()
            .map(|p| Point3D::new(p.x, p.y, p.z))
            .unwrap_or(self.listening_position);
        let head_yaw = self.config.visualization.binaural.head_yaw;

        console_log!(
            "Computing frequency response using {} threads...",
            rayon::current_num_threads()
        );

        // Parallel computation of frequency response
        // Each frequency is computed independently, results collected in order
        let freq_results: Vec<_> = self
            .frequencies
            .par_iter()
            .enumerate()
            .map(|(idx, &freq)| {
                // Main pressure at listening position
                let pressure = self.calculate_direct_field(&self.listening_position, freq);
                let spl = pressure_to_spl(pressure);

                // Per-source SPL values
                let source_spls: Vec<f64> = (0..self.sources.len())
                    .map(|src_idx| {
                        let p =
                            self.calculate_source_field(src_idx, &self.listening_position, freq);
                        pressure_to_spl(p)
                    })
                    .collect();

                // Binaural computation if enabled
                let binaural = if compute_binaural {
                    let left_pressure = self.calculate_direct_field(&left_ear_pos, freq);
                    let right_pressure = self.calculate_direct_field(&right_ear_pos, freq);

                    // Apply simplified HRTF magnitude correction
                    let (mut left_total, mut right_total) = (left_pressure, right_pressure);
                    for source in &self.sources {
                        let (left_gain, right_gain) = approximate_hrtf_magnitude(
                            &source.position,
                            &head_center_for_hrtf,
                            head_yaw,
                            freq,
                        );
                        left_total *= Complex64::new(left_gain, 0.0);
                        right_total *= Complex64::new(right_gain, 0.0);
                    }
                    Some((left_total, right_total))
                } else {
                    None
                };

                (idx, spl, pressure, source_spls, binaural)
            })
            .collect();

        // Unpack parallel results into separate vectors (maintaining order)
        let mut combined_spl = Vec::with_capacity(self.frequencies.len());
        let mut complex_pressures = Vec::with_capacity(self.frequencies.len());
        let mut source_responses: Vec<Vec<f64>> = self
            .sources
            .iter()
            .map(|_| Vec::with_capacity(self.frequencies.len()))
            .collect();
        let mut left_ear_pressures = Vec::with_capacity(self.frequencies.len());
        let mut right_ear_pressures = Vec::with_capacity(self.frequencies.len());

        for (_idx, spl, pressure, source_spls, binaural) in freq_results {
            combined_spl.push(spl);

            if compute_ir {
                complex_pressures.push(pressure);
            }

            for (src_idx, src_spl) in source_spls.into_iter().enumerate() {
                source_responses[src_idx].push(src_spl);
            }

            if let Some((left, right)) = binaural {
                left_ear_pressures.push(left);
                right_ear_pressures.push(right);
            }
        }

        console_log!("Frequency response computed");

        let source_responses_output: Vec<SourceResponse> = self
            .sources
            .iter()
            .zip(source_responses)
            .map(|(source, spl)| SourceResponse {
                source_name: source.name.clone(),
                spl,
            })
            .collect();

        let (horizontal_slices, vertical_slices) = if self.config.visualization.generate_slices {
            console_log!("Computing spatial slices...");
            self.compute_slices()
        } else {
            (None, None)
        };

        let room_output = self.build_room_output();

        // Calculate room modes and acoustics for rectangular rooms only
        let (room_modes, room_acoustics) = match &self.config.room {
            RoomGeometryConfig::Rectangular {
                width,
                depth,
                height,
            } => {
                let max_freq = self.frequencies.last().copied().unwrap_or(500.0);
                let modes = calculate_room_modes(
                    *width,
                    *depth,
                    *height,
                    self.config.solver.speed_of_sound,
                    max_freq,
                    10, // max order
                );
                // Calculate room acoustics at 1kHz (mid-frequency reference)
                let acoustics =
                    calculate_room_acoustics(*width, *depth, *height, &self.wall_materials, 1000.0);
                (Some(modes), Some(acoustics))
            }
            _ => (None, None), // L-shaped and complex rooms don't have simple calculations
        };

        // Calculate impulse response if enabled
        let impulse_response = if compute_ir && !complex_pressures.is_empty() {
            console_log!("Computing impulse response...");
            let ir_config = &self.config.visualization.impulse_response;

            // If duration not set, use RT60 + 100ms if available, otherwise default
            let ir_duration = ir_config
                .duration
                .or_else(|| room_acoustics.as_ref().map(|ra| ra.rt60_eyring + 0.1));

            let config_with_duration = ImpulseResponseConfig {
                duration: ir_duration,
                ..ir_config.clone()
            };

            Some(calculate_impulse_response(
                &self.frequencies,
                &complex_pressures,
                &config_with_duration,
            ))
        } else {
            None
        };

        // Calculate binaural response if enabled
        let binaural_response = if compute_binaural && !left_ear_pressures.is_empty() {
            console_log!("Computing binaural response...");
            let binaural_config = &self.config.visualization.binaural;
            let head_center = binaural_config
                .head_position
                .as_ref()
                .map(|p| Point3D::new(p.x, p.y, p.z))
                .unwrap_or(self.listening_position);

            Some(calculate_binaural_response(
                &self.frequencies,
                &left_ear_pressures,
                &right_ear_pressures,
                binaural_config,
                self.config.solver.speed_of_sound,
                head_center,
            ))
        } else {
            None
        };

        let sources_output: Vec<SourceOutputInfo> = self
            .sources
            .iter()
            .zip(self.config.sources.iter())
            .map(|(source, config)| {
                let crossover_str = match &config.crossover {
                    CrossoverConfig::FullRange => None,
                    CrossoverConfig::Lowpass { cutoff_freq, order } => {
                        Some(format!("LP {}Hz {}nd", cutoff_freq, order))
                    }
                    CrossoverConfig::Highpass { cutoff_freq, order } => {
                        Some(format!("HP {}Hz {}nd", cutoff_freq, order))
                    }
                    CrossoverConfig::Bandpass {
                        low_cutoff,
                        high_cutoff,
                        order,
                    } => Some(format!("BP {}-{}Hz {}nd", low_cutoff, high_cutoff, order)),
                };

                SourceOutputInfo {
                    name: source.name.clone(),
                    position: [source.position.x, source.position.y, source.position.z],
                    crossover: crossover_str,
                }
            })
            .collect();

        let results = SimulationResults {
            room: room_output,
            sources: sources_output,
            listening_position: [
                self.listening_position.x,
                self.listening_position.y,
                self.listening_position.z,
            ],
            frequencies: self.frequencies.clone(),
            frequency_response: combined_spl,
            source_responses: Some(source_responses_output),
            horizontal_slices,
            vertical_slices,
            solver: self.config.solver.method.clone(),
            mesh_nodes: None,
            mesh_elements: None,
            metadata: Some(self.config.metadata.clone()),
            room_modes,
            room_acoustics,
            impulse_response,
            binaural_response,
        };

        console_log!("Simulation complete!");

        serde_json::to_string(&results)
            .map_err(|e| JsValue::from_str(&format!("JSON serialization error: {}", e)))
    }

    /// Compute a single frequency response (for progressive updates)
    #[wasm_bindgen]
    pub fn compute_frequency_point(&self, freq_index: usize) -> Result<String, JsValue> {
        if freq_index >= self.frequencies.len() {
            return Err(JsValue::from_str("Frequency index out of bounds"));
        }

        let freq = self.frequencies[freq_index];
        let pressure = self.calculate_direct_field(&self.listening_position, freq);
        let spl = pressure_to_spl(pressure);

        let result = serde_json::json!({
            "frequency": freq,
            "spl": spl,
            "index": freq_index,
            "total": self.frequencies.len()
        });

        serde_json::to_string(&result).map_err(|e| JsValue::from_str(&format!("JSON error: {}", e)))
    }

    /// Compute a horizontal slice at a specific frequency
    #[wasm_bindgen]
    pub fn compute_horizontal_slice(&self, frequency: f64) -> Result<String, JsValue> {
        console_log!("Computing horizontal slice at {:.1} Hz", frequency);

        let resolution = self.config.visualization.slice_resolution;
        let (room_width, room_depth, _) = self.get_room_dimensions();

        let x_points = lin_space(0.0, room_width, resolution);
        let y_points = lin_space(0.0, room_depth, resolution);

        let mut spl_values = Vec::with_capacity(resolution * resolution);

        for &y in &y_points {
            for &x in &x_points {
                let point = Point3D::new(x, y, self.listening_position.z);
                let pressure = self.calculate_direct_field(&point, frequency);
                spl_values.push(pressure_to_spl(pressure));
            }
        }

        let result = SliceOutput {
            frequency,
            x: x_points,
            y: y_points,
            z: None,
            spl: spl_values,
            shape: [resolution, resolution],
        };

        serde_json::to_string(&result).map_err(|e| JsValue::from_str(&format!("JSON error: {}", e)))
    }

    /// Compute a vertical slice at a specific frequency
    #[wasm_bindgen]
    pub fn compute_vertical_slice(&self, frequency: f64) -> Result<String, JsValue> {
        console_log!("Computing vertical slice at {:.1} Hz", frequency);

        let resolution = self.config.visualization.slice_resolution;
        let (room_width, _, room_height) = self.get_room_dimensions();

        let x_points = lin_space(0.0, room_width, resolution);
        let z_points = lin_space(0.0, room_height, resolution);

        let mut spl_values = Vec::with_capacity(resolution * resolution);

        for &z in &z_points {
            for &x in &x_points {
                let point = Point3D::new(x, self.listening_position.y, z);
                let pressure = self.calculate_direct_field(&point, frequency);
                spl_values.push(pressure_to_spl(pressure));
            }
        }

        let result = SliceOutput {
            frequency,
            x: x_points,
            y: Vec::new(),
            z: Some(z_points),
            spl: spl_values,
            shape: [resolution, resolution],
        };

        serde_json::to_string(&result).map_err(|e| JsValue::from_str(&format!("JSON error: {}", e)))
    }

    /// Get room info as JSON
    #[wasm_bindgen]
    pub fn get_room_info(&self) -> String {
        let room_output = self.build_room_output();
        serde_json::to_string(&room_output).unwrap_or_else(|_| "{}".to_string())
    }

    /// Get configuration as JSON
    #[wasm_bindgen]
    pub fn get_config(&self) -> String {
        serde_json::to_string(&self.config).unwrap_or_else(|_| "{}".to_string())
    }

    /// Get number of frequencies
    #[wasm_bindgen]
    pub fn num_frequencies(&self) -> usize {
        self.frequencies.len()
    }

    /// Get number of sources
    #[wasm_bindgen]
    pub fn num_sources(&self) -> usize {
        self.sources.len()
    }

    fn get_room_dimensions(&self) -> (f64, f64, f64) {
        match &self.room_geometry {
            RoomGeometry::Rectangular(r) => (r.width, r.depth, r.height),
            RoomGeometry::LShaped(r) => (r.width1, r.depth1 + r.depth2, r.height),
        }
    }

    fn build_room_output(&self) -> RoomOutput {
        let edges = self.room_geometry.get_edges();
        let edges_arrays: Vec<[[f64; 3]; 2]> = edges
            .iter()
            .map(|(p1, p2)| [[p1.x, p1.y, p1.z], [p2.x, p2.y, p2.z]])
            .collect();

        match &self.room_geometry {
            RoomGeometry::Rectangular(r) => RoomOutput {
                width: r.width,
                depth: r.depth,
                height: r.height,
                room_type: Some("rectangular".to_string()),
                edges: edges_arrays,
            },
            RoomGeometry::LShaped(r) => RoomOutput {
                width: r.width1,
                depth: r.depth1 + r.depth2,
                height: r.height,
                room_type: Some("lshaped".to_string()),
                edges: edges_arrays,
            },
        }
    }

    fn compute_slices(&self) -> (Option<Vec<SliceOutput>>, Option<Vec<SliceOutput>>) {
        let resolution = self.config.visualization.slice_resolution;
        let (room_width, room_depth, room_height) = self.get_room_dimensions();

        let x_points = lin_space(0.0, room_width, resolution);
        let y_points = lin_space(0.0, room_depth, resolution);
        let z_points = lin_space(0.0, room_height, resolution);

        let freq_indices: Vec<usize> =
            if self.config.visualization.slice_frequency_indices.is_empty() {
                let step = (self.frequencies.len() / 10).max(1);
                (0..self.frequencies.len()).step_by(step).collect()
            } else {
                self.config
                    .visualization
                    .slice_frequency_indices
                    .iter()
                    .filter(|&&i| i < self.frequencies.len())
                    .copied()
                    .collect()
            };

        console_log!(
            "Computing {} spatial slices in parallel...",
            freq_indices.len()
        );

        // Parallel computation of slices - each frequency slice is independent
        let slices: Vec<(SliceOutput, SliceOutput)> = freq_indices
            .par_iter()
            .map(|&freq_idx| {
                let freq = self.frequencies[freq_idx];

                // Horizontal slice - parallelize the grid computation
                // For L-shaped rooms, points outside the room boundary get very low SPL
                let h_spl: Vec<f64> = y_points
                    .iter()
                    .flat_map(|&y| {
                        x_points.iter().map(move |&x| {
                            let point = Point3D::new(x, y, self.listening_position.z);
                            // Check if point is inside the room
                            if !self.point_inside_room(&point) {
                                return -100.0; // Very low SPL for points outside room
                            }
                            let pressure = self.calculate_direct_field(&point, freq);
                            pressure_to_spl(pressure)
                        })
                    })
                    .collect();

                let h_slice = SliceOutput {
                    frequency: freq,
                    x: x_points.clone(),
                    y: y_points.clone(),
                    z: None,
                    spl: h_spl,
                    shape: [resolution, resolution],
                };

                // Vertical slice - parallelize the grid computation
                // For L-shaped rooms, points outside the room boundary get very low SPL
                let v_spl: Vec<f64> = z_points
                    .iter()
                    .flat_map(|&z| {
                        x_points.iter().map(move |&x| {
                            let point = Point3D::new(x, self.listening_position.y, z);
                            // Check if point is inside the room
                            if !self.point_inside_room(&point) {
                                return -100.0; // Very low SPL for points outside room
                            }
                            let pressure = self.calculate_direct_field(&point, freq);
                            pressure_to_spl(pressure)
                        })
                    })
                    .collect();

                let v_slice = SliceOutput {
                    frequency: freq,
                    x: x_points.clone(),
                    y: Vec::new(),
                    z: Some(z_points.clone()),
                    spl: v_spl,
                    shape: [resolution, resolution],
                };

                (h_slice, v_slice)
            })
            .collect();

        // Unzip into separate vectors
        let (horizontal_slices, vertical_slices): (Vec<_>, Vec<_>) = slices.into_iter().unzip();

        console_log!("Spatial slices computed");

        (Some(horizontal_slices), Some(vertical_slices))
    }
}

// ============================================================================
// Standalone utility functions for JS
// ============================================================================

/// Create a default configuration JSON
#[wasm_bindgen]
pub fn create_default_config() -> String {
    let config = SimulationConfig {
        room: RoomGeometryConfig::Rectangular {
            width: 5.0,
            depth: 4.0,
            height: 2.5,
        },
        sources: vec![
            SourceConfig {
                name: "Left Speaker".to_string(),
                position: Point3DConfig {
                    x: 1.5,
                    y: 0.5,
                    z: 1.2,
                },
                amplitude: 1.0,
                directivity: DirectivityConfig::Omnidirectional,
                crossover: CrossoverConfig::FullRange,
                delay_ms: 0.0,
                invert_phase: false,
            },
            SourceConfig {
                name: "Right Speaker".to_string(),
                position: Point3DConfig {
                    x: 3.5,
                    y: 0.5,
                    z: 1.2,
                },
                amplitude: 1.0,
                directivity: DirectivityConfig::Omnidirectional,
                crossover: CrossoverConfig::FullRange,
                delay_ms: 0.0,
                invert_phase: false,
            },
        ],
        listening_positions: vec![Point3DConfig {
            x: 2.5,
            y: 2.5,
            z: 1.2,
        }],
        frequencies: FrequencyConfig {
            min_freq: 20.0,
            max_freq: 500.0,
            num_points: 100,
            spacing: "logarithmic".to_string(),
        },
        solver: SolverConfig::default(),
        visualization: VisualizationConfig::default(),
        wall_materials: WallMaterialsConfig::default(),
        metadata: MetadataConfig {
            description: "Default stereo setup".to_string(),
            author: "Room Simulator WASM".to_string(),
            date: String::new(),
            notes: String::new(),
        },
        scattering_objects: Vec::new(),
    };

    serde_json::to_string_pretty(&config).unwrap_or_else(|_| "{}".to_string())
}

/// Get a list of available wall material presets
#[wasm_bindgen]
pub fn get_material_presets() -> String {
    let presets = vec![
        (
            "concrete",
            "Concrete/Brick (painted)",
            [0.01, 0.01, 0.02, 0.02, 0.02, 0.03],
        ),
        (
            "brick",
            "Unpainted Brick",
            [0.03, 0.03, 0.03, 0.04, 0.05, 0.07],
        ),
        (
            "drywall",
            "Drywall/Gypsum",
            [0.29, 0.10, 0.05, 0.04, 0.07, 0.09],
        ),
        (
            "plaster",
            "Plaster on Brick",
            [0.01, 0.02, 0.02, 0.03, 0.04, 0.05],
        ),
        (
            "glass",
            "Glass (large pane)",
            [0.18, 0.06, 0.04, 0.03, 0.02, 0.02],
        ),
        (
            "wood_thin",
            "Wood Paneling (thin)",
            [0.42, 0.21, 0.10, 0.08, 0.06, 0.06],
        ),
        (
            "wood_thick",
            "Heavy Wood/Door",
            [0.14, 0.10, 0.06, 0.08, 0.10, 0.10],
        ),
        (
            "carpet_thin",
            "Carpet (thin)",
            [0.02, 0.06, 0.14, 0.37, 0.60, 0.65],
        ),
        (
            "carpet_thick",
            "Carpet (heavy)",
            [0.08, 0.24, 0.57, 0.69, 0.71, 0.73],
        ),
        (
            "acoustic_tile",
            "Acoustic Ceiling Tiles",
            [0.50, 0.70, 0.60, 0.70, 0.70, 0.50],
        ),
        (
            "curtains",
            "Curtains/Drapes",
            [0.07, 0.31, 0.49, 0.75, 0.70, 0.60],
        ),
        (
            "acoustic_foam",
            "Acoustic Foam",
            [0.08, 0.25, 0.60, 0.90, 0.95, 0.90],
        ),
        (
            "hardwood",
            "Hardwood Floor",
            [0.15, 0.11, 0.10, 0.07, 0.06, 0.07],
        ),
        (
            "concrete_floor",
            "Concrete Floor",
            [0.01, 0.01, 0.02, 0.02, 0.02, 0.02],
        ),
    ];

    let result: Vec<serde_json::Value> = presets
        .iter()
        .map(|(id, name, absorption)| {
            serde_json::json!({
                "id": id,
                "name": name,
                "absorption": {
                    "125": absorption[0],
                    "250": absorption[1],
                    "500": absorption[2],
                    "1000": absorption[3],
                    "2000": absorption[4],
                    "4000": absorption[5],
                }
            })
        })
        .collect();

    serde_json::to_string(&result).unwrap_or_else(|_| "[]".to_string())
}

/// Calculate room modes for a rectangular room (standalone WASM function)
///
/// Returns JSON with mode information:
/// - frequency: Resonant frequency in Hz
/// - indices: [n, m, p] mode indices
/// - mode_type: "axial", "tangential", or "oblique"
/// - description: Human-readable description
///
/// # Arguments
/// * `width` - Room width (X dimension) in meters
/// * `depth` - Room depth (Y dimension) in meters
/// * `height` - Room height (Z dimension) in meters
/// * `speed_of_sound` - Speed of sound in m/s (typically 343)
/// * `max_frequency` - Maximum frequency to calculate modes up to (Hz)
#[wasm_bindgen]
pub fn get_room_modes(
    width: f64,
    depth: f64,
    height: f64,
    speed_of_sound: f64,
    max_frequency: f64,
) -> String {
    let modes = calculate_room_modes(width, depth, height, speed_of_sound, max_frequency, 10);
    serde_json::to_string(&modes).unwrap_or_else(|_| "[]".to_string())
}

/// Get the Schroeder frequency for a room
///
/// The Schroeder frequency marks the transition between modal behavior (below)
/// and statistical behavior (above). Below this frequency, individual room modes
/// dominate the response. Above it, the modal density is high enough for
/// statistical acoustics methods to be valid.
///
/// Formula: f_s = 2000 * sqrt(RT60 / V)
/// Where RT60 is reverberation time (seconds) and V is room volume (m³)
///
/// For rooms with unknown RT60, a rough estimate can be made:
/// f_s ≈ 2000 * sqrt(0.5 / V) for typical furnished rooms
#[wasm_bindgen]
pub fn get_schroeder_frequency(volume: f64, rt60: f64) -> f64 {
    2000.0 * (rt60 / volume).sqrt()
}

/// Calculate RT60 and other room acoustics metrics
///
/// Returns JSON with:
/// - rt60_sabine: Sabine reverberation time (seconds)
/// - rt60_eyring: Eyring reverberation time (seconds) - more accurate for absorptive rooms
/// - volume: Room volume (m³)
/// - surface_area: Total surface area (m²)
/// - average_alpha: Average absorption coefficient
/// - total_absorption: Total absorption in sabins (m²)
/// - schroeder_frequency: Transition frequency from modal to statistical behavior (Hz)
/// - critical_distance: Distance where direct and reverberant fields are equal (m)
///
/// # Arguments
/// * `width` - Room width (X dimension) in meters
/// * `depth` - Room depth (Y dimension) in meters
/// * `height` - Room height (Z dimension) in meters
/// * `average_absorption` - Average absorption coefficient (0.0 to 1.0)
/// * `frequency` - Reference frequency for absorption (typically 1000 Hz)
#[wasm_bindgen]
pub fn get_rt60(width: f64, depth: f64, height: f64, average_absorption: f64) -> String {
    let volume = width * depth * height;
    let surface_area = 2.0 * (width * depth + width * height + depth * height);
    let total_absorption = average_absorption * surface_area;

    let rt60_sab = rt60_sabine(volume, total_absorption);
    let rt60_eyr = rt60_eyring(volume, surface_area, average_absorption);
    let schroeder_freq = 2000.0 * (rt60_eyr / volume).sqrt();
    let crit_dist = critical_distance(volume, rt60_eyr);

    let result = RoomAcoustics {
        rt60_sabine: rt60_sab,
        rt60_eyring: rt60_eyr,
        volume,
        surface_area,
        average_alpha: average_absorption,
        total_absorption,
        schroeder_frequency: schroeder_freq,
        critical_distance: crit_dist,
    };

    serde_json::to_string(&result).unwrap_or_else(|_| "{}".to_string())
}

/// Compute impulse response from frequency response data
///
/// Takes JSON with:
/// - frequencies: array of frequency values (Hz)
/// - magnitudes: array of magnitude values (linear, not dB)
/// - phases: array of phase values (radians)
/// - config: optional ImpulseResponseConfig object
///
/// Returns JSON with impulse response data
#[wasm_bindgen]
pub fn compute_impulse_response(input_json: &str) -> String {
    #[derive(Deserialize)]
    struct IrInput {
        frequencies: Vec<f64>,
        magnitudes: Vec<f64>,
        phases: Vec<f64>,
        #[serde(default)]
        config: Option<ImpulseResponseConfig>,
    }

    let input: IrInput = match serde_json::from_str(input_json) {
        Ok(v) => v,
        Err(e) => return format!(r#"{{"error": "Invalid JSON: {}"}}"#, e),
    };

    if input.frequencies.len() != input.magnitudes.len()
        || input.frequencies.len() != input.phases.len()
    {
        return r#"{"error": "frequencies, magnitudes, and phases arrays must have same length"}"#
            .to_string();
    }

    // Build complex frequency response from magnitude and phase
    let complex_response: Vec<Complex64> = input
        .magnitudes
        .iter()
        .zip(input.phases.iter())
        .map(|(&mag, &phase)| Complex64::from_polar(mag, phase))
        .collect();

    let config = input.config.unwrap_or_default();
    let ir = calculate_impulse_response(&input.frequencies, &complex_response, &config);

    serde_json::to_string(&ir)
        .unwrap_or_else(|_| r#"{"error": "Serialization failed"}"#.to_string())
}

/// Validate a configuration JSON and return any errors
#[wasm_bindgen]
pub fn validate_config(config_json: &str) -> String {
    match serde_json::from_str::<SimulationConfig>(config_json) {
        Ok(config) => {
            let mut warnings = Vec::new();

            let (w, d, h) = match &config.room {
                RoomGeometryConfig::Rectangular {
                    width,
                    depth,
                    height,
                } => (*width, *depth, *height),
                RoomGeometryConfig::LShaped {
                    width1,
                    depth1,
                    depth2,
                    height,
                    ..
                } => (*width1, depth1 + depth2, *height),
            };

            if w <= 0.0 || d <= 0.0 || h <= 0.0 {
                warnings.push("Room dimensions must be positive".to_string());
            }

            if config.sources.is_empty() {
                warnings.push("At least one source is required".to_string());
            }

            for (idx, source) in config.sources.iter().enumerate() {
                if source.position.x < 0.0
                    || source.position.x > w
                    || source.position.y < 0.0
                    || source.position.y > d
                    || source.position.z < 0.0
                    || source.position.z > h
                {
                    warnings.push(format!("Source {} is outside room bounds", idx + 1));
                }
            }

            if config.listening_positions.is_empty() {
                warnings.push("At least one listening position is required".to_string());
            }

            for (idx, lp) in config.listening_positions.iter().enumerate() {
                if lp.x < 0.0 || lp.x > w || lp.y < 0.0 || lp.y > d || lp.z < 0.0 || lp.z > h {
                    warnings.push(format!(
                        "Listening position {} is outside room bounds",
                        idx + 1
                    ));
                }
            }

            if config.frequencies.min_freq <= 0.0 {
                warnings.push("Minimum frequency must be positive".to_string());
            }
            if config.frequencies.max_freq <= config.frequencies.min_freq {
                warnings.push("Maximum frequency must be greater than minimum".to_string());
            }
            if config.frequencies.num_points < 2 {
                warnings.push("At least 2 frequency points are required".to_string());
            }

            if warnings.is_empty() {
                serde_json::json!({"valid": true, "warnings": []}).to_string()
            } else {
                serde_json::json!({"valid": false, "warnings": warnings}).to_string()
            }
        }
        Err(e) => serde_json::json!({"valid": false, "error": e.to_string()}).to_string(),
    }
}
