use std::collections::HashMap;
use std::error::Error;

use ndarray::Array1;
use reqwest;
use serde_json::Value;
use tokio::fs;
use urlencoding;

use crate::cea2034 as score;
use crate::read::directory::{data_dir_for, measurement_filename};
use crate::read::interpolate::interpolate;
use crate::read::plot::{normalize_plotly_json_from_str, normalize_plotly_value_with_suggestions};
use crate::{Curve, DirectivityCurve, DirectivityData};

/// CEA2034 spin data with all standard curves
#[derive(Debug, Clone)]
pub struct Cea2034Data {
    /// On Axis response
    pub on_axis: Curve,
    /// Listening Window response
    pub listening_window: Curve,
    /// Early Reflections response
    pub early_reflections: Curve,
    /// Sound Power response
    pub sound_power: Curve,
    /// Estimated In-Room Response (PIR)
    pub estimated_in_room: Curve,
    /// Early Reflections Directivity Index (On Axis - ER)
    pub er_di: Curve,
    /// Sound Power Directivity Index (On Axis - SP)
    pub sp_di: Curve,
    /// All curves as HashMap (for backward compatibility)
    pub curves: HashMap<String, Curve>,
}

/// Fetch a frequency response curve from the spinorama API
///
/// # Arguments
/// * `speaker` - Speaker name
/// * `version` - Measurement version
/// * `measurement` - Measurement type (e.g., "CEA2034")
/// * `curve_name` - Name of the specific curve to extract
///
/// # Returns
/// * Result containing a Curve struct or an error
pub async fn read_spinorama(
    speaker: &str,
    version: &str,
    measurement: &str,
    curve_name: &str,
) -> Result<crate::Curve, Box<dyn Error>> {
    fetch_curve_from_api(speaker, version, measurement, curve_name).await
}

/// Fetch a frequency response curve from the spinorama API
///
/// # Arguments
/// * `speaker` - Speaker name
/// * `version` - Measurement version
/// * `measurement` - Measurement type (e.g., "CEA2034")
/// * `curve_name` - Name of the specific curve to extract
///
/// # Returns
/// * Result containing a Curve struct or an error
pub async fn fetch_curve_from_api(
    speaker: &str,
    version: &str,
    measurement: &str,
    curve_name: &str,
) -> Result<crate::Curve, Box<dyn Error>> {
    // Fetch the full measurement once, then extract the requested curve
    let plot_data = fetch_measurement_plot_data(speaker, version, measurement).await?;
    extract_curve_by_name(&plot_data, measurement, curve_name)
}

/// Fetch and parse the full Plotly JSON object for a given measurement (single HTTP GET)
///
/// # Arguments
/// * `speaker` - Speaker name
/// * `version` - Measurement version
/// * `measurement` - Measurement type (e.g., "CEA2034")
///
/// # Returns
/// * Result containing the Plotly JSON data or an error
pub async fn fetch_measurement_plot_data(
    speaker: &str,
    version: &str,
    measurement: &str,
) -> Result<Value, Box<dyn Error>> {
    // 1) Try local cache first: data_cached/speakers/org.spinorama/{speaker}/{measurement}.json
    // We keep filename identical to measurement name when possible (with path separators replaced).
    let cache_dir = data_dir_for(speaker);
    let cache_file = cache_dir.join(measurement_filename(measurement));

    if let Ok(content) = fs::read_to_string(&cache_file).await {
        if let Ok(plot_data) = normalize_plotly_json_from_str(&content) {
            return Ok(plot_data);
        } else {
            eprintln!(
                "⚠️  Cache file exists but could not be parsed as Plotly JSON: {:?}",
                &cache_file
            );
        }
    }

    // URL-encode the parameters
    let encoded_speaker = urlencoding::encode(speaker);
    let encoded_version = urlencoding::encode(version);
    let encoded_measurement = urlencoding::encode(measurement);

    let url = format!(
        "https://api.spinorama.org/v1/speaker/{}/version/{}/measurements/{}?measurement_format=json",
        encoded_speaker, encoded_version, encoded_measurement
    );

    // println!("* Fetching data from {}", url);

    let response = reqwest::get(&url).await?;
    if !response.status().is_success() {
        return Err(format!("API request failed with status: {}", response.status()).into());
    }
    let api_response: Value = response.json().await?;

    // Normalize from API response (array-of-string JSON) to Plotly JSON object
    let plot_data = normalize_plotly_value_with_suggestions(&api_response).await?;

    // 2) Save normalized Plotly JSON to cache for future use
    if let Err(e) = fs::create_dir_all(&cache_dir).await {
        eprintln!("⚠️  Failed to create cache dir {:?}: {}", &cache_dir, e);
    } else {
        match serde_json::to_string(&plot_data) {
            Ok(serialized) => {
                if let Err(e) = fs::write(&cache_file, serialized).await {
                    eprintln!("⚠️  Failed to write cache file {:?}: {}", &cache_file, e);
                }
            }
            Err(e) => eprintln!("⚠️  Failed to serialize plot data for cache: {}", e),
        }
    }

    Ok(plot_data)
}

/// Extract a single curve from a previously-fetched Plotly JSON object
///
/// # Arguments
/// * `plot_data` - The Plotly JSON data
/// * `measurement` - Measurement type (e.g., "CEA2034")
/// * `curve_name` - Name of the specific curve to extract
///
/// # Returns
/// * Result containing a Curve struct or an error
pub fn extract_curve_by_name(
    plot_data: &Value,
    measurement: &str,
    curve_name: &str,
) -> Result<crate::Curve, Box<dyn Error>> {
    // Extract frequency and SPL data from the Plotly JSON structure
    let mut freqs = Vec::new();
    let mut spls = Vec::new();
    let mut phases = Vec::new();

    // Look for the trace with the expected name and extract x and y data
    if let Some(data) = plot_data.get("data").and_then(|d| d.as_array()) {
        // First pass: find SPL (magnitude)
        for trace in data {
            // Check if this is the SPL trace (not DI or other traces)
            let is_spl_trace = trace
                .get("name")
                .and_then(|n| n.as_str())
                .map(|name| is_target_trace_name(measurement, curve_name, name))
                .unwrap_or(false);

            if is_spl_trace {
                // Try to extract x and y data - supports both typed arrays and plain arrays
                let x_val = trace.get("x");
                let y_val = trace.get("y");

                if let (Some(x_data), Some(y_data)) = (x_val, y_val) {
                    // Try typed array format first (object with dtype/bdata)
                    if let (Some(x_obj), Some(y_obj)) = (x_data.as_object(), y_data.as_object()) {
                        // Decode x values (frequency)
                        if let (Some(dtype), Some(bdata)) = (
                            x_obj.get("dtype").and_then(|d| d.as_str()),
                            x_obj.get("bdata").and_then(|b| b.as_str()),
                        ) {
                            freqs = decode_typed_array(bdata, dtype)?;
                        }

                        // Decode y values (SPL)
                        if let (Some(dtype), Some(bdata)) = (
                            y_obj.get("dtype").and_then(|d| d.as_str()),
                            y_obj.get("bdata").and_then(|b| b.as_str()),
                        ) {
                            spls = decode_typed_array(bdata, dtype)?;
                        }
                    }

                    // Fallback: try plain array format
                    if freqs.is_empty()
                        && let (Some(x_arr), Some(y_arr)) = (x_data.as_array(), y_data.as_array())
                    {
                        freqs = x_arr.iter().filter_map(|v| v.as_f64()).collect();
                        spls = y_arr.iter().filter_map(|v| v.as_f64()).collect();
                    }
                }
                if !freqs.is_empty() {
                    break;
                }
            }
        }

        // Second pass: find phase (if available)
        let phase_name = format!("{} Phase", curve_name);
        for trace in data {
            let is_phase_trace = trace
                .get("name")
                .and_then(|n| n.as_str())
                .map(|name| name == phase_name)
                .unwrap_or(false);

            if is_phase_trace {
                let y_val = trace.get("y");
                if let Some(y_data) = y_val {
                    if let Some(y_obj) = y_data.as_object() {
                        if let (Some(dtype), Some(bdata)) = (
                            y_obj.get("dtype").and_then(|d| d.as_str()),
                            y_obj.get("bdata").and_then(|b| b.as_str()),
                        ) {
                            phases = decode_typed_array(bdata, dtype)?;
                        }
                    } else if let Some(y_arr) = y_data.as_array() {
                        phases = y_arr.iter().filter_map(|v| v.as_f64()).collect();
                    }
                }
                if !phases.is_empty() {
                    break;
                }
            }
        }
    }

    if freqs.is_empty() {
        let available = collect_trace_names(plot_data);
        return Err(format!(
            "Failed to extract frequency and SPL data for curve '{}' in measurement '{}'. Available traces: {:?}",
            curve_name, measurement, available
        ).into());
    }

    Ok(crate::Curve {
        freq: Array1::from(freqs),
        spl: Array1::from(spls),
        phase: if !phases.is_empty() {
            Some(Array1::from(phases))
        } else {
            None
        },
    })
}

fn decode_typed_array(bdata: &str, dtype: &str) -> Result<Vec<f64>, Box<dyn Error>> {
    // Create lookup table for base64 decoding
    let chars = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut lookup = [0u8; 256];
    for (i, c) in chars.chars().enumerate() {
        lookup[c as usize] = i as u8;
    }

    // Calculate buffer length
    let len = bdata.len();
    let mut buffer_length = len * 3 / 4;

    // Adjust for padding
    if len > 0 && bdata.chars().nth(len - 1) == Some('=') {
        buffer_length -= 1;
        if len > 1 && bdata.chars().nth(len - 2) == Some('=') {
            buffer_length -= 1;
        }
    }

    // Decode base64
    let mut bytes = vec![0u8; buffer_length];
    let mut p = 0;
    let bdata_bytes = bdata.as_bytes();

    for i in (0..len).step_by(4) {
        let encoded1 = lookup[bdata_bytes[i] as usize] as u32;
        let encoded2 = if i + 1 < len {
            lookup[bdata_bytes[i + 1] as usize] as u32
        } else {
            0
        };
        let encoded3 = if i + 2 < len {
            lookup[bdata_bytes[i + 2] as usize] as u32
        } else {
            0
        };
        let encoded4 = if i + 3 < len {
            lookup[bdata_bytes[i + 3] as usize] as u32
        } else {
            0
        };

        if p < buffer_length {
            bytes[p] = ((encoded1 << 2) | (encoded2 >> 4)) as u8;
            p += 1;
        }

        if p < buffer_length {
            bytes[p] = (((encoded2 & 15) << 4) | (encoded3 >> 2)) as u8;
            p += 1;
        }

        if p < buffer_length {
            bytes[p] = (((encoded3 & 3) << 6) | (encoded4 & 63)) as u8;
            p += 1;
        }
    }

    // Convert to appropriate typed array based on dtype
    let result = match dtype {
        "f8" => {
            // Float64Array - 8 bytes per element
            let mut values = Vec::new();
            for chunk in bytes.chunks_exact(8) {
                let bits = u64::from_le_bytes([
                    chunk[0], chunk[1], chunk[2], chunk[3], chunk[4], chunk[5], chunk[6], chunk[7],
                ]);
                values.push(f64::from_bits(bits));
            }
            values
        }
        "f4" => {
            // Float32Array - 4 bytes per element
            let mut values = Vec::new();
            for chunk in bytes.chunks_exact(4) {
                let bits = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                values.push(f32::from_bits(bits) as f64);
            }
            values
        }
        "i4" => {
            // Int32Array - 4 bytes per element
            let mut values = Vec::new();
            for chunk in bytes.chunks_exact(4) {
                let val = i32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                values.push(val as f64);
            }
            values
        }
        "i2" => {
            // Int16Array - 2 bytes per element
            let mut values = Vec::new();
            for chunk in bytes.chunks_exact(2) {
                let val = i16::from_le_bytes([chunk[0], chunk[1]]);
                values.push(val as f64);
            }
            values
        }
        "i1" => {
            // Int8Array - 1 byte per element
            bytes.into_iter().map(|b| b as i8 as f64).collect()
        }
        "u4" => {
            // Uint32Array - 4 bytes per element
            let mut values = Vec::new();
            for chunk in bytes.chunks_exact(4) {
                let val = u32::from_le_bytes([chunk[0], chunk[1], chunk[2], chunk[3]]);
                values.push(val as f64);
            }
            values
        }
        "u2" => {
            // Uint16Array - 2 bytes per element
            let mut values = Vec::new();
            for chunk in bytes.chunks_exact(2) {
                let val = u16::from_le_bytes([chunk[0], chunk[1]]);
                values.push(val as f64);
            }
            values
        }
        "u1" | "u1c" => {
            // Uint8Array or Uint8ClampedArray - 1 byte per element
            bytes.into_iter().map(|b| b as f64).collect()
        }
        _ => {
            // Default to treating as bytes
            bytes.into_iter().map(|b| b as f64).collect()
        }
    };

    Ok(result)
}

fn is_target_trace_name(measurement: &str, curve_name: &str, candidate: &str) -> bool {
    if measurement.eq_ignore_ascii_case("CEA2034") || measurement.eq("Estimated In-Room Response") {
        // For CEA2034 data, select the specific curve provided by the user
        // Prefer exact match; allow substring match as a fallback
        candidate == curve_name
    } else {
        // Fallback heuristic for other measurement types
        eprintln!(
            "⚠️  Warning: unable to determine if trace name {} is a target for curve {}, using heuristic",
            candidate, curve_name
        );
        candidate.contains("Listening Window")
            || candidate.contains("On Axis")
            || candidate.contains("Sound Power")
            || candidate.contains("Early Reflections")
            || candidate.contains("Estimated In-Room Response")
    }
}

fn collect_trace_names(plot_data: &Value) -> Vec<String> {
    plot_data
        .get("data")
        .and_then(|d| d.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|trace| {
                    trace
                        .get("name")
                        .and_then(|n| n.as_str())
                        .map(|s| s.to_string())
                })
                .collect::<Vec<String>>()
        })
        .unwrap_or_default()
}

/// Extract all CEA2034 curves from plot data using the original frequency grid
///
/// # Arguments
/// * `plot_data` - The Plotly JSON data containing CEA2034 measurements
/// * `measurement` - Measurement type (e.g., "CEA2034")
///
/// # Returns
/// * HashMap of curve names to Curve structs with original frequency grid
///
/// # Details
/// Extracts standard CEA2034 curves (On Axis, Listening Window, Early Reflections,
/// Sound Power, etc.) using the original frequency grid from the measurement data.
/// This is preferred for score calculations to match the Python implementation.
pub fn extract_cea2034_curves_original(
    plot_data: &Value,
    measurement: &str,
) -> Result<HashMap<String, Curve>, Box<dyn Error>> {
    let mut curves = HashMap::new();

    // List of CEA2034 curves to extract
    let curve_names = [
        "On Axis",
        "Listening Window",
        "Early Reflections",
        "Sound Power",
        "Early Reflections DI",
        "Sound Power DI",
    ];

    // Extract each curve with its original frequency grid
    for name in &curve_names {
        match extract_curve_by_name(plot_data, measurement, name) {
            Ok(curve) => {
                curves.insert(name.to_string(), curve);
            }
            Err(e) => {
                let available = collect_trace_names(plot_data);
                return Err(format!(
                    "Could not extract curve '{}' for measurement '{}': {}. Available traces: {:?}",
                    name, measurement, e, available
                )
                .into());
            }
        }
    }

    // Ensure required curves exist for PIR computation
    let lw_curve = curves.get("Listening Window").ok_or_else(|| {
        std::io::Error::other("Missing 'Listening Window' curve after extraction")
    })?;
    let er_curve = curves.get("Early Reflections").ok_or_else(|| {
        std::io::Error::other("Missing 'Early Reflections' curve after extraction")
    })?;
    let sp_curve = curves
        .get("Sound Power")
        .ok_or_else(|| std::io::Error::other("Missing 'Sound Power' curve after extraction"))?;

    let freq = &lw_curve.freq;
    let lw = &lw_curve.spl;
    let er = &er_curve.spl;
    let sp = &sp_curve.spl;
    let pir = score::compute_pir_from_lw_er_sp(lw, er, sp);

    curves.insert(
        "Estimated In-Room Response".to_string(),
        Curve {
            freq: freq.clone(),
            spl: pir,
            phase: None,
        },
    );

    Ok(curves)
}

/// Extract all CEA2034 curves from plot data and interpolate to target frequency grid
///
/// # Arguments
/// * `plot_data` - The Plotly JSON data containing CEA2034 measurements
/// * `measurement` - Measurement type (e.g., "CEA2034")
/// * `freq` - Target frequency grid for interpolation
///
/// # Returns
/// * HashMap of curve names to interpolated Curve structs
///
/// # Details
/// Extracts standard CEA2034 curves (On Axis, Listening Window, Early Reflections,
/// Sound Power, etc.) and interpolates them to the specified frequency grid.
pub fn extract_cea2034_curves(
    plot_data: &Value,
    measurement: &str,
    freq: &Array1<f64>,
) -> Result<HashMap<String, Curve>, Box<dyn Error>> {
    let mut curves = HashMap::new();

    // List of CEA2034 curves to extract
    let curve_names = [
        "On Axis",
        "Listening Window",
        "Early Reflections",
        "Sound Power",
        "Early Reflections DI",
        "Sound Power DI",
    ];

    // Extract each curve
    for name in &curve_names {
        match extract_curve_by_name(plot_data, measurement, name) {
            Ok(curve) => {
                // Interpolate to the target frequency grid
                let interpolated = interpolate(freq, &curve);
                curves.insert(
                    name.to_string(),
                    Curve {
                        freq: freq.clone(),
                        spl: interpolated.spl,
                        phase: None,
                    },
                );
            }
            Err(e) => {
                let available = collect_trace_names(plot_data);
                return Err(format!(
                    "Could not extract curve '{}' for measurement '{}': {}. Available traces: {:?}",
                    name, measurement, e, available
                )
                .into());
            }
        }
    }

    // Ensure required curves exist for PIR computation
    let lw_curve = curves.get("Listening Window").ok_or_else(|| {
        std::io::Error::other("Missing 'Listening Window' curve after extraction")
    })?;
    let er_curve = curves.get("Early Reflections").ok_or_else(|| {
        std::io::Error::other("Missing 'Early Reflections' curve after extraction")
    })?;
    let sp_curve = curves
        .get("Sound Power")
        .ok_or_else(|| std::io::Error::other("Missing 'Sound Power' curve after extraction"))?;

    let lw = &lw_curve.spl;
    let er = &er_curve.spl;
    let sp = &sp_curve.spl;
    let pir = score::compute_pir_from_lw_er_sp(lw, er, sp);
    curves.insert(
        "Estimated In-Room Response".to_string(),
        Curve {
            freq: freq.clone(),
            spl: pir,
            phase: None,
        },
    );

    Ok(curves)
}

/// Parse an angle string from trace names (e.g., "-60°", "0° (ON)", "10°")
///
/// # Arguments
/// * `name` - The trace name containing an angle
///
/// # Returns
/// * Some(angle) if the angle could be parsed, None otherwise
fn parse_angle_from_trace_name(name: &str) -> Option<f64> {
    // Handle special case "0° (ON)"
    let name = name.replace("(ON)", "").trim().to_string();

    // Remove degree symbol and parse
    let angle_str = name.replace('°', "").trim().to_string();

    angle_str.parse::<f64>().ok()
}

/// Extract all directivity curves from a measurement plot
///
/// # Arguments
/// * `plot_data` - The Plotly JSON data containing directivity measurements
///
/// # Returns
/// * Vector of DirectivityCurve structs, sorted by angle
fn extract_directivity_curves(plot_data: &Value) -> Result<Vec<DirectivityCurve>, Box<dyn Error>> {
    let mut curves = Vec::new();

    if let Some(data) = plot_data.get("data").and_then(|d| d.as_array()) {
        for trace in data {
            // Get the trace name
            let name = match trace.get("name").and_then(|n| n.as_str()) {
                Some(n) => n,
                None => continue,
            };

            // Try to parse the angle from the name
            let angle = match parse_angle_from_trace_name(name) {
                Some(a) => a,
                None => continue, // Skip traces without valid angles
            };

            // Extract x and y data which are encoded as typed arrays
            if let (Some(x_data), Some(y_data)) = (
                trace.get("x").and_then(|x| x.as_object()),
                trace.get("y").and_then(|y| y.as_object()),
            ) {
                let mut freqs = Vec::new();
                let mut spls = Vec::new();

                // Decode x values (frequency)
                if let (Some(dtype), Some(bdata)) = (
                    x_data.get("dtype").and_then(|d| d.as_str()),
                    x_data.get("bdata").and_then(|b| b.as_str()),
                ) {
                    freqs = decode_typed_array(bdata, dtype)?;
                }

                // Decode y values (SPL)
                if let (Some(dtype), Some(bdata)) = (
                    y_data.get("dtype").and_then(|d| d.as_str()),
                    y_data.get("bdata").and_then(|b| b.as_str()),
                ) {
                    spls = decode_typed_array(bdata, dtype)?;
                }

                if !freqs.is_empty() && freqs.len() == spls.len() {
                    curves.push(DirectivityCurve {
                        angle,
                        freq: Array1::from(freqs),
                        spl: Array1::from(spls),
                    });
                }
            }
        }
    }

    // Sort by angle
    curves.sort_by(|a, b| {
        a.angle
            .partial_cmp(&b.angle)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    Ok(curves)
}

/// Fetch directivity data (SPL Horizontal and SPL Vertical) from the spinorama API
///
/// # Arguments
/// * `speaker` - Speaker name
/// * `version` - Measurement version
///
/// # Returns
/// * Result containing DirectivityData or an error
///
/// # Details
/// Fetches both "SPL Horizontal" and "SPL Vertical" measurements, extracting
/// all angle traces (typically -60° to +60° in 10° increments).
pub async fn fetch_directivity_data(
    speaker: &str,
    version: &str,
) -> Result<DirectivityData, Box<dyn Error>> {
    // Fetch horizontal data
    let horizontal_data = fetch_measurement_plot_data(speaker, version, "SPL Horizontal").await?;
    let horizontal = extract_directivity_curves(&horizontal_data)?;

    if horizontal.is_empty() {
        return Err("No horizontal directivity curves found".into());
    }

    // Fetch vertical data
    let vertical_data = fetch_measurement_plot_data(speaker, version, "SPL Vertical").await?;
    let vertical = extract_directivity_curves(&vertical_data)?;

    if vertical.is_empty() {
        return Err("No vertical directivity curves found".into());
    }

    Ok(DirectivityData {
        horizontal,
        vertical,
    })
}

/// Extract a single directivity curve at a specific angle
///
/// # Arguments
/// * `directivity` - The full directivity data
/// * `angle` - The desired angle in degrees
/// * `plane` - "horizontal" or "vertical"
///
/// # Returns
/// * Option containing the DirectivityCurve if found
pub fn get_directivity_at_angle<'a>(
    directivity: &'a DirectivityData,
    angle: f64,
    plane: &str,
) -> Option<&'a DirectivityCurve> {
    let curves = match plane.to_lowercase().as_str() {
        "horizontal" | "h" => &directivity.horizontal,
        "vertical" | "v" => &directivity.vertical,
        _ => return None,
    };

    curves.iter().find(|c| (c.angle - angle).abs() < 0.5)
}

/// Get the on-axis response from directivity data
///
/// # Arguments
/// * `directivity` - The full directivity data
/// * `plane` - "horizontal" or "vertical"
///
/// # Returns
/// * Option containing the on-axis DirectivityCurve
pub fn get_on_axis<'a>(
    directivity: &'a DirectivityData,
    plane: &str,
) -> Option<&'a DirectivityCurve> {
    get_directivity_at_angle(directivity, 0.0, plane)
}

/// Get available angles in the directivity data
///
/// # Arguments
/// * `directivity` - The full directivity data
/// * `plane` - "horizontal" or "vertical"
///
/// # Returns
/// * Vector of available angles
pub fn get_available_angles(directivity: &DirectivityData, plane: &str) -> Vec<f64> {
    let curves = match plane.to_lowercase().as_str() {
        "horizontal" | "h" => &directivity.horizontal,
        "vertical" | "v" => &directivity.vertical,
        _ => return Vec::new(),
    };

    curves.iter().map(|c| c.angle).collect()
}

/// Contour plot data with 2D heatmap structure
///
/// Used for "SPL Horizontal Contour" and "SPL Vertical Contour" measurements
/// which provide full angle range data (-180° to +180°).
#[derive(Debug, Clone)]
pub struct ContourPlotData {
    /// Frequency values (X axis)
    pub freq: Vec<f64>,
    /// Angle values in degrees (Y axis)
    pub angles: Vec<f64>,
    /// SPL values as a 2D grid (row-major: angles × freq)
    pub spl: Vec<f64>,
    /// Number of frequency points
    pub freq_count: usize,
    /// Number of angle points
    pub angle_count: usize,
}

/// Fetch contour plot data (SPL Horizontal Contour or SPL Vertical Contour) from the spinorama API
///
/// # Arguments
/// * `speaker` - Speaker name
/// * `version` - Measurement version
/// * `plane` - Either "horizontal" or "vertical"
///
/// # Returns
/// * Result containing ContourPlotData or an error
///
/// # Details
/// Fetches "SPL Horizontal Contour" or "SPL Vertical Contour" measurements, which are
/// pre-computed heatmaps with full angle range (typically -180° to +180°).
pub async fn fetch_contour_data(
    speaker: &str,
    version: &str,
    plane: &str,
) -> Result<ContourPlotData, Box<dyn Error>> {
    let measurement = match plane.to_lowercase().as_str() {
        "horizontal" | "h" => "SPL Horizontal Contour",
        "vertical" | "v" => "SPL Vertical Contour",
        _ => {
            return Err(format!("Invalid plane: {}. Use 'horizontal' or 'vertical'", plane).into());
        }
    };

    let plot_data = fetch_measurement_plot_data(speaker, version, measurement).await?;
    extract_contour_data(&plot_data)
}

/// Load spinorama measurement with full CEA2034 spin data
///
/// This fetches the requested curve and also extracts all CEA2034 curves
/// when the measurement type is CEA2034.
///
/// # Arguments
/// * `speaker` - Speaker name (e.g., "KEF R3")
/// * `version` - Version (e.g., "asr")
/// * `measurement` - Measurement type (e.g., "CEA2034")
/// * `curve_name` - Specific curve to use as primary (e.g., "Listening Window")
///
/// # Returns
/// Tuple of (primary curve, optional spin data)
pub async fn load_spinorama_with_spin(
    speaker: &str,
    version: &str,
    measurement: &str,
    curve_name: &str,
) -> Result<(Curve, Option<Cea2034Data>), Box<dyn Error>> {
    // Handle Estimated In-Room Response specially
    if measurement == "Estimated In-Room Response"
        || (measurement == "CEA2034" && curve_name == "Estimated In-Room Response")
    {
        let plot_data = fetch_measurement_plot_data(speaker, version, "CEA2034").await?;
        let curves = extract_cea2034_curves_original(&plot_data, "CEA2034")?;

        let pir_curve = curves
            .get("Estimated In-Room Response")
            .ok_or_else(|| {
                Box::<dyn Error>::from("Estimated In-Room Response curve not found in CEA2034 data")
            })?
            .clone();

        let spin_data = build_cea2034_data(curves)?;
        return Ok((pir_curve, Some(spin_data)));
    }

    // Standard curve fetch
    let curve = read_spinorama(speaker, version, measurement, curve_name).await?;

    // Extract spin data if CEA2034
    let spin_data = if measurement == "CEA2034" {
        let plot_data = fetch_measurement_plot_data(speaker, version, measurement).await?;
        let curves = extract_cea2034_curves_original(&plot_data, "CEA2034")?;
        Some(build_cea2034_data(curves)?)
    } else {
        None
    };

    Ok((curve, spin_data))
}

/// Build Cea2034Data from curves HashMap
fn build_cea2034_data(curves: HashMap<String, Curve>) -> Result<Cea2034Data, Box<dyn Error>> {
    let get_curve = |name: &str| -> Result<Curve, Box<dyn Error>> {
        curves
            .get(name)
            .cloned()
            .ok_or_else(|| Box::<dyn Error>::from(format!("Missing CEA2034 curve: {}", name)))
    };

    let on_axis = get_curve("On Axis")?;
    let listening_window = get_curve("Listening Window")?;
    let early_reflections = get_curve("Early Reflections")?;
    let sound_power = get_curve("Sound Power")?;
    let estimated_in_room = get_curve("Estimated In-Room Response")?;

    // Compute directivity indices
    let er_di = Curve {
        freq: on_axis.freq.clone(),
        spl: &on_axis.spl - &early_reflections.spl,
        phase: None,
    };
    let sp_di = Curve {
        freq: on_axis.freq.clone(),
        spl: &on_axis.spl - &sound_power.spl,
        phase: None,
    };

    Ok(Cea2034Data {
        on_axis,
        listening_window,
        early_reflections,
        sound_power,
        estimated_in_room,
        er_di,
        sp_di,
        curves,
    })
}

/// Extract contour data from Plotly JSON (heatmap format)
fn extract_contour_data(plot_data: &Value) -> Result<ContourPlotData, Box<dyn Error>> {
    // Look for the first trace with a 'z' array (heatmap data)
    if let Some(data) = plot_data.get("data").and_then(|d| d.as_array()) {
        for trace in data {
            // Look for z data (2D grid), x (frequencies), and y (angles)
            if let (Some(x_data), Some(y_data), Some(z_data)) =
                (trace.get("x"), trace.get("y"), trace.get("z"))
            {
                let mut freq = Vec::new();
                let mut angles = Vec::new();
                let mut spl = Vec::new();

                // Decode x values (frequency) - could be typed array or regular array
                if let Some(x_obj) = x_data.as_object() {
                    if let (Some(dtype), Some(bdata)) = (
                        x_obj.get("dtype").and_then(|d| d.as_str()),
                        x_obj.get("bdata").and_then(|b| b.as_str()),
                    ) {
                        freq = decode_typed_array(bdata, dtype)?;
                    }
                } else if let Some(x_arr) = x_data.as_array() {
                    freq = x_arr.iter().filter_map(|v| v.as_f64()).collect();
                }

                // Decode y values (angles) - could be typed array or regular array
                if let Some(y_obj) = y_data.as_object() {
                    if let (Some(dtype), Some(bdata)) = (
                        y_obj.get("dtype").and_then(|d| d.as_str()),
                        y_obj.get("bdata").and_then(|b| b.as_str()),
                    ) {
                        angles = decode_typed_array(bdata, dtype)?;
                    }
                } else if let Some(y_arr) = y_data.as_array() {
                    angles = y_arr.iter().filter_map(|v| v.as_f64()).collect();
                }

                // Decode z values (SPL grid) - could be typed 2D array or nested regular arrays
                if let Some(z_obj) = z_data.as_object() {
                    // Typed array format with bdata
                    if let (Some(dtype), Some(bdata)) = (
                        z_obj.get("dtype").and_then(|d| d.as_str()),
                        z_obj.get("bdata").and_then(|b| b.as_str()),
                    ) {
                        spl = decode_typed_array(bdata, dtype)?;
                    }
                } else if let Some(z_arr) = z_data.as_array() {
                    // Nested array format [[row0], [row1], ...]
                    for row in z_arr {
                        if let Some(row_obj) = row.as_object() {
                            // Typed array row
                            if let (Some(dtype), Some(bdata)) = (
                                row_obj.get("dtype").and_then(|d| d.as_str()),
                                row_obj.get("bdata").and_then(|b| b.as_str()),
                            ) {
                                let row_data = decode_typed_array(bdata, dtype)?;
                                spl.extend(row_data);
                            }
                        } else if let Some(row_arr) = row.as_array() {
                            let row_data: Vec<f64> =
                                row_arr.iter().filter_map(|v| v.as_f64()).collect();
                            spl.extend(row_data);
                        }
                    }
                }

                if !freq.is_empty() && !angles.is_empty() && !spl.is_empty() {
                    let freq_count = freq.len();
                    let angle_count = angles.len();

                    // Verify data dimensions
                    let expected_size = freq_count * angle_count;
                    if spl.len() != expected_size {
                        eprintln!(
                            "Warning: SPL grid size {} doesn't match expected {} ({}×{})",
                            spl.len(),
                            expected_size,
                            angle_count,
                            freq_count
                        );
                    }

                    return Ok(ContourPlotData {
                        freq,
                        angles,
                        spl,
                        freq_count,
                        angle_count,
                    });
                }
            }
        }
    }

    Err("Failed to extract contour data from plot data".into())
}
