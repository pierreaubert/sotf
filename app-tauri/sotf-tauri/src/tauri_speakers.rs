#[tauri::command]
pub async fn get_speakers() -> Result<Vec<String>, String> {
    match reqwest::get("https://api.spinorama.org/v1/speakers").await {
        Ok(response) => match response.json::<serde_json::Value>().await {
            Ok(data) => {
                if let Some(speakers) = data.as_array() {
                    let speaker_names: Vec<String> = speakers
                        .iter()
                        .filter_map(|s| s.as_str())
                        .map(|s| s.to_string())
                        .collect();
                    Ok(speaker_names)
                } else {
                    Err("Invalid response format".to_string())
                }
            }
            Err(e) => Err(format!("Failed to parse response: {}", e)),
        },
        Err(e) => Err(format!("Failed to fetch speakers: {}", e)),
    }
}

#[tauri::command]
pub async fn get_speaker_versions(speaker: String) -> Result<Vec<String>, String> {
    let url = format!(
        "https://api.spinorama.org/v1/speaker/{}/versions",
        urlencoding::encode(&speaker)
    );
    match reqwest::get(&url).await {
        Ok(response) => match response.json::<serde_json::Value>().await {
            Ok(data) => {
                if let Some(versions) = data.as_array() {
                    let version_names: Vec<String> = versions
                        .iter()
                        .filter_map(|v| v.as_str())
                        .map(|v| v.to_string())
                        .collect();
                    Ok(version_names)
                } else {
                    Err("Invalid response format".to_string())
                }
            }
            Err(e) => Err(format!("Failed to parse response: {}", e)),
        },
        Err(e) => Err(format!("Failed to fetch versions: {}", e)),
    }
}

#[tauri::command]
pub async fn get_speaker_measurements(
    speaker: String,
    version: String,
) -> Result<Vec<String>, String> {
    let url = format!(
        "https://api.spinorama.org/v1/speaker/{}/version/{}/measurements",
        urlencoding::encode(&speaker),
        urlencoding::encode(&version)
    );
    match reqwest::get(&url).await {
        Ok(response) => match response.json::<serde_json::Value>().await {
            Ok(data) => {
                if let Some(measurements) = data.as_array() {
                    let measurement_names: Vec<String> = measurements
                        .iter()
                        .filter_map(|m| m.as_str())
                        .map(|m| m.to_string())
                        .collect();
                    Ok(measurement_names)
                } else {
                    Err("Invalid response format".to_string())
                }
            }
            Err(e) => Err(format!("Failed to parse response: {}", e)),
        },
        Err(e) => Err(format!("Failed to fetch measurements: {}", e)),
    }
}
