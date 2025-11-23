// ============================================================================
// Audio Capture Commands (recording with test signals)
// ============================================================================

use serde::{Deserialize, Serialize};
use sotf_audio::signal_recorder::{SignalParams, SignalType, generate_signal};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::ZipArchive;
use zip::ZipWriter;
use zip::write::FileOptions;

/// Recording result with frequency response data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RecordingResult {
    pub channel: usize,
    pub wav_path: String,
    pub csv_path: String,
    pub frequencies: Vec<f64>,
    pub magnitude_db: Vec<f64>,
    pub phase_deg: Vec<f64>,
}

/// Record a single channel with a test signal
///
/// This command:
/// 1. Generates a test signal (sweep, white noise, pink noise)
/// 2. Plays it on the specified output channel
/// 3. Records from the specified input channel
/// 4. Analyzes the frequency response
/// 5. Returns the analysis results
#[tauri::command]
pub async fn record_channel(
    output_device: String,
    input_device: String,
    output_channel: usize,
    input_channel: usize,
    signal_type: String,
    duration: f32,
    sample_rate: u32,
    output_path: String,
) -> Result<RecordingResult, String> {
    println!(
        "[TAURI] Recording channel: out={} (ch {}), in={} (ch {}), signal={}, duration={}s",
        output_device, output_channel, input_device, input_channel, signal_type, duration
    );

    // Parse signal type
    let sig_type = signal_type
        .parse::<SignalType>()
        .map_err(|e| format!("Invalid signal type: {}", e))?;

    // Generate signal parameters
    let params = match sig_type {
        SignalType::Sweep => SignalParams::Sweep {
            start_freq: 20.0,
            end_freq: 20000.0,
            amp: 0.8,
        },
        SignalType::WhiteNoise | SignalType::PinkNoise => SignalParams::Noise { amp: 0.5 },
        _ => return Err("Unsupported signal type".to_string()),
    };

    // Generate the reference signal
    let reference_signal = generate_signal(sig_type, &params, duration, sample_rate)
        .map_err(|e| format!("Failed to generate signal: {}", e))?;

    // Create temporary WAV file for the signal
    let temp_signal_path = format!("{}_signal.wav", output_path);
    let temp_signal = PathBuf::from(&temp_signal_path);

    // Write signal to WAV file
    write_wav(&temp_signal, &reference_signal, sample_rate)
        .map_err(|e| format!("Failed to write signal WAV: {}", e))?;

    // Output paths
    let recorded_wav_path = format!("{}_recorded.wav", output_path);
    let recorded_wav = PathBuf::from(&recorded_wav_path);
    let csv_path = format!("{}_analysis.csv", output_path);
    let csv_file = PathBuf::from(&csv_path);

    // Record and analyze
    // Note: This uses the existing signal_recorder::record_and_analyze function
    // which handles playback and recording via AudioEngineManager and cpal
    // The function currently only supports one device (combined output/input)
    sotf_audio::signal_recorder::record_and_analyze(
        &temp_signal,
        &recorded_wav,
        &reference_signal,
        sample_rate,
        &csv_file,
        output_channel as u16,
        input_channel as u16,
        Some(&output_device),
        None, // microphone_compensation_path
    )
    .map_err(|e| format!("Recording failed: {}", e))?;

    // Read the CSV analysis results
    let (frequencies, magnitude_db, phase_deg) =
        read_analysis_csv(&csv_file).map_err(|e| format!("Failed to read analysis: {}", e))?;

    // Clean up temporary signal file
    let _ = fs::remove_file(&temp_signal);

    Ok(RecordingResult {
        channel: output_channel,
        wav_path: recorded_wav_path,
        csv_path,
        frequencies,
        magnitude_db,
        phase_deg,
    })
}

/// Save recordings to a ZIP file
#[tauri::command]
pub async fn save_recordings_zip(
    recordings: Vec<RecordingResult>,
    output_path: String,
) -> Result<String, String> {
    println!(
        "[TAURI] Saving {} recordings to {}",
        recordings.len(),
        output_path
    );

    let file =
        fs::File::create(&output_path).map_err(|e| format!("Failed to create ZIP file: {}", e))?;

    let mut zip = ZipWriter::new(file);
    let options: FileOptions<()> = FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o755);

    // Add each recording's WAV and CSV files to the ZIP
    for recording in &recordings {
        // Add WAV file
        if Path::new(&recording.wav_path).exists() {
            let wav_name = format!("channel_{}_recorded.wav", recording.channel);
            zip.start_file(wav_name, options)
                .map_err(|e| format!("Failed to start WAV file in ZIP: {}", e))?;

            let mut wav_file = fs::File::open(&recording.wav_path)
                .map_err(|e| format!("Failed to open WAV file: {}", e))?;
            let mut buffer = Vec::new();
            wav_file
                .read_to_end(&mut buffer)
                .map_err(|e| format!("Failed to read WAV file: {}", e))?;
            zip.write_all(&buffer)
                .map_err(|e| format!("Failed to write WAV to ZIP: {}", e))?;
        }

        // Add CSV file
        if Path::new(&recording.csv_path).exists() {
            let csv_name = format!("channel_{}_analysis.csv", recording.channel);
            zip.start_file(csv_name, options)
                .map_err(|e| format!("Failed to start CSV file in ZIP: {}", e))?;

            let mut csv_file = fs::File::open(&recording.csv_path)
                .map_err(|e| format!("Failed to open CSV file: {}", e))?;
            let mut buffer = Vec::new();
            csv_file
                .read_to_end(&mut buffer)
                .map_err(|e| format!("Failed to read CSV file: {}", e))?;
            zip.write_all(&buffer)
                .map_err(|e| format!("Failed to write CSV to ZIP: {}", e))?;
        }
    }

    // Add metadata JSON
    let metadata = serde_json::to_string_pretty(&recordings)
        .map_err(|e| format!("Failed to serialize metadata: {}", e))?;
    zip.start_file("metadata.json", options)
        .map_err(|e| format!("Failed to start metadata file: {}", e))?;
    zip.write_all(metadata.as_bytes())
        .map_err(|e| format!("Failed to write metadata: {}", e))?;

    zip.finish()
        .map_err(|e| format!("Failed to finalize ZIP: {}", e))?;

    Ok(output_path)
}

/// Load recordings from a ZIP file
#[tauri::command]
pub async fn load_recordings_zip(zip_path: String) -> Result<Vec<RecordingResult>, String> {
    println!("[TAURI] Loading recordings from {}", zip_path);

    let file = fs::File::open(&zip_path).map_err(|e| format!("Failed to open ZIP file: {}", e))?;

    let mut archive =
        ZipArchive::new(file).map_err(|e| format!("Failed to read ZIP archive: {}", e))?;

    // Read metadata to get recording info
    let mut metadata_file = archive
        .by_name("metadata.json")
        .map_err(|e| format!("Failed to find metadata.json: {}", e))?;
    let mut metadata_content = String::new();
    metadata_file
        .read_to_string(&mut metadata_content)
        .map_err(|e| format!("Failed to read metadata: {}", e))?;
    drop(metadata_file);

    let mut recordings: Vec<RecordingResult> = serde_json::from_str(&metadata_content)
        .map_err(|e| format!("Failed to parse metadata: {}", e))?;

    // Extract files to temporary directory
    let temp_dir = std::env::temp_dir().join(format!(
        "autoeq_recordings_{}",
        chrono::Utc::now().timestamp()
    ));
    fs::create_dir_all(&temp_dir).map_err(|e| format!("Failed to create temp directory: {}", e))?;

    // Extract all files
    for i in 0..archive.len() {
        let mut file = archive
            .by_index(i)
            .map_err(|e| format!("Failed to access file in ZIP: {}", e))?;

        let outpath = temp_dir.join(file.name());

        if file.is_dir() {
            fs::create_dir_all(&outpath)
                .map_err(|e| format!("Failed to create directory: {}", e))?;
        } else {
            if let Some(p) = outpath.parent() {
                if !p.exists() {
                    fs::create_dir_all(p)
                        .map_err(|e| format!("Failed to create parent directory: {}", e))?;
                }
            }
            let mut outfile =
                fs::File::create(&outpath).map_err(|e| format!("Failed to create file: {}", e))?;
            std::io::copy(&mut file, &mut outfile)
                .map_err(|e| format!("Failed to extract file: {}", e))?;
        }
    }

    // Update paths in recordings to point to extracted files
    for recording in &mut recordings {
        recording.wav_path = temp_dir
            .join(format!("channel_{}_recorded.wav", recording.channel))
            .to_string_lossy()
            .to_string();
        recording.csv_path = temp_dir
            .join(format!("channel_{}_analysis.csv", recording.channel))
            .to_string_lossy()
            .to_string();
    }

    Ok(recordings)
}

/// Helper function to write WAV file
fn write_wav(path: &Path, samples: &[f32], sample_rate: u32) -> Result<(), String> {
    use hound::{SampleFormat, WavSpec, WavWriter};

    let spec = WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };

    let mut writer =
        WavWriter::create(path, spec).map_err(|e| format!("Failed to create WAV writer: {}", e))?;

    for &sample in samples {
        writer
            .write_sample(sample)
            .map_err(|e| format!("Failed to write sample: {}", e))?;
    }

    writer
        .finalize()
        .map_err(|e| format!("Failed to finalize WAV: {}", e))?;

    Ok(())
}

/// Helper function to read analysis CSV
fn read_analysis_csv(path: &Path) -> Result<(Vec<f64>, Vec<f64>, Vec<f64>), String> {
    use csv::ReaderBuilder;

    let mut reader = ReaderBuilder::new()
        .has_headers(true)
        .from_path(path)
        .map_err(|e| format!("Failed to open CSV: {}", e))?;

    let mut frequencies = Vec::new();
    let mut magnitude_db = Vec::new();
    let mut phase_deg = Vec::new();

    for result in reader.records() {
        let record = result.map_err(|e| format!("Failed to read CSV record: {}", e))?;

        if record.len() >= 3 {
            frequencies.push(
                record[0]
                    .parse::<f64>()
                    .map_err(|e| format!("Failed to parse frequency: {}", e))?,
            );
            magnitude_db.push(
                record[1]
                    .parse::<f64>()
                    .map_err(|e| format!("Failed to parse magnitude: {}", e))?,
            );
            phase_deg.push(
                record[2]
                    .parse::<f64>()
                    .map_err(|e| format!("Failed to parse phase: {}", e))?,
            );
        }
    }

    Ok((frequencies, magnitude_db, phase_deg))
}
