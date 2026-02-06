//! CSV export for simulation results
//!
//! Converts complex pressure data to CSV files with freq, spl, phase columns.

use anyhow::Result;
use math_audio_xem_common::pressure_to_spl;
use std::io::Write;
use std::path::Path;

use crate::SimulationOutput;

/// Export simulation output to CSV files.
///
/// Creates one CSV per (source, listening position) pair with columns:
///   freq,spl,phase
///
/// Files are named `{source_name}_lp{lp_idx}.csv` under `output_dir`.
pub fn export_csvs(output: &SimulationOutput, output_dir: &Path) -> Result<Vec<String>> {
    std::fs::create_dir_all(output_dir)?;

    let mut files_written = Vec::new();

    for (src_idx, source_name) in output.source_names.iter().enumerate() {
        let n_lps = output.pressures[src_idx].len();
        for lp_idx in 0..n_lps {
            let filename = format!("{}_lp{}.csv", source_name, lp_idx);
            let filepath = output_dir.join(&filename);

            let mut file = std::fs::File::create(&filepath)?;
            writeln!(file, "freq,spl,phase")?;

            for (freq_idx, &freq) in output.frequencies.iter().enumerate() {
                let pressure = output.pressures[src_idx][lp_idx][freq_idx];
                let spl = pressure_to_spl(pressure);
                let phase = pressure.arg().to_degrees();

                // Cap extreme SPL values
                let spl_clamped = if spl.is_finite() {
                    spl.clamp(-120.0, 150.0)
                } else {
                    -120.0
                };
                let phase_clean = if phase.is_finite() { phase } else { 0.0 };

                writeln!(file, "{:.4},{:.4},{:.4}", freq, spl_clamped, phase_clean)?;
            }

            files_written.push(filename);
        }
    }

    Ok(files_written)
}
