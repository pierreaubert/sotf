use serde_json::Value;
use std::error::Error;
use strsim::levenshtein;

/// Represents an API error response from the spinorama API
#[derive(Debug)]
pub struct ApiError {
    pub message: String,
    pub speaker_name: Option<String>,
}

impl ApiError {
    /// Parse an API error response and extract the speaker name if present
    pub fn from_json(value: &Value) -> Option<Self> {
        if let Some(error_obj) = value.as_object()
            && let Some(error_message) = error_obj.get("error").and_then(|v| v.as_str())
        {
            let speaker_name = extract_speaker_name_from_error(error_message);
            return Some(ApiError {
                message: error_message.to_string(),
                speaker_name,
            });
        }
        None
    }
}

/// Extract speaker name from error messages like "Speaker ASCILAB F6B is not in our database!"
fn extract_speaker_name_from_error(error_message: &str) -> Option<String> {
    // Pattern: "Speaker <name> is not in our database!"
    if error_message.starts_with("Speaker ") && error_message.contains(" is not in our database!") {
        let start = "Speaker ".len();
        if let Some(end) = error_message.find(" is not in our database!")
            && end > start
        {
            return Some(error_message[start..end].to_string());
        }
    }
    None
}

/// Fetch the list of available speakers from the API
pub async fn fetch_available_speakers() -> Result<Vec<String>, Box<dyn Error>> {
    let url = "https://api.spinorama.org/v1/speakers";

    let response = reqwest::get(url).await?;
    if !response.status().is_success() {
        return Err(format!("Failed to fetch speakers list: {}", response.status()).into());
    }

    let api_response: Value = response.json().await?;

    // Parse the response as an array of speaker names
    if let Some(speakers_array) = api_response.as_array() {
        let mut speakers = Vec::new();

        for speaker in speakers_array {
            if let Some(speaker_name) = speaker.as_str() {
                speakers.push(speaker_name.to_string());
            }
        }

        return Ok(speakers);
    }

    Err("Invalid response format from speakers API - expected array".into())
}

/// Find the closest matching speaker names using Levenshtein distance
pub fn find_similar_speakers(
    invalid_speaker: &str,
    available_speakers: &[String],
    max_suggestions: usize,
) -> Vec<String> {
    if available_speakers.is_empty() {
        return Vec::new();
    }

    let mut scored_speakers: Vec<(usize, &String)> = available_speakers
        .iter()
        .map(|speaker| {
            let distance = calculate_similarity_score(invalid_speaker, speaker);
            (distance, speaker)
        })
        .collect();

    // Sort by distance (lower is better)
    scored_speakers.sort_by_key(|&(distance, _)| distance);

    // Take the best matches and filter out those that are too dissimilar
    scored_speakers
        .into_iter()
        .take(max_suggestions)
        .filter(|&(distance, speaker)| is_reasonable_match(invalid_speaker, speaker, distance))
        .map(|(_, speaker)| speaker.clone())
        .collect()
}

/// Calculate a similarity score between two speaker names
/// Lower scores indicate higher similarity
fn calculate_similarity_score(target: &str, candidate: &str) -> usize {
    let target_lower = target.to_lowercase();
    let candidate_lower = candidate.to_lowercase();

    // Exact case-insensitive match gets the best score
    if target_lower == candidate_lower {
        return 0;
    }

    // Check if one contains the other (substring match)
    if target_lower.contains(&candidate_lower) || candidate_lower.contains(&target_lower) {
        return 1;
    }

    // Use Levenshtein distance for general similarity
    let distance = levenshtein(&target_lower, &candidate_lower);

    // Add a small penalty for length differences to prefer similar-length matches
    let length_diff = (target.len() as isize - candidate.len() as isize).unsigned_abs();
    distance + length_diff / 4
}

/// Determine if a match is reasonable based on the similarity score
fn is_reasonable_match(target: &str, candidate: &str, score: usize) -> bool {
    let max_length = target.len().max(candidate.len());

    // For short strings, be more strict
    if max_length <= 5 {
        return score <= 2;
    }

    // For longer strings, allow more variation
    let threshold = (max_length / 3).clamp(2, 6);
    score <= threshold
}

/// Format a user-friendly error message with speaker suggestions
pub fn format_speaker_not_found_error(invalid_speaker: &str, suggestions: &[String]) -> String {
    if suggestions.is_empty() {
        format!(
            "Speaker '{}' not found in the database. Please check the speaker name and try again.",
            invalid_speaker
        )
    } else {
        let suggestion_list = suggestions
            .iter()
            .map(|s| format!("'{}'", s))
            .collect::<Vec<_>>()
            .join(", ");

        format!(
            "Speaker '{}' not found in the database. Did you mean: {}?",
            invalid_speaker, suggestion_list
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_speaker_name_from_error() {
        let error_msg = "Speaker ASCILAB F6B is not in our database!";
        let extracted = extract_speaker_name_from_error(error_msg);
        assert_eq!(extracted, Some("ASCILAB F6B".to_string()));

        let invalid_msg = "Some other error message";
        let extracted = extract_speaker_name_from_error(invalid_msg);
        assert_eq!(extracted, None);
    }

    #[test]
    fn test_calculate_similarity_score() {
        // Exact case-insensitive match
        assert_eq!(
            calculate_similarity_score("Sony WH-1000XM5", "sony wh-1000xm5"),
            0
        );

        // Substring match
        assert_eq!(calculate_similarity_score("Sony", "Sony WH-1000XM5"), 1);

        // Levenshtein distance
        assert!(calculate_similarity_score("Sony WH-1000XM5", "Sony WH-1000XM4") < 5);
    }

    #[test]
    fn test_find_similar_speakers() {
        let available = vec![
            "Sony WH-1000XM5".to_string(),
            "Sony WH-1000XM4".to_string(),
            "Sony WH-1000XM3".to_string(),
            "Audio-Technica ATH-M50x".to_string(),
            "Sennheiser HD 660S".to_string(),
        ];

        let suggestions = find_similar_speakers("Sony WH-1000XM6", &available, 3);
        assert!(!suggestions.is_empty());
        assert!(suggestions.contains(&"Sony WH-1000XM5".to_string()));
    }

    #[test]
    fn test_format_speaker_not_found_error() {
        let suggestions = vec!["Sony WH-1000XM5".to_string(), "Sony WH-1000XM4".to_string()];
        let error_msg = format_speaker_not_found_error("Sony WH-1000XM6", &suggestions);
        assert!(error_msg.contains("Sony WH-1000XM6"));
        assert!(error_msg.contains("Did you mean"));
        assert!(error_msg.contains("Sony WH-1000XM5"));
    }
}
