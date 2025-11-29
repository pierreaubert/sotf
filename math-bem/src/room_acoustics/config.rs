//! JSON configuration for room acoustics simulations

use super::*;
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
    Rectangular { width: f64, depth: f64, height: f64 },
    #[serde(rename = "lshaped")]
    LShaped {
        /// Main section width
        width1: f64,
        /// Main section depth
        depth1: f64,
        /// Extension width
        width2: f64,
        /// Extension depth
        depth2: f64,
        /// Common height
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
        /// Horizontal angles in degrees [0, 360)
        horizontal_angles: Vec<f64>,
        /// Vertical angles in degrees [0, 180]
        vertical_angles: Vec<f64>,
        /// Magnitude values (row-major: [n_vertical][n_horizontal])
        magnitude: Vec<Vec<f64>>,
    },
}

/// Crossover filter configuration
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CrossoverConfig {
    /// Full range speaker (no crossover)
    #[serde(rename = "fullrange")]
    #[default]
    FullRange,
    /// Lowpass filter for subwoofers
    #[serde(rename = "lowpass")]
    Lowpass {
        /// Cutoff frequency in Hz
        cutoff_freq: f64,
        /// Filter order
        order: u32,
    },
    /// Highpass filter for tweeters/satellites
    #[serde(rename = "highpass")]
    Highpass {
        /// Cutoff frequency in Hz
        cutoff_freq: f64,
        /// Filter order
        order: u32,
    },
    /// Bandpass filter for midrange drivers
    #[serde(rename = "bandpass")]
    Bandpass {
        /// Low cutoff frequency in Hz
        low_cutoff: f64,
        /// High cutoff frequency in Hz
        high_cutoff: f64,
        /// Filter order
        order: u32,
    },
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
    /// Spacing type
    #[serde(default = "default_spacing")]
    pub spacing: String, // "logarithmic" or "linear"
}

fn default_spacing() -> String {
    "logarithmic".to_string()
}

/// Solver configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SolverConfig {
    /// Solver method
    #[serde(default = "default_method")]
    pub method: String, // "direct", "gmres", "gmres+ilu", "fmm+gmres", "fmm+gmres+ilu"

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

    /// Adaptive mesh refinement (frequency-dependent, source-proximity based)
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
    /// Maximum iterations
    #[serde(default = "default_max_iter")]
    pub max_iter: usize,
    /// Restart interval
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
    /// ILU method (TBEM, SLFMM, MLFMM)
    #[serde(default = "default_ilu_method")]
    pub method: String,
    /// Scanning degree (coarse, medium, fine, finest)
    #[serde(default = "default_scanning_degree")]
    pub scanning_degree: String,
    /// Use hierarchical FMM preconditioner instead of ILU
    /// This is faster for very large problems but may converge slower
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
    /// FMM type (SLFMM or MLFMM)
    #[serde(default = "default_fmm_type")]
    pub fmm_type: String,
    /// Expansion order (p)
    #[serde(default = "default_expansion_order")]
    pub expansion_order: usize,
    /// Max particles per leaf
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
    /// Generate horizontal and vertical spatial slices
    #[serde(default = "default_generate_slices")]
    pub generate_slices: bool,
    /// Resolution for spatial grids (points per dimension)
    #[serde(default = "default_slice_resolution")]
    pub slice_resolution: usize,
    /// Frequencies to compute slices at (indices into frequency array, or empty for all)
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
    /// Author
    #[serde(default)]
    pub author: String,
    /// Creation date
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
        // Convert room geometry
        let room = self.room.to_geometry()?;

        // Convert sources
        let sources: Vec<Source> = self
            .sources
            .iter()
            .map(|s| s.to_source())
            .collect::<Result<Vec<_>, _>>()?;

        // Convert listening positions
        let lps: Vec<ListeningPosition> =
            self.listening_positions.iter().map(|&p| p.into()).collect();

        // Create simulation
        let simulation = RoomSimulation::with_frequencies(
            room,
            sources,
            lps,
            self.frequencies.min_freq,
            self.frequencies.max_freq,
            self.frequencies.num_points,
        );

        Ok(simulation)
    }
}

impl RoomGeometryConfig {
    fn to_geometry(&self) -> Result<RoomGeometry, String> {
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

impl SourceConfig {
    fn to_source(&self) -> Result<Source, String> {
        let directivity = self.directivity.to_pattern()?;
        let crossover = self.crossover.to_filter();

        let source = Source::new(self.position.into(), directivity, self.amplitude)
            .with_name(self.name.clone())
            .with_crossover(crossover);

        Ok(source)
    }
}

impl DirectivityConfig {
    fn to_pattern(&self) -> Result<DirectivityPattern, String> {
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

                // Convert Vec<Vec<f64>> to Array2
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
