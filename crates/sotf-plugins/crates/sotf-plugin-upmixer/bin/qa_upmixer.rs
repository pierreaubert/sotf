use hound::{SampleFormat, WavReader, WavSpec, WavWriter};
use sotf_host::{CountingAlloc, Plugin, ProcessContext};
use sotf_host::{ParameterValue, run_standard_tests};
use sotf_plugin_upmixer::params::SPEAKER_CONFIGS;
use sotf_plugin_upmixer::{UpmixerDiagnostics, UpmixerPlugin, UpmixerPluginParams};
use std::env;
use std::f32::consts::PI;
use std::fs::{self, File};
use std::io::{BufWriter, Cursor, Write};
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::time::{SystemTime, UNIX_EPOCH};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("diagnose") => {
            if let Err(err) = run_diagnostic(args.collect()) {
                eprintln!("diagnose failed: {err}");
                process::exit(2);
            }
        }
        Some("isolate") => {
            if let Err(err) = run_isolation(args.collect()) {
                eprintln!("isolate failed: {err}");
                process::exit(2);
            }
        }
        Some("--help") | Some("-h") => print_usage(),
        Some(other) => {
            eprintln!("unknown command: {other}");
            print_usage();
            process::exit(2);
        }
        None => run_self_qa(),
    }
}

fn print_usage() {
    println!(
        "Usage:\n  qa-upmixer\n  qa-upmixer diagnose <input.wav|audio-file> [output.csv] [--config 5.1] [--block-size 1024] [--fft-size 2048] [--frequency-resolution erb|fine_erb|per_bin] [--no-hr] [--bypass-decorrelation] [--bypass-transients] [--ml-model model.onnx]\n  qa-upmixer isolate <input.wav|audio-file> [output-dir] [--config 5.1] [--configs 5.1,7.1] [--all-configs] [--block-size 1024] [--fft-size 2048] [--seconds 10] [--frequency-resolution erb|fine_erb|per_bin] [--write-wavs] [--ml-model model.onnx]"
    );
}

#[derive(Debug)]
struct DiagnosticOptions {
    input_path: PathBuf,
    output_path: PathBuf,
    speaker_config: String,
    block_size: usize,
    fft_size: usize,
    frequency_resolution: String,
    enable_hr_direct: bool,
    bypass_decorrelation: bool,
    bypass_transient_detection: bool,
    ml_model_path: Option<String>,
}

#[derive(Debug)]
struct IsolationOptions {
    input_path: PathBuf,
    output_dir: PathBuf,
    speaker_configs: Vec<String>,
    block_size: usize,
    fft_size: usize,
    seconds: f32,
    frequency_resolutions: Vec<String>,
    write_wavs: bool,
    ml_model_path: Option<String>,
}

#[derive(Debug, Clone)]
struct IsolationVariant {
    name: String,
    config: String,
    frequency_resolution: String,
    notes: String,
    params: UpmixerPluginParams,
}

#[derive(Debug, Clone)]
struct IsolationRunResult {
    variant: IsolationVariant,
    output_channels: usize,
    frames_produced: usize,
    block_csv_path: PathBuf,
    wav_path: Option<PathBuf>,
    artifacts: ArtifactMetrics,
    max_deltas: DiagnosticMaxDeltas,
}

fn run_diagnostic(args: Vec<String>) -> Result<(), String> {
    let opts = parse_diagnostic_options(args)?;
    let input = load_audio_stereo(&opts.input_path)?;

    let mut params = UpmixerPluginParams {
        fft_size: opts.fft_size,
        speaker_config: opts.speaker_config.clone(),
        frequency_resolution: opts.frequency_resolution.clone(),
        enable_hr_direct: opts.enable_hr_direct,
        bypass_decorrelation: opts.bypass_decorrelation,
        bypass_transient_detection: opts.bypass_transient_detection,
        ..Default::default()
    };
    if let Some(model_path) = opts.ml_model_path.clone() {
        params.enable_ml_detection = true;
        params.ml_model_path = model_path;
    }

    let mut plugin = UpmixerPlugin::from_params(params);
    plugin.initialize(input.sample_rate)?;
    let out_channels = plugin.output_channels();

    let file = File::create(&opts.output_path)
        .map_err(|e| format!("could not create {}: {e}", opts.output_path.display()))?;
    let mut writer = BufWriter::new(file);
    write_header(&mut writer, out_channels)?;

    let total_frames = input.samples.len() / 2;
    let mut pos = 0usize;
    let mut block_index = 0usize;
    let mut prev_diag: Option<UpmixerDiagnostics> = None;
    let mut prev_output_last = vec![0.0_f32; out_channels];

    let mut max_dialogue_delta = 0.0_f32;
    let mut max_dialogue_spatial_delta = 0.0_f32;
    let mut max_height_mean_delta = 0.0_f32;
    let mut max_decorrelation_delta = 0.0_f32;
    let mut max_output_peak = 0.0_f32;
    let mut max_output_step = 0.0_f32;

    while pos < total_frames {
        let frames = opts.block_size.min(total_frames - pos);
        let input_slice = &input.samples[pos * 2..(pos + frames) * 2];
        let mut output = vec![0.0_f32; frames * out_channels];
        let context = ProcessContext::new(input.sample_rate, frames);
        let produced = plugin.process(input_slice, &mut output, &context)?;
        let diag = plugin.diagnostics();

        let input_metrics = input_metrics(input_slice);
        let output_metrics =
            channel_metrics(&output, out_channels, produced, &mut prev_output_last);
        let deltas = DiagnosticDeltas::from_previous(&diag, prev_diag.as_ref());

        max_dialogue_delta = max_dialogue_delta.max(deltas.dialogue_probability_abs);
        max_dialogue_spatial_delta =
            max_dialogue_spatial_delta.max(deltas.dialogue_spatial_control_abs);
        max_height_mean_delta = max_height_mean_delta.max(deltas.height_gain_mean_abs);
        max_decorrelation_delta = max_decorrelation_delta.max(deltas.decorrelation_abs);
        max_output_peak = max_output_peak.max(output_metrics.max_peak);
        max_output_step = max_output_step.max(output_metrics.step_peak);

        write_row(
            &mut writer,
            block_index,
            pos,
            input.sample_rate,
            produced,
            &input_metrics,
            &output_metrics,
            &diag,
            &deltas,
        )?;

        prev_diag = Some(diag);
        pos += frames;
        block_index += 1;
    }

    writer
        .flush()
        .map_err(|e| format!("could not flush {}: {e}", opts.output_path.display()))?;

    println!("=== Upmixer Diagnostic Run ===");
    println!("input:  {}", opts.input_path.display());
    println!("output: {}", opts.output_path.display());
    println!("frames: {total_frames}, sample_rate: {}", input.sample_rate);
    println!(
        "speaker_config: {}, output_channels: {out_channels}",
        opts.speaker_config
    );
    println!("frequency_resolution: {}", opts.frequency_resolution);
    println!("max_output_peak:        {max_output_peak:.6}");
    println!("max_output_step:        {max_output_step:.6}");
    println!("max_dialogue_delta:     {max_dialogue_delta:.6}");
    println!("max_dialogue_spatial_delta:{max_dialogue_spatial_delta:.6}");
    println!("max_height_mean_delta:  {max_height_mean_delta:.6}");
    println!("max_decorrelation_delta:{max_decorrelation_delta:.6}");

    Ok(())
}

fn run_isolation(args: Vec<String>) -> Result<(), String> {
    let opts = parse_isolation_options(args)?;
    let input = load_audio_stereo(&opts.input_path)?;

    let total_frames = input.samples.len() / 2;
    let requested_frames =
        ((opts.seconds * input.sample_rate as f32).round() as usize).clamp(1, total_frames.max(1));
    let analysis_frames = requested_frames.min(total_frames);
    if analysis_frames == 0 {
        return Err("input contains no audio frames".to_string());
    }

    fs::create_dir_all(&opts.output_dir)
        .map_err(|e| format!("could not create {}: {e}", opts.output_dir.display()))?;
    let blocks_dir = opts.output_dir.join("blocks");
    fs::create_dir_all(&blocks_dir)
        .map_err(|e| format!("could not create {}: {e}", blocks_dir.display()))?;
    let wavs_dir = opts.output_dir.join("wavs");
    if opts.write_wavs {
        fs::create_dir_all(&wavs_dir)
            .map_err(|e| format!("could not create {}: {e}", wavs_dir.display()))?;
    }

    let summary_path = opts.output_dir.join("summary.csv");
    let events_path = opts.output_dir.join("events.csv");
    let mut summary_writer = BufWriter::new(
        File::create(&summary_path)
            .map_err(|e| format!("could not create {}: {e}", summary_path.display()))?,
    );
    let mut events_writer = BufWriter::new(
        File::create(&events_path)
            .map_err(|e| format!("could not create {}: {e}", events_path.display()))?,
    );
    write_isolation_summary_header(&mut summary_writer)?;
    write_isolation_events_header(&mut events_writer)?;

    let input_artifacts = analyze_input_artifacts(&input.samples[..analysis_frames * 2]);
    let mut results = Vec::new();
    for config in &opts.speaker_configs {
        for variant in build_isolation_variants(&opts, config) {
            let result = run_isolation_variant(
                &input,
                analysis_frames,
                opts.block_size,
                &variant,
                &blocks_dir,
                if opts.write_wavs {
                    Some(wavs_dir.as_path())
                } else {
                    None
                },
            )?;
            write_isolation_summary_row(
                &mut summary_writer,
                &result,
                input.sample_rate,
                analysis_frames,
                &input_artifacts,
            )?;
            write_isolation_event_rows(&mut events_writer, &result, input.sample_rate)?;
            results.push(result);
        }
    }

    summary_writer
        .flush()
        .map_err(|e| format!("could not flush {}: {e}", summary_path.display()))?;
    events_writer
        .flush()
        .map_err(|e| format!("could not flush {}: {e}", events_path.display()))?;

    results.sort_by(|a, b| {
        b.artifacts
            .max_second_diff_rms
            .value
            .partial_cmp(&a.artifacts.max_second_diff_rms.value)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    println!("=== Upmixer Isolation Run ===");
    println!("input:       {}", opts.input_path.display());
    println!("output_dir:  {}", opts.output_dir.display());
    println!("summary:     {}", summary_path.display());
    println!("events:      {}", events_path.display());
    println!(
        "frames:      {analysis_frames}/{total_frames}, sample_rate: {}",
        input.sample_rate
    );
    println!(
        "input_peak:  {:.6}, input_step: {:.6}, input_second_diff: {:.6}",
        input_artifacts.peak.value,
        input_artifacts.max_step.value,
        input_artifacts.max_second_diff.value
    );
    println!("variants:    {}", results.len());
    println!("top high-frequency burst candidates:");
    for result in results.iter().take(8) {
        let event = result.artifacts.max_second_diff_rms;
        println!(
            "  {:<34} rms64={:.6} step={:.6} hop={:.6} t={:.3}s ch={} block={}",
            result.variant.name,
            event.value,
            result.artifacts.max_step.value,
            result.artifacts.max_hop_step.value,
            event.time_sec(input.sample_rate),
            event.channel,
            event.block
        );
    }

    Ok(())
}

fn parse_diagnostic_options(args: Vec<String>) -> Result<DiagnosticOptions, String> {
    let mut positional = Vec::new();
    let mut speaker_config = "5.1".to_string();
    let mut block_size = 1024usize;
    let mut fft_size = 2048usize;
    let mut frequency_resolution = "erb".to_string();
    let mut enable_hr_direct = true;
    let mut bypass_decorrelation = false;
    let mut bypass_transient_detection = false;
    let mut ml_model_path = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_usage();
                process::exit(0);
            }
            "--config" => {
                i += 1;
                speaker_config = args
                    .get(i)
                    .ok_or_else(|| "--config requires a value".to_string())?
                    .clone();
            }
            "--block-size" => {
                i += 1;
                block_size = parse_usize_arg(&args, i, "--block-size")?;
            }
            "--fft-size" => {
                i += 1;
                fft_size = parse_usize_arg(&args, i, "--fft-size")?;
            }
            "--frequency-resolution" => {
                i += 1;
                frequency_resolution = parse_frequency_resolution_arg(&args, i)?;
            }
            "--no-hr" => enable_hr_direct = false,
            "--bypass-decorrelation" => bypass_decorrelation = true,
            "--bypass-transients" => bypass_transient_detection = true,
            "--ml-model" => {
                i += 1;
                ml_model_path = Some(
                    args.get(i)
                        .ok_or_else(|| "--ml-model requires a value".to_string())?
                        .clone(),
                );
            }
            value if value.starts_with('-') => return Err(format!("unknown option: {value}")),
            value => positional.push(value.to_string()),
        }
        i += 1;
    }

    let input_path = positional
        .first()
        .map(PathBuf::from)
        .ok_or_else(|| "missing input WAV path".to_string())?;
    let output_path = positional
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| default_diagnostic_path(&input_path));
    if let Some(config) = positional.get(2) {
        speaker_config = config.clone();
    }
    if !fft_size.is_power_of_two() {
        return Err(format!("--fft-size must be a power of two, got {fft_size}"));
    }
    if block_size == 0 {
        return Err("--block-size must be greater than zero".to_string());
    }

    Ok(DiagnosticOptions {
        input_path,
        output_path,
        speaker_config,
        block_size,
        fft_size,
        frequency_resolution,
        enable_hr_direct,
        bypass_decorrelation,
        bypass_transient_detection,
        ml_model_path,
    })
}

fn parse_isolation_options(args: Vec<String>) -> Result<IsolationOptions, String> {
    let mut positional = Vec::new();
    let mut speaker_configs = Vec::new();
    let mut use_all_configs = false;
    let mut block_size = 1024usize;
    let mut fft_size = 2048usize;
    let mut seconds = 10.0_f32;
    let mut frequency_resolutions = Vec::new();
    let mut write_wavs = false;
    let mut ml_model_path = None;

    let mut i = 0usize;
    while i < args.len() {
        match args[i].as_str() {
            "--help" | "-h" => {
                print_usage();
                process::exit(0);
            }
            "--config" => {
                i += 1;
                speaker_configs.push(
                    args.get(i)
                        .ok_or_else(|| "--config requires a value".to_string())?
                        .clone(),
                );
            }
            "--configs" => {
                i += 1;
                let value = args
                    .get(i)
                    .ok_or_else(|| "--configs requires a comma-separated value".to_string())?;
                for config in value.split(',') {
                    let trimmed = config.trim();
                    if !trimmed.is_empty() {
                        speaker_configs.push(trimmed.to_string());
                    }
                }
            }
            "--all-configs" => use_all_configs = true,
            "--block-size" => {
                i += 1;
                block_size = parse_usize_arg(&args, i, "--block-size")?;
            }
            "--fft-size" => {
                i += 1;
                fft_size = parse_usize_arg(&args, i, "--fft-size")?;
            }
            "--seconds" => {
                i += 1;
                seconds = parse_f32_arg(&args, i, "--seconds")?;
            }
            "--frequency-resolution" => {
                i += 1;
                frequency_resolutions.push(parse_frequency_resolution_arg(&args, i)?);
            }
            "--write-wavs" => write_wavs = true,
            "--ml-model" => {
                i += 1;
                ml_model_path = Some(
                    args.get(i)
                        .ok_or_else(|| "--ml-model requires a value".to_string())?
                        .clone(),
                );
            }
            value if value.starts_with('-') => return Err(format!("unknown option: {value}")),
            value => positional.push(value.to_string()),
        }
        i += 1;
    }

    let input_path = positional
        .first()
        .map(PathBuf::from)
        .ok_or_else(|| "missing input audio path".to_string())?;
    let output_dir = positional
        .get(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| default_isolation_dir(&input_path));
    if let Some(extra) = positional.get(2) {
        return Err(format!("unexpected positional argument: {extra}"));
    }
    if use_all_configs {
        speaker_configs = SPEAKER_CONFIGS
            .iter()
            .copied()
            .filter(|config| *config != "2.0")
            .map(str::to_string)
            .collect();
    }
    if speaker_configs.is_empty() {
        speaker_configs.push("5.1".to_string());
    }
    speaker_configs.sort();
    speaker_configs.dedup();

    if frequency_resolutions.is_empty() {
        frequency_resolutions = vec![
            "erb".to_string(),
            "fine_erb".to_string(),
            "per_bin".to_string(),
        ];
    }
    frequency_resolutions.sort();
    frequency_resolutions.dedup();

    if !fft_size.is_power_of_two() {
        return Err(format!("--fft-size must be a power of two, got {fft_size}"));
    }
    if block_size == 0 {
        return Err("--block-size must be greater than zero".to_string());
    }
    if seconds <= 0.0 || !seconds.is_finite() {
        return Err(format!(
            "--seconds must be finite and greater than zero, got {seconds}"
        ));
    }

    Ok(IsolationOptions {
        input_path,
        output_dir,
        speaker_configs,
        block_size,
        fft_size,
        seconds,
        frequency_resolutions,
        write_wavs,
        ml_model_path,
    })
}

fn parse_frequency_resolution_arg(args: &[String], index: usize) -> Result<String, String> {
    let value = args
        .get(index)
        .ok_or_else(|| "--frequency-resolution requires a value".to_string())?;
    let normalized = value
        .chars()
        .map(|ch| match ch {
            ' ' | '-' => '_',
            _ => ch.to_ascii_lowercase(),
        })
        .collect::<String>();
    match normalized.as_str() {
        "erb" | "fine_erb" | "per_bin" => Ok(normalized),
        _ => Err(format!(
            "--frequency-resolution must be erb, fine_erb, or per_bin; got {value}"
        )),
    }
}

fn parse_usize_arg(args: &[String], index: usize, name: &str) -> Result<usize, String> {
    args.get(index)
        .ok_or_else(|| format!("{name} requires a value"))?
        .parse::<usize>()
        .map_err(|e| format!("invalid {name}: {e}"))
}

fn parse_f32_arg(args: &[String], index: usize, name: &str) -> Result<f32, String> {
    args.get(index)
        .ok_or_else(|| format!("{name} requires a value"))?
        .parse::<f32>()
        .map_err(|e| format!("invalid {name}: {e}"))
}

fn default_diagnostic_path(input_path: &Path) -> PathBuf {
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("upmixer");
    input_path.with_file_name(format!("{stem}.upmixer-diagnostics.csv"))
}

fn default_isolation_dir(input_path: &Path) -> PathBuf {
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("upmixer");
    input_path.with_file_name(format!("{stem}.upmixer-isolate"))
}

struct InputAudio {
    sample_rate: u32,
    samples: Vec<f32>,
}

fn load_audio_stereo(path: &Path) -> Result<InputAudio, String> {
    let bytes =
        std::fs::read(path).map_err(|e| format!("could not read {}: {e}", path.display()))?;
    match load_wav_stereo_from_bytes(path, bytes) {
        Ok(input) => Ok(input),
        Err(wav_err) => {
            let wav_bytes = decode_audio_with_ffmpeg(path, &wav_err)?;
            load_wav_stereo_from_bytes(path, wav_bytes)
        }
    }
}

fn load_wav_stereo_from_bytes(path: &Path, mut bytes: Vec<u8>) -> Result<InputAudio, String> {
    if !bytes.starts_with(b"RIFF") {
        let riff_start = bytes
            .windows(4)
            .position(|w| w == b"RIFF")
            .ok_or_else(|| format!("could not find RIFF header in {}", path.display()))?;
        bytes.drain(..riff_start);
    }

    let mut reader = WavReader::new(Cursor::new(bytes))
        .map_err(|e| format!("could not parse {}: {e}", path.display()))?;
    let spec = reader.spec();
    let channels = spec.channels.max(1) as usize;

    let raw = match spec.sample_format {
        SampleFormat::Float => reader
            .samples::<f32>()
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| format!("could not read float samples: {e}"))?,
        SampleFormat::Int => {
            let bits = spec.bits_per_sample.clamp(1, 32);
            let denom = (1_i64 << (bits - 1)) as f32;
            reader
                .samples::<i32>()
                .map(|s| s.map(|v| (v as f32 / denom).clamp(-1.0, 1.0)))
                .collect::<Result<Vec<_>, _>>()
                .map_err(|e| format!("could not read integer samples: {e}"))?
        }
    };

    let frames = raw.len() / channels;
    let mut stereo = Vec::with_capacity(frames * 2);
    for frame in raw.chunks_exact(channels) {
        let left = frame[0];
        let right = if channels > 1 { frame[1] } else { left };
        stereo.push(left);
        stereo.push(right);
    }

    Ok(InputAudio {
        sample_rate: spec.sample_rate,
        samples: stereo,
    })
}

fn decode_audio_with_ffmpeg(path: &Path, wav_err: &str) -> Result<Vec<u8>, String> {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    let temp_path = env::temp_dir().join(format!("qa-upmixer-{}-{unique}.wav", process::id()));
    let output = Command::new("ffmpeg")
        .args(["-hide_banner", "-loglevel", "error", "-y", "-i"])
        .arg(path)
        .args(["-ac", "2", "-c:a", "pcm_f32le"])
        .arg(&temp_path)
        .output()
        .map_err(|e| {
            format!(
                "could not parse {} as WAV ({wav_err}); ffmpeg fallback failed to start: {e}",
                path.display()
            )
        })?;
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let _ = fs::remove_file(&temp_path);
        return Err(format!(
            "could not parse {} as WAV ({wav_err}); ffmpeg fallback failed: {stderr}",
            path.display()
        ));
    }
    let bytes = fs::read(&temp_path)
        .map_err(|e| format!("could not read ffmpeg output {}: {e}", temp_path.display()))?;
    let _ = fs::remove_file(&temp_path);
    Ok(bytes)
}

#[derive(Debug, Clone, Copy, Default)]
struct InputMetrics {
    peak: f32,
    rms: f32,
}

fn input_metrics(samples: &[f32]) -> InputMetrics {
    if samples.is_empty() {
        return InputMetrics::default();
    }

    let mut peak = 0.0_f32;
    let mut energy = 0.0_f32;
    for &sample in samples {
        peak = peak.max(sample.abs());
        energy += sample * sample;
    }
    InputMetrics {
        peak,
        rms: (energy / samples.len() as f32).sqrt(),
    }
}

#[derive(Debug, Clone, Default)]
struct ChannelMetrics {
    peaks: Vec<f32>,
    rms: Vec<f32>,
    max_peak: f32,
    rms_sum: f32,
    step_peak: f32,
}

fn channel_metrics(
    samples: &[f32],
    channels: usize,
    frames: usize,
    prev_last: &mut [f32],
) -> ChannelMetrics {
    let mut metrics = ChannelMetrics {
        peaks: vec![0.0; channels],
        rms: vec![0.0; channels],
        ..Default::default()
    };
    if frames == 0 || channels == 0 {
        return metrics;
    }

    for frame in 0..frames {
        for ch in 0..channels {
            let idx = frame * channels + ch;
            let sample = samples[idx];
            let abs = sample.abs();
            metrics.peaks[ch] = metrics.peaks[ch].max(abs);
            metrics.max_peak = metrics.max_peak.max(abs);
            metrics.rms[ch] += sample * sample;

            let prev = if frame == 0 {
                prev_last[ch]
            } else {
                samples[(frame - 1) * channels + ch]
            };
            metrics.step_peak = metrics.step_peak.max((sample - prev).abs());
        }
    }

    for ch in 0..channels {
        metrics.rms[ch] = (metrics.rms[ch] / frames as f32).sqrt();
        metrics.rms_sum += metrics.rms[ch];
        prev_last[ch] = samples[(frames - 1) * channels + ch];
    }

    metrics
}

#[derive(Debug, Clone, Copy, Default)]
struct DiagnosticDeltas {
    dialogue_probability_abs: f32,
    dialogue_spatial_control_abs: f32,
    height_gain_mean_abs: f32,
    height_gate_mean_abs: f32,
    decorrelation_abs: f32,
    safety_scale_abs: f32,
}

impl DiagnosticDeltas {
    fn from_previous(current: &UpmixerDiagnostics, previous: Option<&UpmixerDiagnostics>) -> Self {
        if let Some(previous) = previous {
            Self {
                dialogue_probability_abs: (current.dialogue_probability
                    - previous.dialogue_probability)
                    .abs(),
                dialogue_spatial_control_abs: (current.dialogue_spatial_control
                    - previous.dialogue_spatial_control)
                    .abs(),
                height_gain_mean_abs: (current.height_gain.mean - previous.height_gain.mean).abs(),
                height_gate_mean_abs: (current.height_flux_gate.mean
                    - previous.height_flux_gate.mean)
                    .abs(),
                decorrelation_abs: (current.decorrelation_strength
                    - previous.decorrelation_strength)
                    .abs(),
                safety_scale_abs: (current.safety_scale - previous.safety_scale).abs(),
            }
        } else {
            Self::default()
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct DiagnosticMaxDeltas {
    dialogue_probability_abs: f32,
    dialogue_spatial_control_abs: f32,
    height_gain_mean_abs: f32,
    height_gate_mean_abs: f32,
    decorrelation_abs: f32,
    safety_scale_abs: f32,
}

impl DiagnosticMaxDeltas {
    fn observe(&mut self, deltas: &DiagnosticDeltas) {
        self.dialogue_probability_abs = self
            .dialogue_probability_abs
            .max(deltas.dialogue_probability_abs);
        self.dialogue_spatial_control_abs = self
            .dialogue_spatial_control_abs
            .max(deltas.dialogue_spatial_control_abs);
        self.height_gain_mean_abs = self.height_gain_mean_abs.max(deltas.height_gain_mean_abs);
        self.height_gate_mean_abs = self.height_gate_mean_abs.max(deltas.height_gate_mean_abs);
        self.decorrelation_abs = self.decorrelation_abs.max(deltas.decorrelation_abs);
        self.safety_scale_abs = self.safety_scale_abs.max(deltas.safety_scale_abs);
    }
}

#[derive(Debug, Clone, Copy, Default)]
struct ArtifactEvent {
    value: f32,
    frame: usize,
    channel: usize,
    block: usize,
}

impl ArtifactEvent {
    fn time_sec(self, sample_rate: u32) -> f64 {
        self.frame as f64 / sample_rate as f64
    }
}

#[derive(Debug, Clone, Default)]
struct ArtifactMetrics {
    peak: ArtifactEvent,
    max_step: ArtifactEvent,
    max_boundary_step: ArtifactEvent,
    max_hop_step: ArtifactEvent,
    max_second_diff: ArtifactEvent,
    max_second_diff_rms: ArtifactEvent,
}

struct ArtifactTracker {
    channels: usize,
    prev1: Vec<f32>,
    prev2: Vec<f32>,
    has_prev1: Vec<bool>,
    has_prev2: Vec<bool>,
    window_sum_sq: Vec<f32>,
    window_count: Vec<usize>,
    window_start_frame: Vec<usize>,
    window_size: usize,
    hop_size: Option<usize>,
    metrics: ArtifactMetrics,
}

impl ArtifactTracker {
    fn new(channels: usize, window_size: usize, hop_size: Option<usize>) -> Self {
        Self {
            channels,
            prev1: vec![0.0; channels],
            prev2: vec![0.0; channels],
            has_prev1: vec![false; channels],
            has_prev2: vec![false; channels],
            window_sum_sq: vec![0.0; channels],
            window_count: vec![0; channels],
            window_start_frame: vec![0; channels],
            window_size: window_size.max(1),
            hop_size: hop_size.filter(|value| *value > 0),
            metrics: ArtifactMetrics::default(),
        }
    }

    fn observe_block(
        &mut self,
        samples: &[f32],
        frames: usize,
        block_index: usize,
        absolute_frame_start: usize,
    ) {
        if frames == 0 || self.channels == 0 {
            return;
        }

        for frame in 0..frames {
            let absolute_frame = absolute_frame_start + frame;
            for ch in 0..self.channels {
                let sample = samples[frame * self.channels + ch];
                self.observe_peak(sample, absolute_frame, ch, block_index);

                if self.has_prev1[ch] {
                    let step = (sample - self.prev1[ch]).abs();
                    self.observe_step(step, absolute_frame, ch, block_index);
                    if frame == 0 {
                        self.observe_boundary_step(step, absolute_frame, ch, block_index);
                    }
                    if self.hop_size.is_some_and(|hop_size| {
                        absolute_frame > 0 && absolute_frame.is_multiple_of(hop_size)
                    }) {
                        self.observe_hop_step(step, absolute_frame, ch, block_index);
                    }
                }

                if self.has_prev2[ch] {
                    let second_diff = (sample - 2.0 * self.prev1[ch] + self.prev2[ch]).abs();
                    self.observe_second_diff(second_diff, absolute_frame, ch, block_index);
                    self.observe_second_diff_window(second_diff, absolute_frame, ch, block_index);
                }

                if self.has_prev1[ch] {
                    self.prev2[ch] = self.prev1[ch];
                    self.has_prev2[ch] = true;
                }
                self.prev1[ch] = sample;
                self.has_prev1[ch] = true;
            }
        }
    }

    fn finish(mut self) -> ArtifactMetrics {
        for ch in 0..self.channels {
            self.flush_second_diff_window(ch, self.window_start_frame[ch], 0);
        }
        self.metrics
    }

    fn observe_peak(&mut self, sample: f32, frame: usize, channel: usize, block: usize) {
        let value = sample.abs();
        if value > self.metrics.peak.value {
            self.metrics.peak = ArtifactEvent {
                value,
                frame,
                channel,
                block,
            };
        }
    }

    fn observe_step(&mut self, value: f32, frame: usize, channel: usize, block: usize) {
        if value > self.metrics.max_step.value {
            self.metrics.max_step = ArtifactEvent {
                value,
                frame,
                channel,
                block,
            };
        }
    }

    fn observe_boundary_step(&mut self, value: f32, frame: usize, channel: usize, block: usize) {
        if value > self.metrics.max_boundary_step.value {
            self.metrics.max_boundary_step = ArtifactEvent {
                value,
                frame,
                channel,
                block,
            };
        }
    }

    fn observe_hop_step(&mut self, value: f32, frame: usize, channel: usize, block: usize) {
        if value > self.metrics.max_hop_step.value {
            self.metrics.max_hop_step = ArtifactEvent {
                value,
                frame,
                channel,
                block,
            };
        }
    }

    fn observe_second_diff(&mut self, value: f32, frame: usize, channel: usize, block: usize) {
        if value > self.metrics.max_second_diff.value {
            self.metrics.max_second_diff = ArtifactEvent {
                value,
                frame,
                channel,
                block,
            };
        }
    }

    fn observe_second_diff_window(
        &mut self,
        value: f32,
        frame: usize,
        channel: usize,
        block: usize,
    ) {
        if self.window_count[channel] == 0 {
            self.window_start_frame[channel] = frame;
        }
        self.window_sum_sq[channel] += value * value;
        self.window_count[channel] += 1;
        if self.window_count[channel] >= self.window_size {
            self.flush_second_diff_window(channel, self.window_start_frame[channel], block);
        }
    }

    fn flush_second_diff_window(&mut self, channel: usize, frame: usize, block: usize) {
        let count = self.window_count[channel];
        if count == 0 {
            return;
        }
        let rms = (self.window_sum_sq[channel] / count as f32).sqrt();
        if rms > self.metrics.max_second_diff_rms.value {
            self.metrics.max_second_diff_rms = ArtifactEvent {
                value: rms,
                frame,
                channel,
                block,
            };
        }
        self.window_sum_sq[channel] = 0.0;
        self.window_count[channel] = 0;
    }
}

fn analyze_input_artifacts(samples: &[f32]) -> ArtifactMetrics {
    let frames = samples.len() / 2;
    let mut tracker = ArtifactTracker::new(2, 64, None);
    tracker.observe_block(samples, frames, 0, 0);
    tracker.finish()
}

fn write_header(writer: &mut dyn Write, channels: usize) -> Result<(), String> {
    write!(
        writer,
        "block,start_frame,time_sec,frames_produced,input_peak,input_rms,output_peak_max,output_rms_sum,output_step_peak,dialogue_probability,dialogue_delta,dialogue_spatial_control,dialogue_spatial_delta,dialogue_centroid_hz,dialogue_envelope_variance,decorrelation_strength,decorrelation_delta,hr_direct_envelope,hr_transient_env,height_transient_env,spectral_flux_smooth,height_spectral_flux_smooth,safety_scale,safety_delta,output_accumulator_fill,height_gain_mean,height_gain_min,height_gain_max,height_gain_stddev,height_gain_mean_delta,height_gate_mean,height_gate_min,height_gate_max,height_gate_stddev,height_gate_mean_delta,coherence_mean,coherence_min,coherence_max,coherence_stddev"
    )
    .map_err(|e| e.to_string())?;
    for ch in 0..channels {
        write!(writer, ",out_peak_ch{ch},out_rms_ch{ch}").map_err(|e| e.to_string())?;
    }
    writeln!(writer).map_err(|e| e.to_string())
}

fn write_row(
    writer: &mut dyn Write,
    block: usize,
    start_frame: usize,
    sample_rate: u32,
    frames_produced: usize,
    input: &InputMetrics,
    output: &ChannelMetrics,
    diag: &UpmixerDiagnostics,
    deltas: &DiagnosticDeltas,
) -> Result<(), String> {
    let time_sec = start_frame as f64 / sample_rate as f64;
    write!(
        writer,
        "{block},{start_frame},{time_sec:.9},{frames_produced},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.3},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9}",
        input.peak,
        input.rms,
        output.max_peak,
        output.rms_sum,
        output.step_peak,
        diag.dialogue_probability,
        deltas.dialogue_probability_abs,
        diag.dialogue_spatial_control,
        deltas.dialogue_spatial_control_abs,
        diag.dialogue_spectral_centroid_hz,
        diag.dialogue_envelope_variance,
        diag.decorrelation_strength,
        deltas.decorrelation_abs,
        diag.hr_direct_envelope,
        diag.hr_transient_env,
        diag.height_transient_env,
        diag.spectral_flux_smooth,
        diag.height_spectral_flux_smooth,
        diag.safety_scale,
        deltas.safety_scale_abs,
        diag.output_accumulator_fill,
        diag.height_gain.mean,
        diag.height_gain.min,
        diag.height_gain.max,
        diag.height_gain.stddev,
        deltas.height_gain_mean_abs,
        diag.height_flux_gate.mean,
        diag.height_flux_gate.min,
        diag.height_flux_gate.max,
        diag.height_flux_gate.stddev,
        deltas.height_gate_mean_abs,
        diag.coherence.mean,
        diag.coherence.min,
        diag.coherence.max,
        diag.coherence.stddev,
    )
    .map_err(|e| e.to_string())?;

    for ch in 0..output.peaks.len() {
        write!(writer, ",{:.9},{:.9}", output.peaks[ch], output.rms[ch])
            .map_err(|e| e.to_string())?;
    }
    writeln!(writer).map_err(|e| e.to_string())
}

fn build_isolation_variants(opts: &IsolationOptions, config: &str) -> Vec<IsolationVariant> {
    let mut variants = Vec::new();
    for frequency_resolution in &opts.frequency_resolutions {
        let mut base = UpmixerPluginParams {
            fft_size: opts.fft_size,
            speaker_config: config.to_string(),
            frequency_resolution: frequency_resolution.clone(),
            ..Default::default()
        };
        if let Some(model_path) = opts.ml_model_path.clone() {
            base.enable_ml_detection = true;
            base.ml_model_path = model_path;
        }

        push_isolation_variant(
            &mut variants,
            config,
            frequency_resolution,
            "high_full",
            "full high-latency processing",
            base.clone(),
        );

        let mut low_latency = base.clone();
        low_latency.low_latency = true;
        push_isolation_variant(
            &mut variants,
            config,
            frequency_resolution,
            "low_full",
            "full low-latency processing",
            low_latency,
        );

        let mut no_hr = base.clone();
        no_hr.enable_hr_direct = false;
        push_isolation_variant(
            &mut variants,
            config,
            frequency_resolution,
            "high_no_hr",
            "high-latency processing with HR direct path disabled",
            no_hr,
        );

        let mut no_decorrelation = base.clone();
        no_decorrelation.bypass_decorrelation = true;
        push_isolation_variant(
            &mut variants,
            config,
            frequency_resolution,
            "high_no_decorrelation",
            "high-latency processing with decorrelation bypassed",
            no_decorrelation,
        );

        let mut no_transients = base.clone();
        no_transients.bypass_transient_detection = true;
        push_isolation_variant(
            &mut variants,
            config,
            frequency_resolution,
            "high_no_transients",
            "high-latency processing with transient-adaptive controls bypassed",
            no_transients,
        );

        let mut no_height = base.clone();
        no_height.height_gain = 0.0;
        no_height.height_direct_leak = 0.0;
        no_height.rear_late_reflection = 0.0;
        push_isolation_variant(
            &mut variants,
            config,
            frequency_resolution,
            "high_no_height",
            "high-latency processing with height routing disabled",
            no_height,
        );

        let mut no_ambient = base.clone();
        no_ambient.gain_front_ambient = 0.0;
        no_ambient.gain_rear_ambient = 0.0;
        no_ambient.ambient_boost = 0.5;
        no_ambient.surround_direct_bleed = 0.0;
        no_ambient.rear_ambient_boost = 1.0;
        no_ambient.rear_late_reflection = 0.0;
        push_isolation_variant(
            &mut variants,
            config,
            frequency_resolution,
            "high_no_ambient",
            "high-latency processing with ambient/surround routing minimized",
            no_ambient,
        );

        let mut center_off = base.clone();
        center_off.center_spread = 1.0;
        center_off.dialogue_weight = 0.0;
        push_isolation_variant(
            &mut variants,
            config,
            frequency_resolution,
            "high_center_off",
            "high-latency processing with center extraction spread out",
            center_off,
        );

        let mut fft_front_only = base.clone();
        fft_front_only.gain_front_ambient = 0.0;
        fft_front_only.gain_rear_ambient = 0.0;
        fft_front_only.height_gain = 0.0;
        fft_front_only.height_direct_leak = 0.0;
        fft_front_only.lfe_gain = 0.0;
        fft_front_only.surround_direct_bleed = 0.0;
        fft_front_only.rear_late_reflection = 0.0;
        fft_front_only.center_spread = 1.0;
        fft_front_only.dialogue_weight = 0.0;
        push_isolation_variant(
            &mut variants,
            config,
            frequency_resolution,
            "high_fft_front_only",
            "high-latency FFT path with only front direct routing left active",
            fft_front_only,
        );

        let mut bypass_all = base;
        bypass_all.bypass_all_processing = true;
        bypass_all.enable_hr_direct = false;
        push_isolation_variant(
            &mut variants,
            config,
            frequency_resolution,
            "bypass_all",
            "pure stereo pass-through through the upmixer output contract",
            bypass_all,
        );
    }
    variants
}

fn push_isolation_variant(
    variants: &mut Vec<IsolationVariant>,
    config: &str,
    frequency_resolution: &str,
    suffix: &str,
    notes: &str,
    params: UpmixerPluginParams,
) {
    let name = format!(
        "cfg{}_{}_{}",
        safe_filename_fragment(config),
        safe_filename_fragment(frequency_resolution),
        suffix
    );
    variants.push(IsolationVariant {
        name,
        config: config.to_string(),
        frequency_resolution: frequency_resolution.to_string(),
        notes: notes.to_string(),
        params,
    });
}

fn run_isolation_variant(
    input: &InputAudio,
    analysis_frames: usize,
    block_size: usize,
    variant: &IsolationVariant,
    blocks_dir: &Path,
    wavs_dir: Option<&Path>,
) -> Result<IsolationRunResult, String> {
    let mut plugin = UpmixerPlugin::from_params(variant.params.clone());
    plugin.initialize(input.sample_rate)?;
    let out_channels = plugin.output_channels();

    let block_csv_path = blocks_dir.join(format!("{}.csv", variant.name));
    let block_file = File::create(&block_csv_path)
        .map_err(|e| format!("could not create {}: {e}", block_csv_path.display()))?;
    let mut block_writer = BufWriter::new(block_file);
    write_header(&mut block_writer, out_channels)?;

    let wav_path = wavs_dir.map(|dir| dir.join(format!("{}.wav", variant.name)));
    let mut wav_writer = if let Some(path) = wav_path.as_ref() {
        Some(create_wav_writer(path, out_channels, input.sample_rate)?)
    } else {
        None
    };

    let mut pos = 0usize;
    let mut block_index = 0usize;
    let mut produced_total = 0usize;
    let mut prev_diag: Option<UpmixerDiagnostics> = None;
    let mut prev_output_last = vec![0.0_f32; out_channels];
    let hop_size = if variant.params.low_latency {
        512
    } else {
        variant.params.fft_size / 2
    };
    let mut artifact_tracker = ArtifactTracker::new(out_channels, 64, Some(hop_size));
    let mut max_deltas = DiagnosticMaxDeltas::default();

    while pos < analysis_frames {
        let frames = block_size.min(analysis_frames - pos);
        let input_slice = &input.samples[pos * 2..(pos + frames) * 2];
        let mut output = vec![0.0_f32; frames * out_channels];
        let context = ProcessContext::new(input.sample_rate, frames);
        let produced = plugin.process(input_slice, &mut output, &context)?;
        let diag = plugin.diagnostics();

        let input_metrics = input_metrics(input_slice);
        let output_metrics =
            channel_metrics(&output, out_channels, produced, &mut prev_output_last);
        let deltas = DiagnosticDeltas::from_previous(&diag, prev_diag.as_ref());
        max_deltas.observe(&deltas);

        write_row(
            &mut block_writer,
            block_index,
            pos,
            input.sample_rate,
            produced,
            &input_metrics,
            &output_metrics,
            &diag,
            &deltas,
        )?;

        let produced_samples = produced * out_channels;
        artifact_tracker.observe_block(
            &output[..produced_samples],
            produced,
            block_index,
            produced_total,
        );
        if let Some(writer) = wav_writer.as_mut() {
            for &sample in &output[..produced_samples] {
                writer
                    .write_sample(if sample.is_finite() { sample } else { 0.0 })
                    .map_err(|e| format!("could not write {}: {e}", variant.name))?;
            }
        }

        prev_diag = Some(diag);
        produced_total += produced;
        pos += frames;
        block_index += 1;
    }

    block_writer
        .flush()
        .map_err(|e| format!("could not flush {}: {e}", block_csv_path.display()))?;
    if let Some(writer) = wav_writer.take() {
        writer
            .finalize()
            .map_err(|e| format!("could not finalize {}: {e}", variant.name))?;
    }

    Ok(IsolationRunResult {
        variant: variant.clone(),
        output_channels: out_channels,
        frames_produced: produced_total,
        block_csv_path,
        wav_path,
        artifacts: artifact_tracker.finish(),
        max_deltas,
    })
}

fn create_wav_writer(
    path: &Path,
    channels: usize,
    sample_rate: u32,
) -> Result<WavWriter<BufWriter<File>>, String> {
    let spec = WavSpec {
        channels: channels as u16,
        sample_rate,
        bits_per_sample: 32,
        sample_format: SampleFormat::Float,
    };
    WavWriter::create(path, spec).map_err(|e| format!("could not create {}: {e}", path.display()))
}

fn write_isolation_summary_header(writer: &mut dyn Write) -> Result<(), String> {
    write_csv_record(
        writer,
        &[
            "variant",
            "config",
            "frequency_resolution",
            "low_latency",
            "notes",
            "output_channels",
            "analysis_frames",
            "frames_produced",
            "block_csv",
            "output_wav",
            "output_peak",
            "output_peak_time_sec",
            "output_peak_channel",
            "output_max_step",
            "output_max_step_time_sec",
            "output_max_step_channel",
            "output_max_step_block",
            "output_boundary_step",
            "output_boundary_step_time_sec",
            "output_boundary_step_channel",
            "output_boundary_step_block",
            "output_hop_step",
            "output_hop_step_time_sec",
            "output_hop_step_channel",
            "output_hop_step_block",
            "output_second_diff",
            "output_second_diff_time_sec",
            "output_second_diff_channel",
            "output_second_diff_rms64",
            "output_second_diff_rms64_time_sec",
            "output_second_diff_rms64_channel",
            "input_peak",
            "input_max_step",
            "input_second_diff",
            "output_to_input_step_ratio",
            "enable_hr_direct",
            "bypass_decorrelation",
            "bypass_transient_detection",
            "bypass_all_processing",
            "height_gain",
            "center_spread",
            "gain_front_ambient",
            "gain_rear_ambient",
            "surround_direct_bleed",
            "max_dialogue_delta",
            "max_dialogue_spatial_delta",
            "max_height_gain_mean_delta",
            "max_height_gate_mean_delta",
            "max_decorrelation_delta",
            "max_safety_delta",
        ],
    )
}

fn write_isolation_summary_row(
    writer: &mut dyn Write,
    result: &IsolationRunResult,
    sample_rate: u32,
    analysis_frames: usize,
    input_artifacts: &ArtifactMetrics,
) -> Result<(), String> {
    let artifacts = &result.artifacts;
    let params = &result.variant.params;
    let step_ratio = if input_artifacts.max_step.value > 1e-9 {
        artifacts.max_step.value / input_artifacts.max_step.value
    } else {
        0.0
    };
    write_csv_record(
        writer,
        &[
            result.variant.name.clone(),
            result.variant.config.clone(),
            result.variant.frequency_resolution.clone(),
            params.low_latency.to_string(),
            result.variant.notes.clone(),
            result.output_channels.to_string(),
            analysis_frames.to_string(),
            result.frames_produced.to_string(),
            result.block_csv_path.display().to_string(),
            result
                .wav_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_default(),
            format!("{:.9}", artifacts.peak.value),
            format!("{:.9}", artifacts.peak.time_sec(sample_rate)),
            artifacts.peak.channel.to_string(),
            format!("{:.9}", artifacts.max_step.value),
            format!("{:.9}", artifacts.max_step.time_sec(sample_rate)),
            artifacts.max_step.channel.to_string(),
            artifacts.max_step.block.to_string(),
            format!("{:.9}", artifacts.max_boundary_step.value),
            format!("{:.9}", artifacts.max_boundary_step.time_sec(sample_rate)),
            artifacts.max_boundary_step.channel.to_string(),
            artifacts.max_boundary_step.block.to_string(),
            format!("{:.9}", artifacts.max_hop_step.value),
            format!("{:.9}", artifacts.max_hop_step.time_sec(sample_rate)),
            artifacts.max_hop_step.channel.to_string(),
            artifacts.max_hop_step.block.to_string(),
            format!("{:.9}", artifacts.max_second_diff.value),
            format!("{:.9}", artifacts.max_second_diff.time_sec(sample_rate)),
            artifacts.max_second_diff.channel.to_string(),
            format!("{:.9}", artifacts.max_second_diff_rms.value),
            format!("{:.9}", artifacts.max_second_diff_rms.time_sec(sample_rate)),
            artifacts.max_second_diff_rms.channel.to_string(),
            format!("{:.9}", input_artifacts.peak.value),
            format!("{:.9}", input_artifacts.max_step.value),
            format!("{:.9}", input_artifacts.max_second_diff.value),
            format!("{step_ratio:.9}"),
            params.enable_hr_direct.to_string(),
            params.bypass_decorrelation.to_string(),
            params.bypass_transient_detection.to_string(),
            params.bypass_all_processing.to_string(),
            format!("{:.6}", params.height_gain),
            format!("{:.6}", params.center_spread),
            format!("{:.6}", params.gain_front_ambient),
            format!("{:.6}", params.gain_rear_ambient),
            format!("{:.6}", params.surround_direct_bleed),
            format!("{:.9}", result.max_deltas.dialogue_probability_abs),
            format!("{:.9}", result.max_deltas.dialogue_spatial_control_abs),
            format!("{:.9}", result.max_deltas.height_gain_mean_abs),
            format!("{:.9}", result.max_deltas.height_gate_mean_abs),
            format!("{:.9}", result.max_deltas.decorrelation_abs),
            format!("{:.9}", result.max_deltas.safety_scale_abs),
        ],
    )
}

fn write_isolation_events_header(writer: &mut dyn Write) -> Result<(), String> {
    write_csv_record(
        writer,
        &[
            "variant", "event", "value", "time_sec", "frame", "channel", "block", "notes",
        ],
    )
}

fn write_isolation_event_rows(
    writer: &mut dyn Write,
    result: &IsolationRunResult,
    sample_rate: u32,
) -> Result<(), String> {
    let metrics = &result.artifacts;
    for (event_name, event) in [
        ("peak", metrics.peak),
        ("max_step", metrics.max_step),
        ("max_boundary_step", metrics.max_boundary_step),
        ("max_hop_step", metrics.max_hop_step),
        ("max_second_diff", metrics.max_second_diff),
        ("max_second_diff_rms64", metrics.max_second_diff_rms),
    ] {
        write_csv_record(
            writer,
            &[
                result.variant.name.clone(),
                event_name.to_string(),
                format!("{:.9}", event.value),
                format!("{:.9}", event.time_sec(sample_rate)),
                event.frame.to_string(),
                event.channel.to_string(),
                event.block.to_string(),
                result.variant.notes.clone(),
            ],
        )?;
    }
    Ok(())
}

fn write_csv_record(writer: &mut dyn Write, fields: &[impl AsRef<str>]) -> Result<(), String> {
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            write!(writer, ",").map_err(|e| e.to_string())?;
        }
        write!(writer, "{}", csv_escape(field.as_ref())).map_err(|e| e.to_string())?;
    }
    writeln!(writer).map_err(|e| e.to_string())
}

fn csv_escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

fn safe_filename_fragment(value: &str) -> String {
    let mut safe = String::with_capacity(value.len());
    let mut last_was_separator = false;
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            safe.push(ch.to_ascii_lowercase());
            last_was_separator = false;
        } else if !last_was_separator {
            safe.push('_');
            last_was_separator = true;
        }
    }
    safe.trim_matches('_').to_string()
}

fn run_self_qa() {
    let sample_rate = 48000;
    let params = UpmixerPluginParams {
        fft_size: 2048,
        speaker_config: "5.1".to_string(),
        gain_front_direct: 1.0,
        center_spread: 0.0,
        ..Default::default()
    };

    let mut plugin = UpmixerPlugin::from_params(params);
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: Upmixer Plugin ===");

    // Test 1: Center Extraction (Coherent Mono Input)
    println!("\n[Test 1] Center Extraction (Coherent Mono Input)");
    let num_frames = 16384; // 340ms
    let mut input = vec![0.0_f32; num_frames * 2];
    for i in 0..num_frames {
        let s = (2.0 * PI * 1000.0 * i as f32 / sample_rate as f32).sin() * 0.5;
        input[i * 2] = s;
        input[i * 2 + 1] = s;
    }
    let mut output = vec![0.0_f32; num_frames * 6];

    // Process in blocks of 1024
    let block_size = 1024;
    let mut pos = 0;
    while pos < num_frames {
        let end = (pos + block_size).min(num_frames);
        let ctx = ProcessContext::new(sample_rate, end - pos);
        plugin
            .process(
                &input[pos * 2..end * 2],
                &mut output[pos * 6..end * 6],
                &ctx,
            )
            .unwrap();
        pos = end;
    }

    // Measure energies in last 100ms
    let measure_start = num_frames - 4800;
    let mut energies = vec![0.0f32; 6];
    for i in measure_start..num_frames {
        for ch in 0..6 {
            let s = output[i * 6 + ch];
            energies[ch] += s * s;
        }
    }

    println!("  Channel Energies (FL, FR, C, LFE, SL, SR):");
    println!("  {:?}", energies);

    // For coherent input, Center (idx 2) should be dominant
    assert!(energies[2] > 1.0, "Center should have significant energy");
    assert!(
        energies[2] > energies[0],
        "Center should be stronger than FL"
    );
    println!("  Center Extraction: PASS");

    // Test 2: Center Spread
    println!("\n[Test 2] Center Spread (spread=1.0)");
    plugin
        .set_parameter("center_spread".into(), ParameterValue::Float(1.0))
        .unwrap();

    // Process another 1s to see change
    let mut output2 = vec![0.0_f32; num_frames * 6];
    let mut pos = 0;
    while pos < num_frames {
        let end = (pos + block_size).min(num_frames);
        let ctx = ProcessContext::new(sample_rate, end - pos);
        plugin
            .process(
                &input[pos * 2..end * 2],
                &mut output2[pos * 6..end * 6],
                &ctx,
            )
            .unwrap();
        pos = end;
    }

    let mut energies_spread = vec![0.0f32; 6];
    for i in measure_start..num_frames {
        for ch in 0..6 {
            let s = output2[i * 6 + ch];
            energies_spread[ch] += s * s;
        }
    }
    println!("  Channel Energies (spread=1.0):");
    println!("  {:?}", energies_spread);

    assert!(
        energies_spread[2] < energies[2] * 0.2,
        "Center energy should have dropped"
    );
    assert!(
        energies_spread[0] > energies[0],
        "Front Left energy should have increased"
    );
    println!("  Center Spread: PASS");

    // Run standard QA tests
    run_standard_tests(&mut plugin, "UpmixerPlugin");

    println!("\n[ALL PASS] Upmixer QA Complete.");
}
