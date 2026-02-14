//! Parser for C++ NC.inp format
//!
//! This module provides backwards compatibility with the NumCalc input format.
//!
//! ## Format Overview
//!
//! NC.inp files are text-based with sections marked by keywords:
//! - Comments start with `#` or `##`
//! - Sections: NODES, ELEMENTS, BOUNDARY, PLANE WAVES, POINT SOURCES, etc.
//! - END marker signals end of file

use std::fs;
use std::path::{Path, PathBuf};

use ndarray::{Array1, Array2};
use num_complex::Complex64;

use crate::core::types::{BoundaryCondition, Element, ElementProperty, ElementType, PhysicsParams};

/// Parsed NC.inp configuration
#[derive(Debug, Clone)]
pub struct NcInputConfig {
    /// Version string (e.g., "Mesh2HRTF 1.0.0")
    pub version: String,
    /// Description/title
    pub description: String,
    /// Control parameters I: [output_format, ...]
    pub control_params_i: Vec<i32>,
    /// Control parameters II: solver settings
    pub control_params_ii: Vec<f64>,
    /// Frequency curve: [(time, freq, phase), ...]
    pub frequency_curve: Vec<(f64, f64, f64)>,
    /// Main parameters I: mesh info
    pub main_params_i: MainParamsI,
    /// Main parameters II: solver options
    pub main_params_ii: MainParamsII,
    /// Main parameters III: symmetry options
    pub main_params_iii: Vec<i32>,
    /// Main parameters IV: physical constants
    pub main_params_iv: MainParamsIV,
    /// Node file paths
    pub node_files: Vec<PathBuf>,
    /// Element file paths
    pub element_files: Vec<PathBuf>,
    /// Symmetry configuration (if present)
    pub symmetry: Option<SymmetryConfig>,
    /// Boundary conditions
    pub boundary_conditions: Vec<BoundarySpec>,
    /// Plane wave sources
    pub plane_waves: Vec<PlaneWaveSource>,
    /// Point sources
    pub point_sources: Vec<PointSource>,
    /// Base directory for resolving relative paths
    pub base_dir: PathBuf,
}

/// Main parameters I (mesh configuration)
#[derive(Debug, Clone, Default)]
pub struct MainParamsI {
    /// Element type (2 = mixed, etc.)
    pub element_type: i32,
    /// Total number of nodes
    pub num_nodes: usize,
    /// Total number of elements
    pub num_elements: usize,
    /// Number of object mesh files
    pub num_object_files: i32,
    /// Number of evaluation grid files
    pub num_eval_files: i32,
    /// Boundary condition type
    pub bc_type: i32,
    /// Solver method (0=TBEM, 1=SLFMM, 2/3=MLFMM)
    pub solver_method: i32,
    /// FMM method variant
    pub fmm_method: i32,
    /// Parallel processing flag
    pub parallel: i32,
}

/// Main parameters II (solver options)
#[derive(Debug, Clone, Default)]
pub struct MainParamsII {
    /// Preconditioner type (0=ILU, 1=row scaling)
    pub preconditioner: i32,
    /// Iterative solver (0=CGS, 1=BiCGSTAB)
    pub iterative_solver: i32,
    /// Reserved
    pub reserved1: i32,
    /// Reserved
    pub reserved2: f64,
    /// Output level
    pub output_level: i32,
    /// Reserved
    pub reserved3: i32,
    /// Reserved
    pub reserved4: i32,
}

/// Main parameters IV (physical constants)
#[derive(Debug, Clone)]
pub struct MainParamsIV {
    /// Speed of sound (m/s)
    pub speed_of_sound: f64,
    /// Air density (kg/m³)
    pub density: f64,
    /// Reference pressure (Pa)
    pub reference_pressure: f64,
    /// Reserved values
    pub reserved: Vec<f64>,
}

impl Default for MainParamsIV {
    fn default() -> Self {
        Self {
            speed_of_sound: 343.0,
            density: 1.21,
            reference_pressure: 1.0,
            reserved: vec![0.0; 4],
        }
    }
}

/// Symmetry configuration
#[derive(Debug, Clone)]
pub struct SymmetryConfig {
    /// Symmetry flags [x, y, z]
    pub flags: [bool; 3],
    /// Symmetry plane origin
    pub origin: [f64; 3],
}

/// Boundary condition specification
#[derive(Debug, Clone)]
pub struct BoundarySpec {
    /// Element range start
    pub elem_start: usize,
    /// Element range end
    pub elem_end: usize,
    /// Boundary condition type ("VELO", "PRES", "ADMI")
    pub bc_type: String,
    /// Real part of BC value
    pub value_re: f64,
    /// Curve index for real part (-1 = constant)
    pub curve_re: i32,
    /// Imaginary part of BC value
    pub value_im: f64,
    /// Curve index for imaginary part (-1 = constant)
    pub curve_im: i32,
}

/// Plane wave source
#[derive(Debug, Clone)]
pub struct PlaneWaveSource {
    /// Direction vector [x, y, z]
    pub direction: [f64; 3],
    /// Amplitude (real part)
    pub amplitude_re: f64,
    /// Curve index for amplitude (-1 = constant)
    pub curve_re: i32,
    /// Amplitude (imaginary part)
    pub amplitude_im: f64,
    /// Curve index for imaginary (-1 = constant)
    pub curve_im: i32,
}

/// Point source
#[derive(Debug, Clone)]
pub struct PointSource {
    /// Position [x, y, z]
    pub position: [f64; 3],
    /// Amplitude (real part)
    pub amplitude_re: f64,
    /// Curve index for amplitude
    pub curve_re: i32,
    /// Amplitude (imaginary part)
    pub amplitude_im: f64,
    /// Curve index for imaginary
    pub curve_im: i32,
}

/// Parser error types
#[derive(Debug, thiserror::Error)]
pub enum NcParseError {
    /// IO error during file reading
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    /// Parse error at a specific line
    #[error("Parse error at line {line}: {message}")]
    Parse {
        /// Line number where error occurred
        line: usize,
        /// Error message
        message: String,
    },
    /// Required section is missing from the input file
    #[error("Missing required section: {0}")]
    MissingSection(String),
    /// Input file format is invalid
    #[error("Invalid format: {0}")]
    InvalidFormat(String),
}

/// Parse an NC.inp file
pub fn parse_nc_input<P: AsRef<Path>>(path: P) -> Result<NcInputConfig, NcParseError> {
    let path = path.as_ref();
    let base_dir = path.parent().unwrap_or(Path::new(".")).to_path_buf();
    let content = fs::read_to_string(path)?;

    parse_nc_input_string(&content, base_dir)
}

/// Parse NC.inp content from a string
pub fn parse_nc_input_string(
    content: &str,
    base_dir: PathBuf,
) -> Result<NcInputConfig, NcParseError> {
    let mut config = NcInputConfig {
        version: String::new(),
        description: String::new(),
        control_params_i: Vec::new(),
        control_params_ii: Vec::new(),
        frequency_curve: Vec::new(),
        main_params_i: MainParamsI::default(),
        main_params_ii: MainParamsII::default(),
        main_params_iii: Vec::new(),
        main_params_iv: MainParamsIV::default(),
        node_files: Vec::new(),
        element_files: Vec::new(),
        symmetry: None,
        boundary_conditions: Vec::new(),
        plane_waves: Vec::new(),
        point_sources: Vec::new(),
        base_dir,
    };

    let lines: Vec<&str> = content.lines().collect();
    let mut i = 0;

    while i < lines.len() {
        let line = lines[i].trim();

        // Skip empty lines and comments
        if line.is_empty() || line.starts_with("##") {
            i += 1;
            continue;
        }

        // Skip single-# comments (but process # at start of keywords)
        if line.starts_with('#') && !line.starts_with("##") {
            // Check if it's a commented-out section
            i += 1;
            continue;
        }

        // Parse version line
        if line.starts_with("Mesh2HRTF") {
            config.version = line.to_string();
            i += 1;
            continue;
        }

        // Parse description (line after version)
        if config.version.is_empty() {
            i += 1;
            continue;
        }

        // Control parameters I (exclude II)
        if i > 0
            && lines.get(i.saturating_sub(1)).is_some_and(|l| {
                l.contains("Controlparameter I") && !l.contains("Controlparameter II")
            })
        {
            config.control_params_i = parse_int_line(line);
            i += 1;
            continue;
        }

        // Control parameters II
        if i > 0
            && lines
                .get(i.saturating_sub(1))
                .is_some_and(|l| l.contains("Controlparameter II"))
        {
            config.control_params_ii = parse_float_line(line);
            i += 1;
            continue;
        }

        // Frequency curve
        if i > 0
            && lines
                .get(i.saturating_sub(1))
                .is_some_and(|l| l.contains("Frequency Curve"))
        {
            let header = parse_int_line(line);
            let num_points = header.get(1).copied().unwrap_or(0) as usize;
            i += 1;

            for _ in 0..num_points {
                if i < lines.len() {
                    let values = parse_float_line(lines[i]);
                    if values.len() >= 3 {
                        config
                            .frequency_curve
                            .push((values[0], values[1], values[2]));
                    }
                    i += 1;
                }
            }
            continue;
        }

        // Main Parameters I (exclude II, III, IV)
        if i > 0
            && lines.get(i.saturating_sub(1)).is_some_and(|l| {
                l.contains("Main Parameters I")
                    && !l.contains("Main Parameters II")
                    && !l.contains("Main Parameters III")
                    && !l.contains("Main Parameters IV")
            })
        {
            let values = parse_int_line(line);
            config.main_params_i = MainParamsI {
                element_type: values.first().copied().unwrap_or(0),
                num_nodes: values.get(1).copied().unwrap_or(0) as usize,
                num_elements: values.get(2).copied().unwrap_or(0) as usize,
                num_object_files: values.get(3).copied().unwrap_or(0),
                num_eval_files: values.get(4).copied().unwrap_or(0),
                bc_type: values.get(5).copied().unwrap_or(0),
                solver_method: values.get(6).copied().unwrap_or(0),
                fmm_method: values.get(7).copied().unwrap_or(0),
                parallel: values.get(8).copied().unwrap_or(0),
            };
            i += 1;
            continue;
        }

        // Main Parameters II (exclude III, IV)
        if i > 0
            && lines.get(i.saturating_sub(1)).is_some_and(|l| {
                l.contains("Main Parameters II")
                    && !l.contains("Main Parameters III")
                    && !l.contains("Main Parameters IV")
            })
        {
            let values = parse_mixed_line(line);
            config.main_params_ii = MainParamsII {
                preconditioner: values.first().map(|v| *v as i32).unwrap_or(0),
                iterative_solver: values.get(1).map(|v| *v as i32).unwrap_or(0),
                reserved1: values.get(2).map(|v| *v as i32).unwrap_or(0),
                reserved2: values.get(3).copied().unwrap_or(0.0),
                output_level: values.get(4).map(|v| *v as i32).unwrap_or(0),
                reserved3: values.get(5).map(|v| *v as i32).unwrap_or(0),
                reserved4: values.get(6).map(|v| *v as i32).unwrap_or(0),
            };
            i += 1;
            continue;
        }

        // Main Parameters III (exclude IV)
        if i > 0
            && lines.get(i.saturating_sub(1)).is_some_and(|l| {
                l.contains("Main Parameters III") && !l.contains("Main Parameters IV")
            })
        {
            config.main_params_iii = parse_int_line(line);
            i += 1;
            continue;
        }

        // Main Parameters IV
        if i > 0
            && lines
                .get(i.saturating_sub(1))
                .is_some_and(|l| l.contains("Main Parameters IV"))
        {
            let values = parse_float_line(line);
            config.main_params_iv = MainParamsIV {
                speed_of_sound: values.first().copied().unwrap_or(343.0),
                density: values.get(1).copied().unwrap_or(1.21),
                reference_pressure: values.get(2).copied().unwrap_or(1.0),
                reserved: values.get(3..).map(|s| s.to_vec()).unwrap_or_default(),
            };
            i += 1;
            continue;
        }

        // NODES section
        if line == "NODES" {
            i += 1;
            while i < lines.len() {
                let node_line = lines[i].trim();
                if node_line.starts_with("##") || node_line.is_empty() {
                    break;
                }
                if !node_line.starts_with('#') {
                    let path = config.base_dir.join(node_line);
                    config.node_files.push(path);
                }
                i += 1;
            }
            continue;
        }

        // ELEMENTS section
        if line == "ELEMENTS" {
            i += 1;
            while i < lines.len() {
                let elem_line = lines[i].trim();
                if elem_line.starts_with("##") || elem_line.is_empty() {
                    break;
                }
                if !elem_line.starts_with('#') {
                    let path = config.base_dir.join(elem_line);
                    config.element_files.push(path);
                }
                i += 1;
            }
            continue;
        }

        // SYMMETRY section
        if line == "SYMMETRY" {
            i += 1;
            if i < lines.len() {
                let flags_line = lines[i].trim();
                if !flags_line.starts_with('#') {
                    let flags = parse_int_line(flags_line);
                    i += 1;
                    if i < lines.len() {
                        let origin = parse_float_line(lines[i].trim());
                        config.symmetry = Some(SymmetryConfig {
                            flags: [
                                flags.first().copied().unwrap_or(0) != 0,
                                flags.get(1).copied().unwrap_or(0) != 0,
                                flags.get(2).copied().unwrap_or(0) != 0,
                            ],
                            origin: [
                                origin.first().copied().unwrap_or(0.0),
                                origin.get(1).copied().unwrap_or(0.0),
                                origin.get(2).copied().unwrap_or(0.0),
                            ],
                        });
                        i += 1;
                    }
                }
            }
            continue;
        }

        // BOUNDARY section
        if line == "BOUNDARY" {
            i += 1;
            while i < lines.len() {
                let bc_line = lines[i].trim();
                if bc_line.starts_with("##") || bc_line == "RETU" {
                    i += 1;
                    break;
                }
                if bc_line.starts_with('#') || bc_line.is_empty() {
                    i += 1;
                    continue;
                }

                if let Some(bc) = parse_boundary_line(bc_line) {
                    config.boundary_conditions.push(bc);
                }
                i += 1;
            }
            continue;
        }

        // PLANE WAVES section
        if line == "PLANE WAVES" {
            i += 1;
            while i < lines.len() {
                let pw_line = lines[i].trim();
                if pw_line.starts_with("##") || pw_line.is_empty() {
                    break;
                }
                if !pw_line.starts_with('#')
                    && let Some(pw) = parse_plane_wave_line(pw_line)
                {
                    config.plane_waves.push(pw);
                }
                i += 1;
            }
            continue;
        }

        // POINT SOURCES section
        if line == "POINT SOURCES" {
            i += 1;
            while i < lines.len() {
                let ps_line = lines[i].trim();
                if ps_line.starts_with("##") || ps_line.is_empty() {
                    break;
                }
                if !ps_line.starts_with('#')
                    && let Some(ps) = parse_point_source_line(ps_line)
                {
                    config.point_sources.push(ps);
                }
                i += 1;
            }
            continue;
        }

        // END marker
        if line == "END" {
            break;
        }

        i += 1;
    }

    Ok(config)
}

/// Parse a line of integers
fn parse_int_line(line: &str) -> Vec<i32> {
    line.split_whitespace()
        .filter_map(|s| s.parse::<i32>().ok())
        .collect()
}

/// Parse a line of floats
fn parse_float_line(line: &str) -> Vec<f64> {
    line.split_whitespace()
        .filter_map(|s| {
            // Handle scientific notation with space (e.g., "0.0 e+00")
            let s = s.replace(" ", "");
            s.parse::<f64>().ok()
        })
        .collect()
}

/// Parse a line with mixed int/float values
fn parse_mixed_line(line: &str) -> Vec<f64> {
    parse_float_line(line)
}

/// Parse a boundary condition line
fn parse_boundary_line(line: &str) -> Option<BoundarySpec> {
    let parts: Vec<&str> = line.split_whitespace().collect();

    // ELEM start TO end TYPE value curve value curve
    if parts.len() >= 9 && parts[0] == "ELEM" && parts[2] == "TO" {
        let elem_start = parts[1].parse().ok()?;
        let elem_end = parts[3].parse().ok()?;
        let bc_type = parts[4].to_string();
        let value_re = parts[5].parse().ok()?;
        let curve_re = parts[6].parse().ok()?;
        let value_im = parts[7].parse().ok()?;
        let curve_im = parts[8].parse().ok()?;

        return Some(BoundarySpec {
            elem_start,
            elem_end,
            bc_type,
            value_re,
            curve_re,
            value_im,
            curve_im,
        });
    }

    None
}

/// Parse a plane wave line
fn parse_plane_wave_line(line: &str) -> Option<PlaneWaveSource> {
    let values = parse_float_line(line);
    if values.len() >= 8 {
        Some(PlaneWaveSource {
            direction: [values[1], values[2], values[3]],
            amplitude_re: values[4],
            curve_re: values[5] as i32,
            amplitude_im: values[6],
            curve_im: values[7] as i32,
        })
    } else {
        None
    }
}

/// Parse a point source line
fn parse_point_source_line(line: &str) -> Option<PointSource> {
    let values = parse_float_line(line);
    if values.len() >= 8 {
        Some(PointSource {
            position: [values[1], values[2], values[3]],
            amplitude_re: values[4],
            curve_re: values[5] as i32,
            amplitude_im: values[6],
            curve_im: values[7] as i32,
        })
    } else {
        None
    }
}

/// Load nodes from a NumCalc nodes file
pub fn load_nc_nodes<P: AsRef<Path>>(path: P) -> Result<Array2<f64>, NcParseError> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() {
        return Ok(Array2::zeros((0, 3)));
    }

    // First line may contain node count
    let start_line = if lines[0].trim().parse::<usize>().is_ok() {
        1
    } else {
        0
    };

    let mut nodes = Vec::new();
    for line in &lines[start_line..] {
        let values = parse_float_line(line);
        if values.len() >= 4 {
            // Format: node_id x y z
            nodes.push([values[1], values[2], values[3]]);
        } else if values.len() >= 3 {
            // Format: x y z
            nodes.push([values[0], values[1], values[2]]);
        }
    }

    let n = nodes.len();
    let flat: Vec<f64> = nodes.into_iter().flatten().collect();
    Array2::from_shape_vec((n, 3), flat).map_err(|e| NcParseError::InvalidFormat(e.to_string()))
}

/// Load elements from a NumCalc elements file
pub fn load_nc_elements<P: AsRef<Path>>(
    path: P,
    property: ElementProperty,
) -> Result<Vec<Element>, NcParseError> {
    let content = fs::read_to_string(path)?;
    let lines: Vec<&str> = content.lines().collect();

    if lines.is_empty() {
        return Ok(Vec::new());
    }

    let start_line = if lines[0].trim().parse::<usize>().is_ok() {
        1
    } else {
        0
    };

    let mut elements = Vec::new();
    for (idx, line) in lines[start_line..].iter().enumerate() {
        let values: Vec<i32> = line
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();

        if values.len() >= 4 {
            // Format: elem_id n1 n2 n3 [n4]
            let connectivity: Vec<usize> = values[1..]
                .iter()
                .take_while(|&&v| v >= 0)
                .map(|&v| v as usize)
                .collect();

            let element_type = if connectivity.len() == 3 {
                ElementType::Tri3
            } else {
                ElementType::Quad4
            };

            let elem = Element {
                connectivity,
                element_type,
                property,
                normal: Array1::zeros(3),
                node_normals: Array2::zeros((element_type.num_nodes(), 3)),
                center: Array1::zeros(3),
                area: 0.0,
                boundary_condition: BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]),
                group: 0,
                dof_addresses: vec![idx],
            };
            elements.push(elem);
        }
    }

    Ok(elements)
}

/// Convert NcInputConfig to PhysicsParams
impl NcInputConfig {
    /// Create PhysicsParams from the configuration
    pub fn to_physics_params(&self, frequency: f64) -> PhysicsParams {
        PhysicsParams::new(
            frequency,
            self.main_params_iv.speed_of_sound,
            self.main_params_iv.density,
            false, // external problem
        )
    }

    /// Get the BEM method from configuration
    pub fn bem_method(&self) -> crate::core::types::BemMethod {
        match self.main_params_i.solver_method {
            0 => crate::core::types::BemMethod::Traditional,
            1 => crate::core::types::BemMethod::SingleLevelFmm,
            2 | 3 => crate::core::types::BemMethod::MultiLevelFmm,
            _ => crate::core::types::BemMethod::Traditional,
        }
    }

    /// Get solver method from configuration
    pub fn solver_method(&self) -> crate::core::types::SolverMethod {
        match self.main_params_ii.iterative_solver {
            0 => crate::core::types::SolverMethod::Cgs,
            1 => crate::core::types::SolverMethod::BiCgstab,
            _ => crate::core::types::SolverMethod::Cgs,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_NC_INP: &str = r#"##-------------------------------------------
## This file was created by mesh2input
##-------------------------------------------
Mesh2HRTF 1.0.0
##
Test Description
##
## Controlparameter I
0 0 0 0 7 0
##
## Controlparameter II
1 1 0.000001 0.00e+00 1 0 0
##
## Load Frequency Curve
0 2
0.000000 0.000000e+00 0.0
0.000001 0.400000e+04 0.0
##
## 1. Main Parameters I
2 100 50 0 0 2 1 0 0
##
## 2. Main Parameters II
1 0 0 0.0000e+00 0 0 0
##
## 3. Main Parameters III
0 0 0 0
##
## 4. Main Parameters IV
343 1.21 1.0 0.0 0.0 0.0 0.0
##
NODES
nodes.txt
##
ELEMENTS
elements.txt
##
BOUNDARY
ELEM 0 TO 49 VELO 1.0 -1 0.0 -1
RETU
##
PLANE WAVES
1 0.0 -1.0 0.0 1.0 -1 0.0 -1
##
END
"#;

    #[test]
    fn test_parse_nc_input() {
        let config = parse_nc_input_string(SAMPLE_NC_INP, PathBuf::from(".")).unwrap();

        assert!(config.version.contains("Mesh2HRTF"));
        assert_eq!(config.main_params_i.num_nodes, 100);
        assert_eq!(config.main_params_i.num_elements, 50);
        assert_eq!(config.main_params_i.solver_method, 1);
        assert!((config.main_params_iv.speed_of_sound - 343.0).abs() < 0.01);
        assert!((config.main_params_iv.density - 1.21).abs() < 0.01);
        assert_eq!(config.node_files.len(), 1);
        assert_eq!(config.element_files.len(), 1);
        assert_eq!(config.boundary_conditions.len(), 1);
        assert_eq!(config.plane_waves.len(), 1);
    }

    #[test]
    fn test_parse_boundary_line() {
        let bc = parse_boundary_line("ELEM 0 TO 100 VELO 1.0 -1 0.0 -1").unwrap();
        assert_eq!(bc.elem_start, 0);
        assert_eq!(bc.elem_end, 100);
        assert_eq!(bc.bc_type, "VELO");
        assert!((bc.value_re - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_parse_plane_wave() {
        let pw = parse_plane_wave_line("1 0.0 -1.0 0.0 1.0 -1 0.0 -1").unwrap();
        assert!((pw.direction[1] - (-1.0)).abs() < 0.001);
        assert!((pw.amplitude_re - 1.0).abs() < 0.001);
    }

    #[test]
    fn test_config_to_physics() {
        let config = parse_nc_input_string(SAMPLE_NC_INP, PathBuf::from(".")).unwrap();
        let physics = config.to_physics_params(1000.0);

        assert!((physics.speed_of_sound - 343.0).abs() < 0.01);
        assert!((physics.density - 1.21).abs() < 0.01);
        assert!((physics.frequency - 1000.0).abs() < 0.01);
    }
}
