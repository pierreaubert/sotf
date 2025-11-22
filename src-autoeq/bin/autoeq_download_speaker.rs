use std::path::PathBuf;
use std::{error::Error, io};

use autoeq::read;
use clap::Parser;
use reqwest::Client;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::fs;

const BASE_URL: &str = "https://api.spinorama.org";

/// Download speaker measurements from spinorama.org API
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Force re-download of existing measurements
    #[arg(short, long, default_value_t = false)]
    force: bool,

    /// Download only measurements for a specific speaker (case-insensitive substring match)
    #[arg(short, long)]
    speaker: Option<String>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    let client = Client::new();

    let speakers: Vec<String> = fetch_json(&client, &format!("{}/v1/speakers", BASE_URL)).await?;
    println!("Found {} speakers", speakers.len());

    // Filter speakers if --speaker flag is provided
    let speakers_to_process: Vec<String> = if let Some(ref filter) = args.speaker {
        let filter_lower = filter.to_lowercase();
        speakers
            .into_iter()
            .filter(|s| s.to_lowercase().contains(&filter_lower))
            .collect()
    } else {
        speakers
    };

    if speakers_to_process.is_empty() {
        if let Some(ref filter) = args.speaker {
            eprintln!("No speakers found matching '{}'", filter);
            return Ok(());
        }
    }

    println!("Processing {} speaker(s)", speakers_to_process.len());

    for speaker in speakers_to_process {
        if let Err(e) = process_speaker(&client, &speaker, args.force).await {
            eprintln!("[WARN] Skipping speaker '{}': {}", speaker, e);
        }
    }

    Ok(())
}

async fn process_speaker(
    client: &Client,
    speaker: &str,
    force: bool,
) -> Result<(), Box<dyn Error>> {
    let enc_speaker = urlencoding::encode(speaker);

    // 1. versions
    let versions_url = format!("{}/v1/speaker/{}/versions", BASE_URL, enc_speaker);
    let versions: Vec<String> = fetch_json(client, &versions_url).await?;
    if versions.is_empty() {
        return Ok(());
    }
    let version = &versions[0];

    // 2. measurements list for the first version
    let enc_version = urlencoding::encode(version);
    let measurements_url = format!(
        "{}/v1/speaker/{}/version/{}/measurements",
        BASE_URL, enc_speaker, enc_version
    );
    let measurements: Vec<String> = fetch_json(client, &measurements_url).await?;

    // 3. if CEA2034 present, download CEA2034 and Estimated In-Room Response and metadata
    if measurements.iter().any(|m| m == "CEA2034") {
        let dir = read::data_dir_for(speaker);

        // Check if measurements already exist (unless --force is specified)
        if !force {
            let cea2034_file = dir.join("CEA2034.json");
            let in_room_file = dir.join("Estimated In-Room Response.json");
            let metadata_file = dir.join("metadata.json");

            // If all files exist, skip downloading
            if cea2034_file.exists() && in_room_file.exists() && metadata_file.exists() {
                println!(
                    "Skipping '{}': measurements already cached (use --force to re-download)",
                    speaker
                );
                return Ok(());
            }
        }

        // Create directory if needed
        fs::create_dir_all(&dir).await?;

        // metadata
        let metadata_url = format!("{}/v1/speaker/{}/metadata", BASE_URL, enc_speaker);
        let metadata: Value = fetch_json(client, &metadata_url).await?;
        write_json(&dir.join("metadata.json"), &metadata).await?;

        // Measurements: leverage shared cache-aware fetcher, which also saves to disk
        // Note: We need to delete existing cache files first if force is enabled
        // because fetch_measurement_plot_data will use cache if it exists
        if force {
            let cea2034_file = dir.join("CEA2034.json");
            let in_room_file = dir.join("Estimated In-Room Response.json");
            let _ = fs::remove_file(&cea2034_file).await;
            let _ = fs::remove_file(&in_room_file).await;
        }

        let _ = read::fetch_measurement_plot_data(speaker, version, "CEA2034").await?;
        let _ = read::fetch_measurement_plot_data(speaker, version, "Estimated In-Room Response")
            .await?;

        println!(
            "Saved CEA2034, Estimated In-Room Response and metadata for '{}' (version '{}')",
            speaker, version
        );
    }

    Ok(())
}

async fn fetch_json<T: DeserializeOwned>(client: &Client, url: &str) -> Result<T, Box<dyn Error>> {
    let resp = client.get(url).send().await?;
    if !resp.status().is_success() {
        let err = io::Error::other(format!("HTTP {} for {}", resp.status(), url));
        return Err(Box::new(err));
    }
    let val = resp.json::<T>().await?;
    Ok(val)
}

async fn write_json(path: &PathBuf, value: &Value) -> Result<(), Box<dyn Error>> {
    let data = serde_json::to_vec_pretty(value)?;
    fs::write(path, data).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use autoeq::read;
    use autoeq_env::DATA_CACHED;

    #[test]
    fn sanitize_replaces_forbidden() {
        assert_eq!(
            autoeq::read::sanitize_dir_name("A/B\\C|D?E*F: G"),
            "A_B_C_D_E_F_ G"
        );
    }

    #[test]
    fn data_dir_builds_expected_path() {
        let p = read::data_dir_for("KEF LS50 Meta");
        let expected = std::path::Path::new(DATA_CACHED).join("speakers/org.spinorama/KEF LS50 Meta");
        assert!(p.ends_with(expected));
    }
}
