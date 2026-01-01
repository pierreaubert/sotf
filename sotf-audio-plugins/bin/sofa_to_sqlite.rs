// ============================================================================
// SOFA to SQLite Converter
// ============================================================================
//
// This tool converts a SOFA file (.sofa) containing HRTF data into a
// SQLite database file (.hrtfdb). This allows for faster loading and avoids
// the NetCDF/HDF5 dependency on systems where it's problematic.
//
// Usage:
// cargo run --bin sofa_to_sqlite --features=sofa_support -- <input.sofa> <output.hrtfdb>
//
// ============================================================================

#[cfg(feature = "sofa_support")]
mod converter {
    use rusqlite::{Connection, Result};
    use sotf_plugins::SofaFile;
    use std::path::Path;

    // Helper to convert Vec<f32> to bytes and back
    fn f32_vec_to_bytes(vec: &[f32]) -> Vec<u8> {
        vec.iter().flat_map(|&f| f.to_le_bytes().to_vec()).collect()
    }

    pub fn convert_sofa_to_sqlite(sofa_path: &Path, db_path: &Path) -> Result<(), anyhow::Error> {
        // Load SOFA file
        log::info!("Loading SOFA file: {:?}", sofa_path);
        let sofa = SofaFile::load(sofa_path).map_err(|e| anyhow::anyhow!(e))?;
        log::info!("SOFA file loaded successfully.");

        // Create/connect to SQLite database
        let mut conn = Connection::open(db_path)?;
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
        let positions_blob = bincode::serialize(&sofa.positions)
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
}

fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    #[cfg(feature = "sofa_support")]
    {
        let args: Vec<String> = std::env::args().collect();
        if args.len() != 3 {
            log::error!("Usage: {} <input.sofa> <output.hrtfdb>", args[0]);
            std::process::exit(1);
        }

        let sofa_path = std::path::Path::new(&args[1]);
        let db_path = std::path::Path::new(&args[2]);

        if let Err(e) = converter::convert_sofa_to_sqlite(sofa_path, db_path) {
            log::error!("Failed to convert file: {}", e);
            std::process::exit(1);
        }
    }

    #[cfg(not(feature = "sofa_support"))]
    {
        log::error!("This tool was compiled without the 'sofa_support' feature.");
        log::error!(
            "Please re-run with 'cargo run --bin sofa_to_sqlite --features=sofa_support -- ...'"
        );
        std::process::exit(1);
    }
}
