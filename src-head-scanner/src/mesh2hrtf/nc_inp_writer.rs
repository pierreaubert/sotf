//! NC.inp file writer for NumCalc simulations
//!
//! This module generates NC.inp files that control NumCalc BEM simulations.
//!
//! File format documentation:
//! https://github.com/Any2HRTF/Mesh2HRTF/wiki/Structure_of_NC.inp

use crate::mesh2hrtf::source_config::{Material, SourceConfiguration};
use crate::mesh2hrtf::types::*;
use anyhow::{Context, Result};
use std::fs::File;
use std::io::Write;
use std::path::Path;

/// NumCalc project parameters
#[derive(Debug, Clone)]
pub struct NumCalcParameters {
    /// Project title
    pub title: String,

    /// Mesh2HRTF version
    pub version: String,

    /// Speed of sound (m/s)
    pub speed_of_sound: f64,

    /// Density of medium (kg/m³)
    pub density_of_medium: f64,

    /// Simulation frequencies (Hz)
    pub frequencies: Vec<f64>,

    /// BEM method
    pub bem_method: BemMethod,

    /// Evaluation grid names
    pub evaluation_grids: Vec<String>,

    /// Number of mesh nodes
    pub num_mesh_nodes: usize,

    /// Number of mesh elements
    pub num_mesh_elements: usize,

    /// Number of evaluation grid nodes
    pub num_eval_nodes: usize,

    /// Number of evaluation grid elements
    pub num_eval_elements: usize,
}

impl Default for NumCalcParameters {
    fn default() -> Self {
        Self {
            title: "Head-Related Transfer Functions".to_string(),
            version: "1.0.0".to_string(),
            speed_of_sound: 343.0,
            density_of_medium: 1.1839,
            frequencies: vec![],
            bem_method: BemMethod::MlFmmBem,
            evaluation_grids: vec![],
            num_mesh_nodes: 0,
            num_mesh_elements: 0,
            num_eval_nodes: 0,
            num_eval_elements: 0,
        }
    }
}

/// NC.inp file writer
pub struct NCInpWriter;

impl NCInpWriter {
    /// Write NC.inp file for a specific source
    pub fn write<P: AsRef<Path>>(
        path: P,
        params: &NumCalcParameters,
        source_config: &SourceConfiguration,
        source_index: usize,
    ) -> Result<()> {
        let file = File::create(path.as_ref())
            .with_context(|| format!("Failed to create NC.inp at {:?}", path.as_ref()))?;

        let mut writer = std::io::BufWriter::new(file);

        // Write header
        Self::write_header(&mut writer, params)?;

        // Write control parameters
        Self::write_control_parameters(&mut writer, params)?;

        // Write main parameters
        Self::write_main_parameters(&mut writer, params, source_config)?;

        // Write node and element paths
        Self::write_geometry_paths(&mut writer, params)?;

        // Write symmetry section (disabled)
        Self::write_symmetry(&mut writer)?;

        // Write boundary conditions
        Self::write_boundary_conditions(&mut writer, source_config, source_index)?;

        // Write source information
        Self::write_sources(&mut writer, source_config, source_index)?;

        // Write material curves (if any)
        Self::write_curves(&mut writer, source_config, source_index)?;

        // Write post-processing and end
        Self::write_footer(&mut writer)?;

        Ok(())
    }

    fn write_header(writer: &mut impl Write, params: &NumCalcParameters) -> Result<()> {
        writeln!(writer, "##-------------------------------------------")?;
        writeln!(writer, "## This file was created by mesh2hrtf (Rust)")?;
        writeln!(
            writer,
            "## Date: {}",
            chrono::Local::now().format("%Y-%m-%d")
        )?;
        writeln!(writer, "##-------------------------------------------")?;
        writeln!(writer, "Mesh2HRTF {}", params.version)?;
        writeln!(writer, "##")?;
        writeln!(writer, "{}", params.title)?;
        writeln!(writer, "##")?;
        Ok(())
    }

    fn write_control_parameters(writer: &mut impl Write, params: &NumCalcParameters) -> Result<()> {
        // Control parameter I (hard coded, not documented)
        writeln!(writer, "## Controlparameter I")?;
        writeln!(writer, "0 0 0 0 7 0")?;
        writeln!(writer, "##")?;

        // Control parameter II
        writeln!(writer, "## Controlparameter II")?;
        writeln!(
            writer,
            "1 {} 0.000001 0.00e+00 1 0 0",
            params.frequencies.len()
        )?;
        writeln!(writer, "##")?;

        // Load frequency curve
        writeln!(writer, "## Load Frequency Curve")?;
        writeln!(writer, "0 {}", params.frequencies.len() + 1)?;
        writeln!(writer, "0.000000 0.000000e+00 0.0")?;
        for (i, freq) in params.frequencies.iter().enumerate() {
            writeln!(
                writer,
                "{:.6} {:.6}e+04 0.0",
                0.000001 * (i + 1) as f64,
                freq / 10000.0
            )?;
        }
        writeln!(writer, "##")?;

        Ok(())
    }

    fn write_main_parameters(
        writer: &mut impl Write,
        params: &NumCalcParameters,
        source_config: &SourceConfiguration,
    ) -> Result<()> {
        // Main Parameters I
        writeln!(writer, "## 1. Main Parameters I")?;
        let total_elements = params.num_mesh_elements + params.num_eval_elements;
        let total_nodes = params.num_mesh_nodes + params.num_eval_nodes;
        let method_id = match params.bem_method {
            BemMethod::Bem => 0,
            BemMethod::SlFmmBem => 1,
            BemMethod::MlFmmBem => 4,
        };
        writeln!(
            writer,
            "2 {} {} 0 0 2 1 {} 0",
            total_elements, total_nodes, method_id
        )?;
        writeln!(writer, "##")?;

        // Main Parameters II
        writeln!(writer, "## 2. Main Parameters II")?;
        if source_config.is_ear_source() {
            write!(writer, "0 0 ")?;
        } else if source_config.is_plane_wave() {
            write!(writer, "1 0 ")?;
        } else if source_config.is_point_source() {
            write!(writer, "0 1 ")?;
        }
        writeln!(writer, "0 0.0000e+00 0 0 0")?;
        writeln!(writer, "##")?;

        // Main Parameters III
        writeln!(writer, "## 3. Main Parameters III")?;
        writeln!(writer, "0 0 0 0")?;
        writeln!(writer, "##")?;

        // Main Parameters IV
        writeln!(writer, "## 4. Main Parameters IV")?;
        writeln!(
            writer,
            "{} {:.4}e+00 1.0 0.0e+00 0.0 e+00 0.0e+00 0.0e+00",
            params.speed_of_sound, params.density_of_medium
        )?;
        writeln!(writer, "##")?;

        Ok(())
    }

    fn write_geometry_paths(writer: &mut impl Write, params: &NumCalcParameters) -> Result<()> {
        // Nodes
        writeln!(writer, "NODES")?;
        writeln!(writer, "../../ObjectMeshes/Reference/Nodes.txt")?;
        for grid in &params.evaluation_grids {
            writeln!(writer, "../../EvaluationGrids/{}/Nodes.txt", grid)?;
        }
        writeln!(writer, "##")?;

        // Elements
        writeln!(writer, "ELEMENTS")?;
        writeln!(writer, "../../ObjectMeshes/Reference/Elements.txt")?;
        for grid in &params.evaluation_grids {
            writeln!(writer, "../../EvaluationGrids/{}/Elements.txt", grid)?;
        }
        writeln!(writer, "##")?;

        Ok(())
    }

    fn write_symmetry(writer: &mut impl Write) -> Result<()> {
        writeln!(writer, "# SYMMETRY")?;
        writeln!(writer, "# 0 0 0")?;
        writeln!(writer, "# 0.0000e+00 0.0000e+00 0.0000e+00")?;
        writeln!(writer, "##")?;
        Ok(())
    }

    fn write_boundary_conditions(
        writer: &mut impl Write,
        source_config: &SourceConfiguration,
        source_index: usize,
    ) -> Result<()> {
        writeln!(writer, "BOUNDARY")?;

        if source_config.is_ear_source() && source_index < source_config.materials.len() {
            let materials = &source_config.materials[source_index];

            for (name, material) in materials.iter() {
                writeln!(writer, "# {} velocity source", name)?;
                writeln!(
                    writer,
                    "ELEM {} TO {} {} 0.1 -1 0.0 -1",
                    material.index_start,
                    material.index_end,
                    material.boundary_type.to_string()
                )?;
            }
        }

        writeln!(writer, "RETU")?;
        writeln!(writer, "##")?;
        Ok(())
    }

    fn write_sources(
        writer: &mut impl Write,
        source_config: &SourceConfiguration,
        source_index: usize,
    ) -> Result<()> {
        if source_config.is_point_source() {
            writeln!(writer, "POINT SOURCES")?;
            if source_index < source_config.source_positions.len() {
                let pos = &source_config.source_positions[source_index];
                writeln!(
                    writer,
                    "0 {:.6} {:.6} {:.6} 0.1 -1 0.0 -1",
                    pos.x, pos.y, pos.z
                )?;
            }
        } else if source_config.is_plane_wave() {
            writeln!(writer, "PLANE WAVES")?;
            if source_index < source_config.source_positions.len() {
                let pos = &source_config.source_positions[source_index];
                // For plane wave, position represents direction
                writeln!(
                    writer,
                    "1 {:.6} {:.6} {:.6} 1.0 -1 0.0 -1",
                    pos.x, pos.y, pos.z
                )?;
            }
        }
        writeln!(writer, "##")?;
        Ok(())
    }

    fn write_curves(
        writer: &mut impl Write,
        _source_config: &SourceConfiguration,
        _source_index: usize,
    ) -> Result<()> {
        // Placeholder for material curves
        // In the future, this will write frequency-dependent material properties
        writeln!(writer, "# CURVES")?;
        writeln!(writer, "# Frequency Factor 0.0")?;
        writeln!(writer, "##")?;
        Ok(())
    }

    fn write_footer(writer: &mut impl Write) -> Result<()> {
        writeln!(writer, "POST PROCESS")?;
        writeln!(writer, "##")?;
        writeln!(writer, "END")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mesh2hrtf::source_config::SourceConfiguration;
    use nalgebra::Point3;
    use std::io::Cursor;

    #[test]
    fn test_write_header() {
        let mut output = Cursor::new(Vec::new());
        let params = NumCalcParameters::default();

        NCInpWriter::write_header(&mut output, &params).unwrap();

        let result = String::from_utf8(output.into_inner()).unwrap();
        assert!(result.contains("Mesh2HRTF 1.0.0"));
        assert!(result.contains("Head-Related Transfer Functions"));
    }

    #[test]
    fn test_write_control_parameters() {
        let mut output = Cursor::new(Vec::new());
        let mut params = NumCalcParameters::default();
        params.frequencies = vec![1000.0, 2000.0, 4000.0];

        NCInpWriter::write_control_parameters(&mut output, &params).unwrap();

        let result = String::from_utf8(output.into_inner()).unwrap();
        assert!(result.contains("1 3 0.000001"));
        assert!(result.contains("0 4")); // 3 frequencies + 1
    }

    #[test]
    fn test_point_source_output() {
        let mut output = Cursor::new(Vec::new());
        let pos = Point3::new(1.0, 2.0, 3.0);
        let config = SourceConfiguration::point_source(pos);

        NCInpWriter::write_sources(&mut output, &config, 0).unwrap();

        let result = String::from_utf8(output.into_inner()).unwrap();
        assert!(result.contains("POINT SOURCES"));
        assert!(result.contains("1.000000 2.000000 3.000000"));
    }

    #[test]
    fn test_plane_wave_output() {
        let mut output = Cursor::new(Vec::new());
        let dir = Vec3::new(0.0, -1.0, 0.0);
        let config = SourceConfiguration::plane_wave(dir);

        NCInpWriter::write_sources(&mut output, &config, 0).unwrap();

        let result = String::from_utf8(output.into_inner()).unwrap();
        assert!(result.contains("PLANE WAVES"));
        assert!(result.contains("0.000000 -1.000000 0.000000"));
    }
}
