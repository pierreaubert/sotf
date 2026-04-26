use hound::{SampleFormat, WavReader};
use sotf_host::{CountingAlloc, Plugin, ProcessContext};
use sotf_host::{ParameterValue, run_standard_tests};
use sotf_plugin_upmixer::{UpmixerDiagnostics, UpmixerPlugin, UpmixerPluginParams};
use std::env;
use std::f32::consts::PI;
use std::fs::File;
use std::io::{BufWriter, Cursor, Write};
use std::path::{Path, PathBuf};
use std::process;

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
        "Usage:\n  qa-upmixer\n  qa-upmixer diagnose <input.wav> [output.csv] [--config 5.1] [--block-size 1024] [--fft-size 2048] [--no-hr] [--bypass-decorrelation] [--bypass-transients] [--ml-model model.onnx]"
    );
}

#[derive(Debug)]
struct DiagnosticOptions {
    input_path: PathBuf,
    output_path: PathBuf,
    speaker_config: String,
    block_size: usize,
    fft_size: usize,
    enable_hr_direct: bool,
    bypass_decorrelation: bool,
    bypass_transient_detection: bool,
    ml_model_path: Option<String>,
}

fn run_diagnostic(args: Vec<String>) -> Result<(), String> {
    let opts = parse_diagnostic_options(args)?;
    let input = load_wav_stereo(&opts.input_path)?;

    let mut params = UpmixerPluginParams {
        fft_size: opts.fft_size,
        speaker_config: opts.speaker_config.clone(),
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
    let mut max_height_mean_delta = 0.0_f32;
    let mut max_decorrelation_delta = 0.0_f32;
    let mut max_output_peak = 0.0_f32;
    let mut max_output_step = 0.0_f32;

    while pos < total_frames {
        let frames = opts.block_size.min(total_frames - pos);
        let input_slice = &input.samples[pos * 2..(pos + frames) * 2];
        let mut output = vec![0.0_f32; frames * out_channels];
        let context = ProcessContext {
            sample_rate: input.sample_rate,
            num_frames: frames,
        };
        let produced = plugin.process(input_slice, &mut output, &context)?;
        let diag = plugin.diagnostics();

        let input_metrics = input_metrics(input_slice);
        let output_metrics =
            channel_metrics(&output, out_channels, produced, &mut prev_output_last);
        let deltas = DiagnosticDeltas::from_previous(&diag, prev_diag.as_ref());

        max_dialogue_delta = max_dialogue_delta.max(deltas.dialogue_probability_abs);
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
    println!("max_output_peak:        {max_output_peak:.6}");
    println!("max_output_step:        {max_output_step:.6}");
    println!("max_dialogue_delta:     {max_dialogue_delta:.6}");
    println!("max_height_mean_delta:  {max_height_mean_delta:.6}");
    println!("max_decorrelation_delta:{max_decorrelation_delta:.6}");

    Ok(())
}

fn parse_diagnostic_options(args: Vec<String>) -> Result<DiagnosticOptions, String> {
    let mut positional = Vec::new();
    let mut speaker_config = "5.1".to_string();
    let mut block_size = 1024usize;
    let mut fft_size = 2048usize;
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
        enable_hr_direct,
        bypass_decorrelation,
        bypass_transient_detection,
        ml_model_path,
    })
}

fn parse_usize_arg(args: &[String], index: usize, name: &str) -> Result<usize, String> {
    args.get(index)
        .ok_or_else(|| format!("{name} requires a value"))?
        .parse::<usize>()
        .map_err(|e| format!("invalid {name}: {e}"))
}

fn default_diagnostic_path(input_path: &Path) -> PathBuf {
    let stem = input_path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("upmixer");
    input_path.with_file_name(format!("{stem}.upmixer-diagnostics.csv"))
}

struct InputAudio {
    sample_rate: u32,
    samples: Vec<f32>,
}

fn load_wav_stereo(path: &Path) -> Result<InputAudio, String> {
    let mut bytes =
        std::fs::read(path).map_err(|e| format!("could not read {}: {e}", path.display()))?;
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

fn write_header(writer: &mut dyn Write, channels: usize) -> Result<(), String> {
    write!(
        writer,
        "block,start_frame,time_sec,frames_produced,input_peak,input_rms,output_peak_max,output_rms_sum,output_step_peak,dialogue_probability,dialogue_delta,dialogue_centroid_hz,dialogue_envelope_variance,decorrelation_strength,decorrelation_delta,hr_direct_envelope,hr_transient_env,height_transient_env,spectral_flux_smooth,height_spectral_flux_smooth,safety_scale,safety_delta,output_accumulator_fill,height_gain_mean,height_gain_min,height_gain_max,height_gain_stddev,height_gain_mean_delta,height_gate_mean,height_gate_min,height_gate_max,height_gate_stddev,height_gate_mean_delta,coherence_mean,coherence_min,coherence_max,coherence_stddev"
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
        "{block},{start_frame},{time_sec:.9},{frames_produced},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.3},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9},{:.9}",
        input.peak,
        input.rms,
        output.max_peak,
        output.rms_sum,
        output.step_peak,
        diag.dialogue_probability,
        deltas.dialogue_probability_abs,
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
        let ctx = ProcessContext {
            sample_rate,
            num_frames: end - pos,
        };
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
        let ctx = ProcessContext {
            sample_rate,
            num_frames: end - pos,
        };
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
