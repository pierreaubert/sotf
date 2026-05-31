// ============================================================================
// SOFA to SQLite Converter
// ============================================================================
//
// This tool converts a SOFA file (.sofa) containing HRTF data into a
// SQLite database file (.hrtfdb). This allows for faster loading.
//
// Usage:
// cargo run --bin sofa-to-sqlite -- <input.sofa> <output.hrtfdb>
//
// ============================================================================

use anyhow::{Context, Result};
use clap::Parser;
use rusqlite::Connection;
use sotf_plugins::SofaFile;
use std::path::{Path, PathBuf};

#[derive(Parser)]
#[command(name = "sofa-to-sqlite")]
#[command(about = "Convert a SOFA HRTF file to a SQLite .hrtfdb database")]
struct Cli {
    /// Input SOFA file
    input: PathBuf,

    /// Output SQLite database path
    output: PathBuf,
}

// Helper to convert Vec<f32> to bytes
fn f32_vec_to_bytes(vec: &[f32]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(vec.len() * std::mem::size_of::<f32>());
    for &sample in vec {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

fn convert_sofa_to_sqlite(sofa_path: &Path, db_path: &Path) -> Result<()> {
    // Load SOFA file
    log::info!("Loading SOFA file: {:?}", sofa_path);
    let sofa = SofaFile::load(sofa_path).map_err(|e| anyhow::anyhow!(e))?;
    log::info!("SOFA file loaded successfully.");

    // Create/connect to SQLite database
    let mut conn =
        Connection::open(db_path).with_context(|| format!("open {}", db_path.display()))?;
    log::info!("Opened database: {:?}", db_path);

    // Create schema
    conn.execute(
        "CREATE TABLE metadata (
            key     TEXT PRIMARY KEY,
            value   TEXT NOT NULL
        )",
        [],
    )?;
    conn.execute(
        "CREATE TABLE data (
            key     TEXT PRIMARY KEY,
            value   BLOB NOT NULL
        )",
        [],
    )?;
    log::info!("Database schema created.");

    // Insert metadata
    let tx = conn.transaction()?;
    tx.execute(
        "INSERT INTO metadata (key, value) VALUES (?1, ?2)",
        ["convention", &sofa.convention],
    )?;
    tx.execute(
        "INSERT INTO metadata (key, value) VALUES (?1, ?2)",
        ["sample_rate", &sofa.sample_rate.to_string()],
    )?;
    tx.execute(
        "INSERT INTO metadata (key, value) VALUES (?1, ?2)",
        ["ir_length", &sofa.ir_length.to_string()],
    )?;
    tx.execute(
        "INSERT INTO metadata (key, value) VALUES (?1, ?2)",
        ["num_measurements", &sofa.num_measurements.to_string()],
    )?;
    if let Some(dsr) = sofa.data_sample_rate {
        tx.execute(
            "INSERT INTO metadata (key, value) VALUES (?1, ?2)",
            ["data_sample_rate", &dsr.to_string()],
        )?;
    }

    // Serialize and insert large data as blobs
    let positions_blob =
        bincode::serde::encode_to_vec(&sofa.positions, bincode::config::standard())
            .map_err(|e| anyhow::anyhow!("Failed to serialize positions: {}", e))?;
    let ir_blob = f32_vec_to_bytes(&sofa.impulse_responses);

    tx.execute(
        "INSERT INTO data (key, value) VALUES (?1, ?2)",
        rusqlite::params!["positions", &positions_blob],
    )?;
    tx.execute(
        "INSERT INTO data (key, value) VALUES (?1, ?2)",
        rusqlite::params!["impulse_responses", &ir_blob],
    )?;

    tx.commit()?;
    log::info!("Data inserted into database.");

    Ok(())
}

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let cli = Cli::parse();
    convert_sofa_to_sqlite(&cli.input, &cli.output).with_context(|| {
        format!(
            "convert {} to {}",
            cli.input.display(),
            cli.output.display()
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn f32_vec_to_bytes_writes_little_endian_samples_without_padding() {
        let samples = [1.0_f32, -2.5, 0.0];
        let bytes = f32_vec_to_bytes(&samples);
        let expected: Vec<u8> = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();

        assert_eq!(bytes, expected);
        assert_eq!(bytes.len(), samples.len() * 4);
    }
}
