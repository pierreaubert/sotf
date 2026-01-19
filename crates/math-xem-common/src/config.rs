//! JSON configuration for room acoustics simulations

use crate::geometry::{LShapedRoom, RectangularRoom, RoomGeometry};
use crate::source::{CrossoverFilter, DirectivityPattern, Source};
use crate::types::{Point3D, log_space};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Complete room configuration loaded from JSON
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomConfig {
    /// Room geometry specification
    pub room: RoomGeometryConfig,
    /// Sound sources
    pub sources: Vec<SourceConfig>,
    /// Listening positions
    pub listening_positions: Vec<Point3DConfig>,
    /// Frequency configuration
    pub frequencies: FrequencyConfig,
    /// Boundary conditions
    #[serde(default)]
    pub boundaries: BoundaryConfig,
    /// Solver configuration
    #[serde(default)]
    pub solver: SolverConfig,
    /// Visualization configuration
    #[serde(default)]
    pub visualization: VisualizationConfig,
    /// Simulation metadata
    #[serde(default)]
    pub metadata: MetadataConfig,
}

/// Room geometry configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum RoomGeometryConfig {
    #[serde(rename = "rectangular")]
    /// Rectangular room geometry
    Rectangular {
        /// Width (x-dimension)
        width: f64,
        /// Depth (y-dimension)
        depth: f64,
        /// Height (z-dimension)
        height: f64,
    },
    #[serde(rename = "lshaped")]
    /// L-shaped room geometry
    LShaped {
        /// Width of the main section
        width1: f64,
        /// Depth of the main section
        depth1: f64,
        /// Width of the extension
        width2: f64,
        /// Depth of the extension
        depth2: f64,
        /// Height of the room
        height: f64,
    },
}

impl RoomGeometryConfig {
    /// Convert to RoomGeometry
    pub fn to_geometry(&self) -> Result<RoomGeometry, String> {
        match self {
            RoomGeometryConfig::Rectangular {
                width,
                depth,
                height,
            } => Ok(RoomGeometry::Rectangular(RectangularRoom::new(
                *width, *depth, *height,
            ))),
            RoomGeometryConfig::LShaped {
                width1,
                depth1,
                width2,
                depth2,
                height,
            } => Ok(RoomGeometry::LShaped(LShapedRoom::new(
                *width1, *depth1, *width2, *depth2, *height,
            ))),
        }
    }
}

/// Boundary conditions configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryConfig {
    /// Floor boundary condition
    #[serde(default = "default_rigid")]
    pub floor: SurfaceConfig,
    /// Ceiling boundary condition
    #[serde(default = "default_rigid")]
    pub ceiling: SurfaceConfig,
    /// Default condition for all vertical walls
    #[serde(default = "default_rigid")]
    pub walls: SurfaceConfig,
    /// Override for front wall (y=0)
    pub front_wall: Option<SurfaceConfig>,
    /// Override for back wall (y=depth)
    pub back_wall: Option<SurfaceConfig>,
    /// Override for left wall (x=0)
    pub left_wall: Option<SurfaceConfig>,
    /// Override for right wall (x=width)
    pub right_wall: Option<SurfaceConfig>,
}

impl Default for BoundaryConfig {
    fn default() -> Self {
        Self {
            floor: SurfaceConfig::Rigid,
            ceiling: SurfaceConfig::Rigid,
            walls: SurfaceConfig::Rigid,
            front_wall: None,
            back_wall: None,
            left_wall: None,
            right_wall: None,
        }
    }
}

fn default_rigid() -> SurfaceConfig {
    SurfaceConfig::Rigid
}

/// Surface boundary condition type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SurfaceConfig {
    /// Perfectly rigid (Neumann BC, velocity = 0)
    #[serde(rename = "rigid")]
    Rigid,
    /// Absorption coefficient (0.0 to 1.0)
    #[serde(rename = "absorption")]
    Absorption { coefficient: f64 },
    /// Specific acoustic impedance (complex)
    #[serde(rename = "impedance")]
    Impedance { real: f64, imag: f64 },
}

/// 3D point configuration
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Point3DConfig {
    /// X coordinate
    pub x: f64,
    /// Y coordinate
    pub y: f64,
    /// Z coordinate
    pub z: f64,
}

impl From<Point3DConfig> for Point3D {
    fn from(p: Point3DConfig) -> Self {
        Point3D::new(p.x, p.y, p.z)
    }
}

impl From<Point3D> for Point3DConfig {
    fn from(p: Point3D) -> Self {
        Point3DConfig {
            x: p.x,
            y: p.y,
            z: p.z,
        }
    }
}

/// Source configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceConfig {
    /// Source name
    pub name: String,
    /// Source position
    pub position: Point3DConfig,
    /// Source amplitude
    #[serde(default = "default_amplitude")]
    pub amplitude: f64,
    /// Directivity pattern
    #[serde(default)]
    pub directivity: DirectivityConfig,
    /// Crossover filter
    #[serde(default)]
    pub crossover: CrossoverConfig,
}

fn default_amplitude() -> f64 {
    1.0
}

impl SourceConfig {
    /// Convert to Source
    pub fn to_source(&self) -> Result<Source, String> {
        let directivity = self.directivity.to_pattern()?;
        let crossover = self.crossover.to_filter();

        let source = Source::new(self.position.into(), directivity, self.amplitude)
            .with_name(self.name.clone())
            .with_crossover(crossover);

        Ok(source)
    }
}

/// Directivity pattern configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DirectivityConfig {
    /// Omnidirectional (spherical) radiation pattern
    #[serde(rename = "omnidirectional")]
    #[default]
    Omnidirectional,
    /// Custom directivity from measured data
    #[serde(rename = "custom")]
    Custom {
        /// Horizontal angles (degrees)
        horizontal_angles: Vec<f64>,
        /// Vertical angles (degrees)
        vertical_angles: Vec<f64>,
        /// Magnitude data (matrix)
        magnitude: Vec<Vec<f64>>,
    },
}

impl DirectivityConfig {
    /// Convert to DirectivityPattern
    pub fn to_pattern(&self) -> Result<DirectivityPattern, String> {
        match self {
            DirectivityConfig::Omnidirectional => Ok(DirectivityPattern::omnidirectional()),
            DirectivityConfig::Custom {
                horizontal_angles,
                vertical_angles,
                magnitude,
            } => {
                use ndarray::Array2;

                if magnitude.is_empty() {
                    return Err("Empty magnitude array".to_string());
                }

                let n_vert = magnitude.len();
                let n_horiz = magnitude[0].len();

                if n_vert != vertical_angles.len() {
                    return Err(format!(
                        "Vertical angles mismatch: {} vs {}",
                        n_vert,
                        vertical_angles.len()
                    ));
                }
                if n_horiz != horizontal_angles.len() {
                    return Err(format!(
                        "Horizontal angles mismatch: {} vs {}",
                        n_horiz,
                        horizontal_angles.len()
                    ));
                }

                let flat: Vec<f64> = magnitude
                    .iter()
                    .flat_map(|row| row.iter().copied())
                    .collect();
                let mag_array = Array2::from_shape_vec((n_vert, n_horiz), flat)
                    .map_err(|e| format!("Failed to create magnitude array: {}", e))?;

                Ok(DirectivityPattern {
                    horizontal_angles: horizontal_angles.clone(),
                    vertical_angles: vertical_angles.clone(),
                    magnitude: mag_array,
                })
            }
        }
    }
}

/// Crossover filter configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CrossoverConfig {
    /// Full range (no filter)
    #[serde(rename = "fullrange")]
    #[default]
    FullRange,
    /// Low-pass filter
    #[serde(rename = "lowpass")]
    Lowpass {
        /// Cutoff frequency (Hz)
        cutoff_freq: f64,
        /// Filter order (e.g., 2, 4)
        order: u32,
    },
    /// High-pass filter
    #[serde(rename = "highpass")]
    Highpass {
        /// Cutoff frequency (Hz)
        cutoff_freq: f64,
        /// Filter order (e.g., 2, 4)
        order: u32,
    },
    /// Band-pass filter
    #[serde(rename = "bandpass")]
    Bandpass {
        /// Low cutoff frequency (Hz)
        low_cutoff: f64,
        /// High cutoff frequency (Hz)
        high_cutoff: f64,
        /// Filter order
        order: u32,
    },
}

impl CrossoverConfig {
    /// Convert to CrossoverFilter
    pub fn to_filter(&self) -> CrossoverFilter {
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

/// Frequency configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrequencyConfig {
    /// Minimum frequency (Hz)
    pub min_freq: f64,
    /// Maximum frequency (Hz)
    pub max_freq: f64,
    /// Number of frequency points
    pub num_points: usize,
    /// Spacing type ("logarithmic" or "linear")
    #[serde(default = "default_spacing")]
    pub spacing: String,
}

fn default_spacing() -> String {
    "logarithmic".to_string()
}

impl FrequencyConfig {
    /// Generate frequency array based on configuration
    pub fn generate_frequencies(&self) -> Vec<f64> {
        if self.spacing.to_lowercase() == "linear" {
            crate::types::lin_space(self.min_freq, self.max_freq, self.num_points)
        } else {
            log_space(self.min_freq, self.max_freq, self.num_points)
        }
    }
}

/// Solver configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverConfig {
    /// Solver method
    #[serde(default = "default_method")]
    pub method: String,
    /// Mesh resolution (elements per meter)
    #[serde(default = "default_mesh_resolution")]
    pub mesh_resolution: usize,
    /// GMRES parameters
    #[serde(default)]
    pub gmres: GmresConfig,
    /// ILU preconditioner parameters
    #[serde(default)]
    pub ilu: IluConfig,
    /// FMM parameters
    #[serde(default)]
    pub fmm: FmmConfig,
    /// Adaptive integration
    #[serde(default = "default_adaptive_integration")]
    pub adaptive_integration: bool,
    /// Adaptive mesh refinement
    #[serde(default)]
    pub adaptive_meshing: Option<bool>,
}

impl Default for SolverConfig {
    fn default() -> Self {
        Self {
            method: default_method(),
            mesh_resolution: default_mesh_resolution(),
            gmres: GmresConfig::default(),
            ilu: IluConfig::default(),
            fmm: FmmConfig::default(),
            adaptive_integration: default_adaptive_integration(),
            adaptive_meshing: None,
        }
    }
}

fn default_method() -> String {
    "direct".to_string()
}

fn default_mesh_resolution() -> usize {
    2
}

fn default_adaptive_integration() -> bool {
    false
}

/// GMRES solver configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GmresConfig {
    /// Maximum number of iterations
    #[serde(default = "default_max_iter")]
    pub max_iter: usize,
    /// Restart parameter
    #[serde(default = "default_restart")]
    pub restart: usize,
    /// Convergence tolerance
    #[serde(default = "default_tolerance")]
    pub tolerance: f64,
}

impl Default for GmresConfig {
    fn default() -> Self {
        Self {
            max_iter: default_max_iter(),
            restart: default_restart(),
            tolerance: default_tolerance(),
        }
    }
}

fn default_max_iter() -> usize {
    100
}

fn default_restart() -> usize {
    50
}

fn default_tolerance() -> f64 {
    1e-6
}

/// ILU preconditioner configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IluConfig {
    /// ILU method (e.g., "tbem", "slfmm")
    #[serde(default = "default_ilu_method")]
    pub method: String,
    /// Scanning degree (e.g., "fine", "coarse")
    #[serde(default = "default_scanning_degree")]
    pub scanning_degree: String,
    /// Whether to use hierarchical preconditioning
    #[serde(default)]
    pub use_hierarchical: bool,
}

impl Default for IluConfig {
    fn default() -> Self {
        Self {
            method: default_ilu_method(),
            scanning_degree: default_scanning_degree(),
            use_hierarchical: false,
        }
    }
}

fn default_ilu_method() -> String {
    "tbem".to_string()
}

fn default_scanning_degree() -> String {
    "fine".to_string()
}

/// FMM configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FmmConfig {
    /// FMM type (e.g., "slfmm", "mlfmm")
    #[serde(default = "default_fmm_type")]
    pub fmm_type: String,
    /// Expansion order
    #[serde(default = "default_expansion_order")]
    pub expansion_order: usize,
    /// Maximum particles per leaf in the octree
    #[serde(default = "default_max_particles")]
    pub max_particles_per_leaf: usize,
}

impl Default for FmmConfig {
    fn default() -> Self {
        Self {
            fmm_type: default_fmm_type(),
            expansion_order: default_expansion_order(),
            max_particles_per_leaf: default_max_particles(),
        }
    }
}

fn default_fmm_type() -> String {
    "slfmm".to_string()
}

fn default_expansion_order() -> usize {
    6
}

fn default_max_particles() -> usize {
    50
}

/// Visualization configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VisualizationConfig {
    /// Whether to generate field slices
    #[serde(default = "default_generate_slices")]
    pub generate_slices: bool,
    /// Resolution of the slices (points per side)
    #[serde(default = "default_slice_resolution")]
    pub slice_resolution: usize,
    /// Indices of frequencies to visualize (empty for all)
    #[serde(default)]
    pub slice_frequency_indices: Vec<usize>,
}

impl Default for VisualizationConfig {
    fn default() -> Self {
        Self {
            generate_slices: default_generate_slices(),
            slice_resolution: default_slice_resolution(),
            slice_frequency_indices: Vec::new(),
        }
    }
}

fn default_generate_slices() -> bool {
    false
}

fn default_slice_resolution() -> usize {
    50
}

/// Simulation metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetadataConfig {
    /// Simulation description
    #[serde(default)]
    pub description: String,
    /// Author name
    #[serde(default)]
    pub author: String,
    /// Simulation date
    #[serde(default)]
    pub date: String,
}

impl Default for MetadataConfig {
    fn default() -> Self {
        Self {
            description: String::new(),
            author: String::new(),
            date: chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string(),
        }
    }
}

impl RoomConfig {
    /// Load configuration from JSON file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let contents =
            fs::read_to_string(path).map_err(|e| format!("Failed to read config file: {}", e))?;

        let config: RoomConfig =
            serde_json::from_str(&contents).map_err(|e| format!("Failed to parse JSON: {}", e))?;

        Ok(config)
    }

    /// Save configuration to JSON file
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        let json = serde_json::to_string_pretty(self)
            .map_err(|e| format!("Failed to serialize config: {}", e))?;

        fs::write(path, json).map_err(|e| format!("Failed to write config file: {}", e))?;

        Ok(())
    }

    /// Convert to RoomSimulation
    pub fn to_simulation(&self) -> Result<RoomSimulation, String> {
        let room = self.room.to_geometry()?;

        let sources: Vec<Source> = self
            .sources
            .iter()
            .map(|s| s.to_source())
            .collect::<Result<Vec<_>, _>>()?;

        let listening_positions: Vec<Point3D> =
            self.listening_positions.iter().map(|&p| p.into()).collect();

        let frequencies = self.frequencies.generate_frequencies();

        Ok(RoomSimulation {
            room,
            sources,
            listening_positions,
            frequencies,
            boundaries: self.boundaries.clone(), // Pass boundaries
            speed_of_sound: crate::types::constants::SPEED_OF_SOUND_20C,
        })
    }
}

/// Room acoustics simulation configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoomSimulation {
    /// Room geometry
    pub room: RoomGeometry,
    /// Sound sources
    pub sources: Vec<Source>,
    /// Listening positions
    pub listening_positions: Vec<Point3D>,
    /// Frequencies to simulate
    pub frequencies: Vec<f64>,
    /// Boundary conditions
    #[serde(default)]
    pub boundaries: BoundaryConfig,
    /// Speed of sound (m/s)
    pub speed_of_sound: f64,
}

impl RoomSimulation {
    /// Create a new simulation with default frequency range
    pub fn new(
        room: RoomGeometry,
        sources: Vec<Source>,
        listening_positions: Vec<Point3D>,
    ) -> Self {
        let frequencies = log_space(20.0, 20000.0, 200);

        Self {
            room,
            sources,
            listening_positions,
            frequencies,
            boundaries: BoundaryConfig::default(),
            speed_of_sound: crate::types::constants::SPEED_OF_SOUND_20C,
        }
    }

    /// Create simulation with custom frequency configuration
    pub fn with_frequencies(
        room: RoomGeometry,
        sources: Vec<Source>,
        listening_positions: Vec<Point3D>,
        min_freq: f64,
        max_freq: f64,
        num_points: usize,
    ) -> Self {
        let frequencies = log_space(min_freq, max_freq, num_points);

        Self {
            room,
            sources,
            listening_positions,
            frequencies,
            boundaries: BoundaryConfig::default(),
            speed_of_sound: crate::types::constants::SPEED_OF_SOUND_20C,
        }
    }

    /// Calculate wavenumber k = 2πf/c
    pub fn wavenumber(&self, frequency: f64) -> f64 {
        crate::types::wavenumber(frequency, self.speed_of_sound)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_frequency_config() {
        let config = FrequencyConfig {
            min_freq: 20.0,
            max_freq: 20000.0,
            num_points: 100,
            spacing: "logarithmic".to_string(),
        };
        let freqs = config.generate_frequencies();
        assert_eq!(freqs.len(), 100);
    }

    #[test]
    fn test_room_geometry_config() {
        let config = RoomGeometryConfig::Rectangular {
            width: 5.0,
            depth: 4.0,
            height: 2.5,
        };
        let geometry = config.to_geometry().unwrap();
        let (w, d, h) = geometry.dimensions();
        assert_eq!(w, 5.0);
        assert_eq!(d, 4.0);
        assert_eq!(h, 2.5);
    }
}
