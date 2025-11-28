//! WASM bindings for Room Acoustics Simulator
//!
//! This module provides WebAssembly bindings to run room acoustics
//! simulations in the browser. It supports:
//! - Rectangular and L-shaped room geometries
//! - Multiple sources with directivity and crossover filters
//! - Direct field computation (free-field propagation)
//! - Frequency response and spatial slice visualization
//!
//! Note: This is a self-contained WASM implementation that doesn't
//! depend on the full math-bem crate (which requires BLAS/LAPACK).

use ndarray::Array2;
use num_complex::Complex64;
use serde::{Deserialize, Serialize};
use std::f64::consts::PI;
use wasm_bindgen::prelude::*;

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
        Self { width, depth, height }
    }

    pub fn get_edges(&self) -> Vec<(Point3D, Point3D)> {
        vec![
            // Floor edges
            (Point3D::new(0.0, 0.0, 0.0), Point3D::new(self.width, 0.0, 0.0)),
            (Point3D::new(self.width, 0.0, 0.0), Point3D::new(self.width, self.depth, 0.0)),
            (Point3D::new(self.width, self.depth, 0.0), Point3D::new(0.0, self.depth, 0.0)),
            (Point3D::new(0.0, self.depth, 0.0), Point3D::new(0.0, 0.0, 0.0)),
            // Ceiling edges
            (Point3D::new(0.0, 0.0, self.height), Point3D::new(self.width, 0.0, self.height)),
            (Point3D::new(self.width, 0.0, self.height), Point3D::new(self.width, self.depth, self.height)),
            (Point3D::new(self.width, self.depth, self.height), Point3D::new(0.0, self.depth, self.height)),
            (Point3D::new(0.0, self.depth, self.height), Point3D::new(0.0, 0.0, self.height)),
            // Vertical edges
            (Point3D::new(0.0, 0.0, 0.0), Point3D::new(0.0, 0.0, self.height)),
            (Point3D::new(self.width, 0.0, 0.0), Point3D::new(self.width, 0.0, self.height)),
            (Point3D::new(self.width, self.depth, 0.0), Point3D::new(self.width, self.depth, self.height)),
            (Point3D::new(0.0, self.depth, 0.0), Point3D::new(0.0, self.depth, self.height)),
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
        Self { width1, depth1, width2, depth2, height }
    }

    pub fn get_edges(&self) -> Vec<(Point3D, Point3D)> {
        let total_depth = self.depth1 + self.depth2;
        vec![
            // Floor edges - Main section
            (Point3D::new(0.0, 0.0, 0.0), Point3D::new(self.width1, 0.0, 0.0)),
            (Point3D::new(self.width1, 0.0, 0.0), Point3D::new(self.width1, self.depth1, 0.0)),
            (Point3D::new(self.width1, self.depth1, 0.0), Point3D::new(self.width2, self.depth1, 0.0)),
            (Point3D::new(self.width2, self.depth1, 0.0), Point3D::new(self.width2, total_depth, 0.0)),
            (Point3D::new(self.width2, total_depth, 0.0), Point3D::new(0.0, total_depth, 0.0)),
            (Point3D::new(0.0, total_depth, 0.0), Point3D::new(0.0, 0.0, 0.0)),
            // Ceiling edges
            (Point3D::new(0.0, 0.0, self.height), Point3D::new(self.width1, 0.0, self.height)),
            (Point3D::new(self.width1, 0.0, self.height), Point3D::new(self.width1, self.depth1, self.height)),
            (Point3D::new(self.width1, self.depth1, self.height), Point3D::new(self.width2, self.depth1, self.height)),
            (Point3D::new(self.width2, self.depth1, self.height), Point3D::new(self.width2, total_depth, self.height)),
            (Point3D::new(self.width2, total_depth, self.height), Point3D::new(0.0, total_depth, self.height)),
            (Point3D::new(0.0, total_depth, self.height), Point3D::new(0.0, 0.0, self.height)),
            // Vertical edges
            (Point3D::new(0.0, 0.0, 0.0), Point3D::new(0.0, 0.0, self.height)),
            (Point3D::new(self.width1, 0.0, 0.0), Point3D::new(self.width1, 0.0, self.height)),
            (Point3D::new(self.width1, self.depth1, 0.0), Point3D::new(self.width1, self.depth1, self.height)),
            (Point3D::new(self.width2, self.depth1, 0.0), Point3D::new(self.width2, self.depth1, self.height)),
            (Point3D::new(self.width2, total_depth, 0.0), Point3D::new(self.width2, total_depth, self.height)),
            (Point3D::new(0.0, total_depth, 0.0), Point3D::new(0.0, total_depth, self.height)),
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
    Lowpass { cutoff_freq: f64, order: u32 },
    Highpass { cutoff_freq: f64, order: u32 },
    Bandpass { low_cutoff: f64, high_cutoff: f64, order: u32 },
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
            CrossoverFilter::Bandpass { low_cutoff, high_cutoff, order } => {
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

        Self { horizontal_angles, vertical_angles, magnitude }
    }

    pub fn interpolate(&self, theta: f64, phi: f64) -> f64 {
        let theta_deg = theta.to_degrees();
        let mut phi_deg = phi.to_degrees();

        while phi_deg < 0.0 { phi_deg += 360.0; }
        while phi_deg >= 360.0 { phi_deg -= 360.0; }

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
}

impl Source {
    pub fn new(position: Point3D, directivity: DirectivityPattern, amplitude: f64) -> Self {
        Self {
            position,
            directivity,
            amplitude,
            crossover: CrossoverFilter::FullRange,
            name: String::from("Source"),
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
    LShaped { width1: f64, depth1: f64, width2: f64, depth2: f64, height: f64 },
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
    Bandpass { low_cutoff: f64, high_cutoff: f64, order: u32 },
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
            CrossoverConfig::Bandpass { low_cutoff, high_cutoff, order } => CrossoverFilter::Bandpass {
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
}

fn default_amplitude() -> f64 { 1.0 }

/// Frequency range configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrequencyConfig {
    pub min_freq: f64,
    pub max_freq: f64,
    pub num_points: usize,
    #[serde(default = "default_spacing")]
    pub spacing: String,
}

fn default_spacing() -> String { "logarithmic".to_string() }

/// Solver configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverConfig {
    #[serde(default = "default_method")]
    pub method: String,
    #[serde(default = "default_mesh_resolution")]
    pub mesh_resolution: usize,
    #[serde(default = "default_speed_of_sound")]
    pub speed_of_sound: f64,
}

fn default_method() -> String { "direct".to_string() }
fn default_mesh_resolution() -> usize { 2 }
fn default_speed_of_sound() -> f64 { 343.0 }

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            method: default_method(),
            mesh_resolution: default_mesh_resolution(),
            speed_of_sound: default_speed_of_sound(),
        }
    }
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
}

fn default_generate_slices() -> bool { true }
fn default_slice_resolution() -> usize { 50 }

impl Default for VisualizationConfig {
    fn default() -> Self {
        Self {
            generate_slices: true,
            slice_resolution: 50,
            slice_frequency_indices: Vec::new(),
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
    pub metadata: MetadataConfig,
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
}

#[derive(Debug, Clone, Serialize)]
pub struct SourceOutputInfo {
    pub name: String,
    pub position: [f64; 3],
    #[serde(skip_serializing_if = "Option::is_none")]
    pub crossover: Option<String>,
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
    if num <= 1 { return vec![start]; }
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
    if num <= 1 { return vec![start]; }
    (0..num)
        .map(|i| start + (end - start) * i as f64 / (num - 1) as f64)
        .collect()
}

/// Create a simple cardioid directivity pattern
fn create_cardioid_pattern(front_back_ratio: f64) -> DirectivityPattern {
    let horizontal_angles: Vec<f64> = (0..36).map(|i| i as f64 * 10.0).collect();
    let vertical_angles: Vec<f64> = (0..19).map(|i| i as f64 * 10.0).collect();

    let mut magnitude = Array2::zeros((vertical_angles.len(), horizontal_angles.len()));

    let a = (front_back_ratio + 1.0) / (front_back_ratio + 1.0);
    let b = (front_back_ratio - 1.0) / (front_back_ratio + 1.0);

    for (v_idx, &theta_deg) in vertical_angles.iter().enumerate() {
        for (h_idx, &phi_deg) in horizontal_angles.iter().enumerate() {
            let theta = theta_deg.to_radians();
            let phi = phi_deg.to_radians();
            let cos_angle = theta.sin() * phi.cos();
            let response = (a + b * cos_angle).max(0.0);
            magnitude[[v_idx, h_idx]] = response;
        }
    }

    DirectivityPattern { horizontal_angles, vertical_angles, magnitude }
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
            RoomGeometryConfig::Rectangular { width, depth, height } => {
                console_log!("Creating rectangular room: {}x{}x{} m", width, depth, height);
                RoomGeometry::Rectangular(RectangularRoom::new(*width, *depth, *height))
            }
            RoomGeometryConfig::LShaped { width1, depth1, width2, depth2, height } => {
                console_log!("Creating L-shaped room: {}x{} + {}x{} x {} m", width1, depth1, width2, depth2, height);
                RoomGeometry::LShaped(LShapedRoom::new(*width1, *depth1, *width2, *depth2, *height))
            }
        };

        // Build sources
        let sources: Vec<Source> = config.sources.iter().map(|s| {
            let directivity = match &s.directivity {
                DirectivityConfig::Omnidirectional => DirectivityPattern::omnidirectional(),
                DirectivityConfig::Cardioid { front_back_ratio } => create_cardioid_pattern(*front_back_ratio),
            };

            Source::new(s.position.into(), directivity, s.amplitude)
                .with_name(s.name.clone())
                .with_crossover(s.crossover.to_filter())
        }).collect();

        console_log!("Created {} sources", sources.len());

        let listening_position = config.listening_positions.first()
            .map(|p| (*p).into())
            .unwrap_or(Point3D::new(0.0, 0.0, 0.0));

        let frequencies = if config.frequencies.spacing == "linear" {
            lin_space(config.frequencies.min_freq, config.frequencies.max_freq, config.frequencies.num_points)
        } else {
            log_space(config.frequencies.min_freq, config.frequencies.max_freq, config.frequencies.num_points)
        };

        console_log!("Frequency range: {:.1} - {:.1} Hz ({} points)",
            frequencies.first().unwrap_or(&0.0),
            frequencies.last().unwrap_or(&0.0),
            frequencies.len()
        );

        let speed_of_sound = config.solver.speed_of_sound;

        Ok(RoomSimulatorWasm {
            config,
            room_geometry,
            sources,
            listening_position,
            frequencies,
            speed_of_sound,
        })
    }

    fn wavenumber(&self, frequency: f64) -> f64 {
        2.0 * PI * frequency / self.speed_of_sound
    }

    fn calculate_direct_field(&self, point: &Point3D, frequency: f64) -> Complex64 {
        let k = self.wavenumber(frequency);
        let mut total_pressure = Complex64::new(0.0, 0.0);

        // Determine reflection order from solver method
        let reflection_order = match self.config.solver.method.as_str() {
            "direct" => 0,
            "image-source-1" => 1,
            "image-source-2" => 2,
            "image-source-3" => 3,
            _ => 2, // Default to 2nd order
        };

        // Get room dimensions for image source calculation
        let (room_width, room_depth, room_height) = self.get_room_dimensions();

        // Wall reflection coefficient (typical for plaster walls)
        let wall_reflection = 0.9;

        for source in &self.sources {
            let amplitude = source.amplitude_towards(point, frequency);

            // Direct sound (always included)
            let r_direct = source.position.distance_to(point);
            total_pressure += greens_function_3d(r_direct, k) * amplitude;

            if reflection_order >= 1 {
                // First-order image sources (6 walls)
                let image_sources = [
                    // Left wall (x=0)
                    Point3D::new(-source.position.x, source.position.y, source.position.z),
                    // Right wall (x=width)
                    Point3D::new(2.0 * room_width - source.position.x, source.position.y, source.position.z),
                    // Front wall (y=0)
                    Point3D::new(source.position.x, -source.position.y, source.position.z),
                    // Back wall (y=depth)
                    Point3D::new(source.position.x, 2.0 * room_depth - source.position.y, source.position.z),
                    // Floor (z=0)
                    Point3D::new(source.position.x, source.position.y, -source.position.z),
                    // Ceiling (z=height)
                    Point3D::new(source.position.x, source.position.y, 2.0 * room_height - source.position.z),
                ];

                for image_src in &image_sources {
                    let r_image = image_src.distance_to(point);
                    if r_image > 1e-6 {
                        total_pressure += greens_function_3d(r_image, k) * amplitude * wall_reflection;
                    }
                }
            }

            if reflection_order >= 2 {
                // Second-order image sources (edges - 12 combinations)
                let edge_images = [
                    // x=0, y=0
                    Point3D::new(-source.position.x, -source.position.y, source.position.z),
                    // x=0, y=depth
                    Point3D::new(-source.position.x, 2.0 * room_depth - source.position.y, source.position.z),
                    // x=width, y=0
                    Point3D::new(2.0 * room_width - source.position.x, -source.position.y, source.position.z),
                    // x=width, y=depth
                    Point3D::new(2.0 * room_width - source.position.x, 2.0 * room_depth - source.position.y, source.position.z),
                    // x=0, z=0
                    Point3D::new(-source.position.x, source.position.y, -source.position.z),
                    // x=0, z=height
                    Point3D::new(-source.position.x, source.position.y, 2.0 * room_height - source.position.z),
                    // x=width, z=0
                    Point3D::new(2.0 * room_width - source.position.x, source.position.y, -source.position.z),
                    // x=width, z=height
                    Point3D::new(2.0 * room_width - source.position.x, source.position.y, 2.0 * room_height - source.position.z),
                    // y=0, z=0
                    Point3D::new(source.position.x, -source.position.y, -source.position.z),
                    // y=0, z=height
                    Point3D::new(source.position.x, -source.position.y, 2.0 * room_height - source.position.z),
                    // y=depth, z=0
                    Point3D::new(source.position.x, 2.0 * room_depth - source.position.y, -source.position.z),
                    // y=depth, z=height
                    Point3D::new(source.position.x, 2.0 * room_depth - source.position.y, 2.0 * room_height - source.position.z),
                ];

                let reflection_coeff_2 = wall_reflection * wall_reflection;
                for image_src in &edge_images {
                    let r_image = image_src.distance_to(point);
                    if r_image > 1e-6 {
                        total_pressure += greens_function_3d(r_image, k) * amplitude * reflection_coeff_2;
                    }
                }
            }

            if reflection_order >= 3 {
                // Third-order image sources (corners - 8 combinations)
                let corner_images = [
                    Point3D::new(-source.position.x, -source.position.y, -source.position.z),
                    Point3D::new(-source.position.x, -source.position.y, 2.0 * room_height - source.position.z),
                    Point3D::new(-source.position.x, 2.0 * room_depth - source.position.y, -source.position.z),
                    Point3D::new(-source.position.x, 2.0 * room_depth - source.position.y, 2.0 * room_height - source.position.z),
                    Point3D::new(2.0 * room_width - source.position.x, -source.position.y, -source.position.z),
                    Point3D::new(2.0 * room_width - source.position.x, -source.position.y, 2.0 * room_height - source.position.z),
                    Point3D::new(2.0 * room_width - source.position.x, 2.0 * room_depth - source.position.y, -source.position.z),
                    Point3D::new(2.0 * room_width - source.position.x, 2.0 * room_depth - source.position.y, 2.0 * room_height - source.position.z),
                ];

                let reflection_coeff_3 = wall_reflection * wall_reflection * wall_reflection;
                for image_src in &corner_images {
                    let r_image = image_src.distance_to(point);
                    if r_image > 1e-6 {
                        total_pressure += greens_function_3d(r_image, k) * amplitude * reflection_coeff_3;
                    }
                }
            }
        }

        total_pressure
    }

    fn calculate_source_field(&self, source_idx: usize, point: &Point3D, frequency: f64) -> Complex64 {
        if source_idx >= self.sources.len() {
            return Complex64::new(0.0, 0.0);
        }

        let k = self.wavenumber(frequency);
        let source = &self.sources[source_idx];
        let amplitude = source.amplitude_towards(point, frequency);

        let (room_width, room_depth, room_height) = self.get_room_dimensions();
        let wall_reflection = 0.9;

        let mut total_pressure = Complex64::new(0.0, 0.0);

        // Direct sound
        let r_direct = source.position.distance_to(point);
        total_pressure += greens_function_3d(r_direct, k) * amplitude;

        // First-order reflections
        let image_sources = [
            Point3D::new(-source.position.x, source.position.y, source.position.z),
            Point3D::new(2.0 * room_width - source.position.x, source.position.y, source.position.z),
            Point3D::new(source.position.x, -source.position.y, source.position.z),
            Point3D::new(source.position.x, 2.0 * room_depth - source.position.y, source.position.z),
            Point3D::new(source.position.x, source.position.y, -source.position.z),
            Point3D::new(source.position.x, source.position.y, 2.0 * room_height - source.position.z),
        ];

        for image_src in &image_sources {
            let r_image = image_src.distance_to(point);
            if r_image > 1e-6 {
                total_pressure += greens_function_3d(r_image, k) * amplitude * wall_reflection;
            }
        }

        total_pressure
    }

    /// Run the full simulation and return JSON results
    #[wasm_bindgen]
    pub fn run_simulation(&self) -> Result<String, JsValue> {
        console_log!("Starting simulation...");

        let mut combined_spl = Vec::with_capacity(self.frequencies.len());
        let mut source_responses: Vec<Vec<f64>> = self.sources.iter()
            .map(|_| Vec::with_capacity(self.frequencies.len()))
            .collect();

        for (idx, &freq) in self.frequencies.iter().enumerate() {
            if idx % 20 == 0 {
                console_log!("Computing frequency {}/{} ({:.1} Hz)", idx + 1, self.frequencies.len(), freq);
            }

            let pressure = self.calculate_direct_field(&self.listening_position, freq);
            combined_spl.push(pressure_to_spl(pressure));

            for (src_idx, src_spl) in source_responses.iter_mut().enumerate() {
                let p = self.calculate_source_field(src_idx, &self.listening_position, freq);
                src_spl.push(pressure_to_spl(p));
            }
        }

        console_log!("Frequency response computed");

        let source_responses_output: Vec<SourceResponse> = self.sources.iter()
            .zip(source_responses.into_iter())
            .map(|(source, spl)| SourceResponse { source_name: source.name.clone(), spl })
            .collect();

        let (horizontal_slices, vertical_slices) = if self.config.visualization.generate_slices {
            console_log!("Computing spatial slices...");
            self.compute_slices()
        } else {
            (None, None)
        };

        let room_output = self.build_room_output();

        let sources_output: Vec<SourceOutputInfo> = self.sources.iter()
            .zip(self.config.sources.iter())
            .map(|(source, config)| {
                let crossover_str = match &config.crossover {
                    CrossoverConfig::FullRange => None,
                    CrossoverConfig::Lowpass { cutoff_freq, order } => Some(format!("LP {}Hz {}nd", cutoff_freq, order)),
                    CrossoverConfig::Highpass { cutoff_freq, order } => Some(format!("HP {}Hz {}nd", cutoff_freq, order)),
                    CrossoverConfig::Bandpass { low_cutoff, high_cutoff, order } => Some(format!("BP {}-{}Hz {}nd", low_cutoff, high_cutoff, order)),
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
            listening_position: [self.listening_position.x, self.listening_position.y, self.listening_position.z],
            frequencies: self.frequencies.clone(),
            frequency_response: combined_spl,
            source_responses: Some(source_responses_output),
            horizontal_slices,
            vertical_slices,
            solver: self.config.solver.method.clone(),
            mesh_nodes: None,
            mesh_elements: None,
            metadata: Some(self.config.metadata.clone()),
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
        let edges_arrays: Vec<[[f64; 3]; 2]> = edges.iter()
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

        let freq_indices: Vec<usize> = if self.config.visualization.slice_frequency_indices.is_empty() {
            let step = (self.frequencies.len() / 10).max(1);
            (0..self.frequencies.len()).step_by(step).collect()
        } else {
            self.config.visualization.slice_frequency_indices.iter()
                .filter(|&&i| i < self.frequencies.len())
                .copied()
                .collect()
        };

        let mut horizontal_slices = Vec::with_capacity(freq_indices.len());
        let mut vertical_slices = Vec::with_capacity(freq_indices.len());

        for (idx, &freq_idx) in freq_indices.iter().enumerate() {
            let freq = self.frequencies[freq_idx];

            if idx % 3 == 0 {
                console_log!("Computing slice {}/{} ({:.1} Hz)", idx + 1, freq_indices.len(), freq);
            }

            // Horizontal slice
            let mut h_spl = Vec::with_capacity(resolution * resolution);
            for &y in &y_points {
                for &x in &x_points {
                    let point = Point3D::new(x, y, self.listening_position.z);
                    let pressure = self.calculate_direct_field(&point, freq);
                    h_spl.push(pressure_to_spl(pressure));
                }
            }

            horizontal_slices.push(SliceOutput {
                frequency: freq,
                x: x_points.clone(),
                y: y_points.clone(),
                z: None,
                spl: h_spl,
                shape: [resolution, resolution],
            });

            // Vertical slice
            let mut v_spl = Vec::with_capacity(resolution * resolution);
            for &z in &z_points {
                for &x in &x_points {
                    let point = Point3D::new(x, self.listening_position.y, z);
                    let pressure = self.calculate_direct_field(&point, freq);
                    v_spl.push(pressure_to_spl(pressure));
                }
            }

            vertical_slices.push(SliceOutput {
                frequency: freq,
                x: x_points.clone(),
                y: Vec::new(),
                z: Some(z_points.clone()),
                spl: v_spl,
                shape: [resolution, resolution],
            });
        }

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
        room: RoomGeometryConfig::Rectangular { width: 5.0, depth: 4.0, height: 2.5 },
        sources: vec![
            SourceConfig {
                name: "Left Speaker".to_string(),
                position: Point3DConfig { x: 1.5, y: 0.5, z: 1.2 },
                amplitude: 1.0,
                directivity: DirectivityConfig::Omnidirectional,
                crossover: CrossoverConfig::FullRange,
            },
            SourceConfig {
                name: "Right Speaker".to_string(),
                position: Point3DConfig { x: 3.5, y: 0.5, z: 1.2 },
                amplitude: 1.0,
                directivity: DirectivityConfig::Omnidirectional,
                crossover: CrossoverConfig::FullRange,
            },
        ],
        listening_positions: vec![Point3DConfig { x: 2.5, y: 2.5, z: 1.2 }],
        frequencies: FrequencyConfig {
            min_freq: 20.0,
            max_freq: 500.0,
            num_points: 100,
            spacing: "logarithmic".to_string(),
        },
        solver: SolverConfig::default(),
        visualization: VisualizationConfig::default(),
        metadata: MetadataConfig {
            description: "Default stereo setup".to_string(),
            author: "Room Simulator WASM".to_string(),
            date: String::new(),
            notes: String::new(),
        },
    };

    serde_json::to_string_pretty(&config).unwrap_or_else(|_| "{}".to_string())
}

/// Validate a configuration JSON and return any errors
#[wasm_bindgen]
pub fn validate_config(config_json: &str) -> String {
    match serde_json::from_str::<SimulationConfig>(config_json) {
        Ok(config) => {
            let mut warnings = Vec::new();

            let (w, d, h) = match &config.room {
                RoomGeometryConfig::Rectangular { width, depth, height } => (*width, *depth, *height),
                RoomGeometryConfig::LShaped { width1, depth1, depth2, height, .. } => (*width1, depth1 + depth2, *height),
            };

            if w <= 0.0 || d <= 0.0 || h <= 0.0 {
                warnings.push("Room dimensions must be positive".to_string());
            }

            if config.sources.is_empty() {
                warnings.push("At least one source is required".to_string());
            }

            for (idx, source) in config.sources.iter().enumerate() {
                if source.position.x < 0.0 || source.position.x > w ||
                   source.position.y < 0.0 || source.position.y > d ||
                   source.position.z < 0.0 || source.position.z > h {
                    warnings.push(format!("Source {} is outside room bounds", idx + 1));
                }
            }

            if config.listening_positions.is_empty() {
                warnings.push("At least one listening position is required".to_string());
            }

            for (idx, lp) in config.listening_positions.iter().enumerate() {
                if lp.x < 0.0 || lp.x > w || lp.y < 0.0 || lp.y > d || lp.z < 0.0 || lp.z > h {
                    warnings.push(format!("Listening position {} is outside room bounds", idx + 1));
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
        Err(e) => {
            serde_json::json!({"valid": false, "error": e.to_string()}).to_string()
        }
    }
}
