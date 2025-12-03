use autoeq::iir::BiquadFilterType;
use serde::{Deserialize, Serialize};

/// Filter parameter for EQ response computation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterParam {
    pub filter_type: String, // "Peak", "Lowpass", "Highpass", etc.
    pub frequency: f64,
    pub q: f64,
    pub gain: f64,
    pub enabled: bool,
}

/// Filter response containing magnitude values in dB
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FilterResponse {
    pub magnitudes_db: Vec<f64>,
}

/// Combined EQ response computation result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EqResponseResult {
    pub frequencies: Vec<f64>,
    pub individual_responses: Vec<FilterResponse>,
    pub combined_response: Vec<f64>, // Combined magnitude in dB
}

/// Parse filter type string to BiquadFilterType enum
fn parse_filter_type(type_str: &str) -> Option<BiquadFilterType> {
    match type_str.to_lowercase().as_str() {
        "peak" => Some(BiquadFilterType::Peak),
        "lowpass" => Some(BiquadFilterType::Lowpass),
        "highpass" => Some(BiquadFilterType::Highpass),
        "highpassvariableq" => Some(BiquadFilterType::HighpassVariableQ),
        "bandpass" => Some(BiquadFilterType::Bandpass),
        "notch" => Some(BiquadFilterType::Notch),
        "lowshelf" => Some(BiquadFilterType::Lowshelf),
        "highshelf" => Some(BiquadFilterType::Highshelf),
        _ => None,
    }
}

/// Compute frequency response for a list of filters

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_filter_type() {
        assert!(matches!(
            parse_filter_type("peak"),
            Some(BiquadFilterType::Peak)
        ));
        assert!(matches!(
            parse_filter_type("Peak"),
            Some(BiquadFilterType::Peak)
        ));
        assert!(matches!(
            parse_filter_type("lowpass"),
            Some(BiquadFilterType::Lowpass)
        ));
        assert!(parse_filter_type("invalid").is_none());
    }

    #[tokio::test]
    async fn test_compute_eq_response() {
        let filters = vec![FilterParam {
            filter_type: "Peak".to_string(),
            frequency: 1000.0,
            q: 1.0,
            gain: 3.0,
            enabled: true,
        }];

        let sample_rate = 48000.0;
        let frequencies = vec![100.0, 1000.0, 10000.0];

        let result = compute_eq_response(filters, sample_rate, frequencies)
            .await
            .unwrap();

        assert_eq!(result.frequencies.len(), 3);
        assert_eq!(result.individual_responses.len(), 1);
        assert_eq!(result.combined_response.len(), 3);

        // At 1000 Hz (center frequency), gain should be close to +3 dB
        assert!((result.combined_response[1] - 3.0).abs() < 0.1);
    }

    #[tokio::test]
    async fn test_disabled_filter() {
        let filters = vec![FilterParam {
            filter_type: "Peak".to_string(),
            frequency: 1000.0,
            q: 1.0,
            gain: 3.0,
            enabled: false,
        }];

        let sample_rate = 48000.0;
        let frequencies = vec![1000.0];

        let result = compute_eq_response(filters, sample_rate, frequencies)
            .await
            .unwrap();

        // Disabled filter should contribute 0 dB
        assert_eq!(result.combined_response[0], 0.0);
    }
}

#[tauri::command]
pub async fn compute_eq_response(
    filters: Vec<FilterParam>,
    sample_rate: f64,
    frequencies: Vec<f64>,
) -> Result<EqResponseResult, String> {
    println!(
        "[TAURI] Computing response for {} filters at {} points",
        filters.len(),
        frequencies.len()
    );

    // Convert filters to PEQ format
    let freq_array = ndarray::Array1::from_vec(frequencies.clone());
    let mut peq: Vec<(f64, autoeq::iir::Biquad)> = Vec::new();
    let mut individual_responses = Vec::new();

    for filter_param in filters.iter() {
        if !filter_param.enabled {
            // Skip disabled filters but add empty response
            individual_responses.push(FilterResponse {
                magnitudes_db: vec![0.0; frequencies.len()],
            });
            continue;
        }

        // Parse filter type
        let filter_type = parse_filter_type(&filter_param.filter_type)
            .ok_or_else(|| format!("Invalid filter type: {}", filter_param.filter_type))?;

        // Create biquad filter
        let biquad = autoeq::iir::Biquad::new(
            filter_type,
            filter_param.frequency,
            sample_rate,
            filter_param.q,
            filter_param.gain,
        );

        // Compute individual filter response
        let response_array = biquad.np_log_result(&freq_array);
        let magnitudes_db: Vec<f64> = response_array.to_vec();

        individual_responses.push(FilterResponse { magnitudes_db });

        // Add to PEQ for combined response (weight = 1.0)
        peq.push((1.0, biquad));
    }

    // Compute combined response
    let combined_array = autoeq::iir::compute_peq_response(&freq_array, &peq, sample_rate);
    let combined_response: Vec<f64> = combined_array.to_vec();

    Ok(EqResponseResult {
        frequencies,
        individual_responses,
        combined_response,
    })
}
