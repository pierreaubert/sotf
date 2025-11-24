use crate::read::plot;
use crate::read::speaker_suggestions::{
    ApiError, fetch_available_speakers, find_similar_speakers, format_speaker_not_found_error,
};
use serde_json::Value;
use std::error::Error;

pub fn normalize_plotly_value(v: &Value) -> Result<Value, Box<dyn Error>> {
    // API format is ["{...plotly json...}"]
    if let Some(arr) = v.as_array() {
        if let Some(first) = arr.first() {
            if let Some(s) = first.as_str() {
                let parsed: Value = serde_json::from_str(s)?;
                return Ok(parsed);
            } else {
                return Err("First element is not a string".into());
            }
        } else {
            return Err("Empty API response".into());
        }
    }
    Err("API response is not an array".into())
}

/// Enhanced version of normalize_plotly_value that provides helpful suggestions for speaker errors
pub async fn normalize_plotly_value_with_suggestions(v: &Value) -> Result<Value, Box<dyn Error>> {
    // First try the normal processing
    if let Some(arr) = v.as_array() {
        if let Some(first) = arr.first() {
            if let Some(s) = first.as_str() {
                let parsed: Value = serde_json::from_str(s)?;
                return Ok(parsed);
            } else {
                return Err("First element is not a string".into());
            }
        } else {
            return Err("Empty API response".into());
        }
    }

    // If it's not an array, check if it's a speaker error
    if let Some(api_error) = ApiError::from_json(v) {
        if let Some(invalid_speaker) = api_error.speaker_name {
            // Try to fetch available speakers and provide suggestions
            match fetch_available_speakers().await {
                Ok(available_speakers) => {
                    let suggestions =
                        find_similar_speakers(&invalid_speaker, &available_speakers, 5);
                    let helpful_error =
                        format_speaker_not_found_error(&invalid_speaker, &suggestions);
                    return Err(helpful_error.into());
                }
                Err(_) => {
                    // Fallback to original error message if we can't fetch speakers
                    let fallback_error = format!(
                        "Speaker '{}' not found in the database. Unable to fetch suggestions at this time. Please check the speaker name and try again.",
                        invalid_speaker
                    );
                    return Err(fallback_error.into());
                }
            }
        } else {
            // Return the original API error if we couldn't extract speaker name
            return Err(api_error.message.into());
        }
    }

    // Fallback to the original error message
    Err("API response is not an array".into())
}

pub fn normalize_plotly_json_from_str(content: &str) -> Result<Value, Box<dyn Error>> {
    // Content could be one of:
    // - Already a Plotly JSON object with "data" key
    // - A JSON array with one string (API response)
    // - A bare JSON string containing the Plotly JSON
    let v: Value = serde_json::from_str(content)?;
    if v.is_object() {
        return Ok(v);
    }
    if let Ok(parsed) = plot::normalize_plotly_value(&v) {
        return Ok(parsed);
    }
    if let Some(s) = v.as_str() {
        let inner: Value = serde_json::from_str(s)?;
        return Ok(inner);
    }
    Err("Unrecognized cached JSON format".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn normalize_plotly_handles_object_array_and_string() {
        // Case 1: already a Plotly object
        let obj = json!({"data": [{"name": "On Axis"}]});
        let s1 = serde_json::to_string(&obj).unwrap();
        let p1 = normalize_plotly_json_from_str(&s1).unwrap();
        assert!(p1.get("data").is_some());

        // Case 2: API array-of-string format
        let inner = json!({"data": [{"name": "Listening Window"}]});
        let s_inner = serde_json::to_string(&inner).unwrap();
        let api = json!([s_inner]);
        let s2 = serde_json::to_string(&api).unwrap();
        let p2 = normalize_plotly_json_from_str(&s2).unwrap();
        assert!(p2.get("data").is_some());

        // Case 3: bare JSON string containing the Plotly JSON
        let s3 = serde_json::to_string(&s_inner).unwrap();
        let p3 = normalize_plotly_json_from_str(&s3).unwrap();
        assert!(p3.get("data").is_some());
    }

    #[test]
    fn test_normalize_plotly_value_with_valid_array() {
        // Test with valid array response
        let plotly_data = json!({"data": [{"name": "Test"}]});
        let plotly_str = serde_json::to_string(&plotly_data).unwrap();
        let api_response = json!([plotly_str]);

        let result = normalize_plotly_value(&api_response).unwrap();
        assert_eq!(result, plotly_data);
    }

    #[test]
    fn test_normalize_plotly_value_with_error_object() {
        // Test with speaker not found error
        let error_response = json!({
            "error": "Speaker ASCILAB F6B is not in our database!"
        });

        let result = normalize_plotly_value(&error_response);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert_eq!(error_msg, "API response is not an array");
    }

    #[test]
    fn test_normalize_plotly_value_with_empty_array() {
        // Test with empty array
        let empty_array = json!([]);

        let result = normalize_plotly_value(&empty_array);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert_eq!(error_msg, "Empty API response");
    }

    #[test]
    fn test_normalize_plotly_value_with_invalid_content() {
        // Test with array containing non-string
        let invalid_array = json!([123]);

        let result = normalize_plotly_value(&invalid_array);
        assert!(result.is_err());
        let error_msg = result.unwrap_err().to_string();
        assert_eq!(error_msg, "First element is not a string");
    }

    #[tokio::test]
    async fn test_normalize_with_suggestions_detects_speaker_error() {
        // Test that the enhanced version properly detects speaker errors
        let error_response = json!({
            "error": "Speaker Test Speaker is not in our database!"
        });

        let result = normalize_plotly_value_with_suggestions(&error_response).await;
        assert!(result.is_err());

        let error_msg = result.unwrap_err().to_string();
        // The error should either contain suggestions or indicate that suggestions couldn't be fetched
        assert!(error_msg.contains("Test Speaker"));
    }
}
