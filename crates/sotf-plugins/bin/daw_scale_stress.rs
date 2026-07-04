use clap::{Parser, ValueEnum};
use math_audio_iir_fir::{Biquad, BiquadFilterType};
use rayon::prelude::*;
use sotf_plugins::{
    AutoGain, AutoGainParams, ChannelMuteSoloPlugin, CrossfeedPlugin, CrossfeedPluginParams,
    DawHost, DelayPlugin, DenoiserPlugin, EqPlugin, GainPlugin, LimiterPlugin,
    LoudnessCompensationPlugin, MultibandCompressorPlugin, ParametricInPlacePluginAdapter,
    ParametricPluginAdapter, Plugin,
};
use std::hint::black_box;
use std::time::{Duration, Instant};

const CHANNELS: usize = 2;

#[derive(Parser, Debug)]
#[command(about = "DAW-scale SOTF plugin/host stress harness")]
struct Args {
    /// Run a short smoke workload.
    #[arg(long)]
    quick: bool,

    /// Override the track count for a single scenario.
    #[arg(long)]
    tracks: Option<usize>,

    /// Override plugins per track for a single scenario.
    #[arg(long)]
    plugins: Option<usize>,

    /// Measured callback blocks per scenario.
    #[arg(long)]
    blocks: Option<usize>,

    /// Warmup callback blocks per scenario.
    #[arg(long, default_value_t = 64)]
    warmup_blocks: usize,

    /// Audio callback block size in frames.
    #[arg(long, default_value_t = 128)]
    block_size: usize,

    /// Sample rate in Hz.
    #[arg(long, default_value_t = 48_000)]
    sample_rate: u32,

    /// Plugin chain family.
    #[arg(long, value_enum, default_value_t = ChainKind::Mixed)]
    chain: ChainKind,

    /// Execution mode.
    #[arg(long, value_enum, default_value_t = Mode::Both)]
    mode: Mode,

    /// Disable the compiled linear render plan to compare against graph fallback.
    #[arg(long)]
    disable_compiled_linear: bool,

    /// Isolate one suspected cost center instead of running full DAW stress.
    #[arg(long, value_enum, default_value_t = Focus::Stress)]
    focus: Focus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum Mode {
    Serial,
    Parallel,
    Adaptive,
    Both,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ChainKind {
    Gain,
    Eq,
    MuteSolo,
    Limiter,
    MultibandCompressor,
    Linear,
    Dynamics,
    Stft,
    Mixed,
    Heavy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum Focus {
    Stress,
    Flush,
    AutoGain,
    Scheduling,
}

#[derive(Clone, Copy, Debug)]
struct Scenario {
    name: &'static str,
    tracks: usize,
    plugins_per_track: usize,
}

struct TrackState {
    host: DawHost,
    input: Vec<f32>,
    output: Vec<f32>,
}

#[derive(Debug)]
struct RunStats {
    min_ns: u128,
    mean_ns: u128,
    p50_ns: u128,
    p95_ns: u128,
    p99_ns: u128,
    p999_ns: u128,
    max_ns: u128,
    min_us: u128,
    mean_us: u128,
    p50_us: u128,
    p95_us: u128,
    p99_us: u128,
    p999_us: u128,
    max_us: u128,
    deadline_misses: usize,
    realtime_factor_p99: f64,
    realtime_factor_max: f64,
}

fn main() -> Result<(), String> {
    let args = Args::parse();
    if args.focus != Focus::Stress {
        return run_focused(&args);
    }

    let scenarios = scenarios(&args);
    let modes = modes(args.mode);

    println!(
        "scenario,mode,chain,sample_rate,block_size,tracks,plugins_per_track,total_plugins,blocks,deadline_us,min_us,mean_us,p50_us,p95_us,p99_us,p999_us,max_us,deadline_misses,p99_realtime_factor,max_realtime_factor"
    );

    for scenario in scenarios {
        for mode in &modes {
            let stats = run_scenario(&args, scenario, *mode)?;
            let deadline_us = callback_budget(args.sample_rate, args.block_size).as_micros();
            println!(
                "{},{:?},{:?},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{:.3}",
                scenario.name,
                mode,
                args.chain,
                args.sample_rate,
                args.block_size,
                scenario.tracks,
                scenario.plugins_per_track,
                scenario.tracks * scenario.plugins_per_track,
                measured_blocks(&args),
                deadline_us,
                stats.min_us,
                stats.mean_us,
                stats.p50_us,
                stats.p95_us,
                stats.p99_us,
                stats.p999_us,
                stats.max_us,
                stats.deadline_misses,
                stats.realtime_factor_p99,
                stats.realtime_factor_max,
            );
        }
    }

    Ok(())
}

fn run_focused(args: &Args) -> Result<(), String> {
    match args.focus {
        Focus::Stress => unreachable!(),
        Focus::Flush => benchmark_flush(args),
        Focus::AutoGain => benchmark_auto_gain(args),
        Focus::Scheduling => benchmark_scheduling(args),
    }
}

fn scenarios(args: &Args) -> Vec<Scenario> {
    if let (Some(tracks), Some(plugins_per_track)) = (args.tracks, args.plugins) {
        return vec![Scenario {
            name: "custom",
            tracks,
            plugins_per_track,
        }];
    }

    if args.quick {
        return vec![
            Scenario {
                name: "quick-light",
                tracks: args.tracks.unwrap_or(16),
                plugins_per_track: args.plugins.unwrap_or(4),
            },
            Scenario {
                name: "quick-medium",
                tracks: args.tracks.unwrap_or(32),
                plugins_per_track: args.plugins.unwrap_or(8),
            },
        ];
    }

    vec![
        Scenario {
            name: "light",
            tracks: args.tracks.unwrap_or(32),
            plugins_per_track: args.plugins.unwrap_or(4),
        },
        Scenario {
            name: "medium",
            tracks: args.tracks.unwrap_or(64),
            plugins_per_track: args.plugins.unwrap_or(8),
        },
        Scenario {
            name: "large",
            tracks: args.tracks.unwrap_or(128),
            plugins_per_track: args.plugins.unwrap_or(12),
        },
        Scenario {
            name: "daw-target",
            tracks: args.tracks.unwrap_or(256),
            plugins_per_track: args.plugins.unwrap_or(16),
        },
    ]
}

fn modes(mode: Mode) -> Vec<Mode> {
    match mode {
        Mode::Serial => vec![Mode::Serial],
        Mode::Parallel => vec![Mode::Parallel],
        Mode::Adaptive => vec![Mode::Adaptive],
        Mode::Both => vec![Mode::Serial, Mode::Parallel],
    }
}

fn measured_blocks(args: &Args) -> usize {
    args.blocks
        .unwrap_or(if args.quick { 128 } else { 1_024 })
        .max(1)
}

fn run_scenario(args: &Args, scenario: Scenario, mode: Mode) -> Result<RunStats, String> {
    let mut tracks = build_tracks(args, scenario)?;
    for block in 0..args.warmup_blocks {
        process_block(args.chain, &mut tracks, args.block_size, block, mode)?;
    }

    let blocks = measured_blocks(args);
    let mut durations = Vec::with_capacity(blocks);
    for block in 0..blocks {
        let start = Instant::now();
        process_block(
            args.chain,
            &mut tracks,
            args.block_size,
            block + args.warmup_blocks,
            mode,
        )?;
        durations.push(start.elapsed());
    }

    Ok(summarize(
        &mut durations,
        callback_budget(args.sample_rate, args.block_size),
    ))
}

fn build_tracks(args: &Args, scenario: Scenario) -> Result<Vec<TrackState>, String> {
    let samples = args.block_size * CHANNELS;
    (0..scenario.tracks)
        .map(|track| {
            let mut host = DawHost::new(CHANNELS, args.sample_rate);
            host.set_compiled_linear_enabled(!args.disable_compiled_linear);
            for plugin_index in 0..scenario.plugins_per_track {
                host.add_plugin(make_plugin(
                    args.chain,
                    CHANNELS,
                    args.sample_rate,
                    plugin_index,
                )?)?;
            }

            let input = generated_input(samples, track);
            let output = vec![0.0; samples];
            Ok(TrackState {
                host,
                input,
                output,
            })
        })
        .collect()
}

fn make_plugin(
    chain: ChainKind,
    channels: usize,
    sample_rate: u32,
    plugin_index: usize,
) -> Result<Box<dyn Plugin>, String> {
    match chain {
        ChainKind::Gain => make_gain(channels, plugin_index),
        ChainKind::Eq => make_eq(channels, sample_rate, plugin_index),
        ChainKind::MuteSolo => make_mute_solo(channels, plugin_index),
        ChainKind::Limiter => make_limiter(channels, 0.0),
        ChainKind::MultibandCompressor => make_multiband_compressor(channels),
        ChainKind::Linear => match plugin_index % 5 {
            0 | 4 => make_gain(channels, plugin_index),
            1 => make_crossfeed(),
            2 => make_eq(channels, sample_rate, plugin_index),
            _ => make_loudness(channels),
        },
        ChainKind::Dynamics => match plugin_index % 3 {
            0 => make_eq(channels, sample_rate, plugin_index),
            1 => make_multiband_compressor(channels),
            _ => make_limiter(channels, 0.0),
        },
        ChainKind::Stft => make_denoiser(channels, plugin_index),
        ChainKind::Mixed => match plugin_index % 4 {
            0 => make_gain(channels, plugin_index),
            1 => make_eq(channels, sample_rate, plugin_index),
            2 => make_delay(channels, plugin_index),
            _ => make_limiter(channels, 5.0),
        },
        ChainKind::Heavy => match plugin_index % 5 {
            0 | 3 => make_eq(channels, sample_rate, plugin_index),
            1 => make_delay(channels, plugin_index),
            2 => make_limiter(channels, 5.0),
            _ => make_multiband_compressor(channels),
        },
    }
}

fn make_mute_solo(channels: usize, plugin_index: usize) -> Result<Box<dyn Plugin>, String> {
    let mut plugin = ChannelMuteSoloPlugin::new(channels, true);
    match plugin_index % 3 {
        0 => {}
        1 => {
            plugin.set_channel_state(0, false, false, true)?;
        }
        _ => {
            plugin.set_channel_state(1.min(channels - 1), true, false, false)?;
        }
    }
    Ok(Box::new(ParametricInPlacePluginAdapter::new(plugin)))
}

fn make_gain(channels: usize, plugin_index: usize) -> Result<Box<dyn Plugin>, String> {
    let gain_db = match plugin_index % 3 {
        0 => -1.5,
        1 => 0.0,
        _ => 1.5,
    };
    Ok(Box::new(ParametricPluginAdapter::new(GainPlugin::new(
        channels, gain_db,
    ))))
}

fn make_eq(
    channels: usize,
    sample_rate: u32,
    plugin_index: usize,
) -> Result<Box<dyn Plugin>, String> {
    let sr = sample_rate as f64;
    let offset = plugin_index as f64 * 17.0;
    let filters = vec![
        Biquad::new(BiquadFilterType::Highpass, 30.0, sr, 0.707, 0.0),
        Biquad::new(BiquadFilterType::Lowshelf, 120.0 + offset, sr, 0.707, 1.5),
        Biquad::new(BiquadFilterType::Peak, 420.0 + offset, sr, 1.0, -1.5),
        Biquad::new(BiquadFilterType::Peak, 1_800.0 + offset, sr, 1.8, 1.0),
        Biquad::new(BiquadFilterType::Peak, 4_200.0 + offset, sr, 1.4, -1.0),
        Biquad::new(BiquadFilterType::Highshelf, 10_000.0, sr, 0.707, 1.0),
    ];
    Ok(Box::new(ParametricPluginAdapter::new(EqPlugin::new(
        channels, filters,
    ))))
}

fn make_crossfeed() -> Result<Box<dyn Plugin>, String> {
    let params = CrossfeedPluginParams {
        autogain_enabled: false,
        ..Default::default()
    };
    Ok(Box::new(ParametricInPlacePluginAdapter::new(
        CrossfeedPlugin::new(params)?,
    )))
}

fn make_loudness(channels: usize) -> Result<Box<dyn Plugin>, String> {
    Ok(Box::new(ParametricInPlacePluginAdapter::new(
        LoudnessCompensationPlugin::new(channels, 120.0, 1.0, 10_000.0, 1.0),
    )))
}

fn make_delay(channels: usize, plugin_index: usize) -> Result<Box<dyn Plugin>, String> {
    let delay_ms = 8.0 + (plugin_index % 7) as f32 * 3.0;
    Ok(Box::new(ParametricInPlacePluginAdapter::new(
        DelayPlugin::new(channels, delay_ms, 0.18, 0.22),
    )))
}

fn make_limiter(channels: usize, lookahead_ms: f32) -> Result<Box<dyn Plugin>, String> {
    Ok(Box::new(ParametricInPlacePluginAdapter::new(
        LimiterPlugin::new(channels, -1.0, 50.0, lookahead_ms, false),
    )))
}

fn make_multiband_compressor(channels: usize) -> Result<Box<dyn Plugin>, String> {
    Ok(Box::new(ParametricInPlacePluginAdapter::new(
        MultibandCompressorPlugin::new(channels),
    )))
}

fn make_denoiser(channels: usize, plugin_index: usize) -> Result<Box<dyn Plugin>, String> {
    Ok(Box::new(ParametricInPlacePluginAdapter::new(
        DenoiserPlugin::new(channels, plugin_index.is_multiple_of(2)),
    )))
}

fn generated_input(samples: usize, track: usize) -> Vec<f32> {
    let phase = track as f32 * 0.013;
    (0..samples)
        .map(|sample| {
            let t = sample as f32 * 0.017 + phase;
            (t.sin() * 0.25) + ((t * 0.37).cos() * 0.05)
        })
        .collect()
}

fn process_block(
    chain: ChainKind,
    tracks: &mut [TrackState],
    block_size: usize,
    block_index: usize,
    mode: Mode,
) -> Result<(), String> {
    match mode {
        Mode::Serial | Mode::Both => tracks
            .iter_mut()
            .try_for_each(|track| process_track(track, block_size, block_index)),
        Mode::Parallel => tracks
            .par_iter_mut()
            .try_for_each(|track| process_track(track, block_size, block_index)),
        Mode::Adaptive => {
            if should_parallelize_tracks(
                chain,
                tracks.len(),
                tracks.first().map_or(0, |track| track.host.plugin_count()),
                block_size,
            ) {
                tracks
                    .par_iter_mut()
                    .try_for_each(|track| process_track(track, block_size, block_index))
            } else {
                tracks
                    .iter_mut()
                    .try_for_each(|track| process_track(track, block_size, block_index))
            }
        }
    }
}

fn should_parallelize_tracks(
    chain: ChainKind,
    tracks: usize,
    plugins_per_track: usize,
    block_size: usize,
) -> bool {
    let work_units = tracks
        .saturating_mul(plugins_per_track.max(1))
        .saturating_mul(block_size)
        .saturating_mul(chain_scheduler_cost(chain));
    work_units >= 64 * 4 * 128 * 2
}

fn chain_scheduler_cost(chain: ChainKind) -> usize {
    match chain {
        ChainKind::Gain => 1,
        ChainKind::Eq => 4,
        ChainKind::MuteSolo => 2,
        ChainKind::Limiter => 3,
        ChainKind::MultibandCompressor => 8,
        ChainKind::Linear => 4,
        ChainKind::Dynamics => 7,
        ChainKind::Stft => 12,
        ChainKind::Mixed => 6,
        ChainKind::Heavy => 10,
    }
}

fn process_track(
    track: &mut TrackState,
    block_size: usize,
    block_index: usize,
) -> Result<(), String> {
    rotate_input(&mut track.input, block_index);
    let frames = black_box(&mut track.host)
        .process(black_box(&track.input), black_box(&mut track.output))?;
    if frames != block_size {
        return Err(format!(
            "host returned {frames} frames for a {block_size}-frame stress block"
        ));
    }
    black_box(track.output[block_index % track.output.len()]);
    Ok(())
}

fn rotate_input(input: &mut [f32], block_index: usize) {
    if input.is_empty() {
        return;
    }
    let idx = block_index % input.len();
    input[idx] = -input[idx];
}

fn callback_budget(sample_rate: u32, block_size: usize) -> Duration {
    Duration::from_secs_f64(block_size as f64 / sample_rate as f64)
}

fn summarize(durations: &mut [Duration], budget: Duration) -> RunStats {
    durations.sort_unstable();
    let total_ns: u128 = durations.iter().map(|d| d.as_nanos()).sum();
    let total_us: u128 = durations.iter().map(|d| d.as_micros()).sum();
    let mean_ns = total_ns / durations.len() as u128;
    let mean_us = total_us / durations.len() as u128;
    let p50 = percentile(durations, 0.50);
    let p95 = percentile(durations, 0.95);
    let p99 = percentile(durations, 0.99);
    let p999 = percentile(durations, 0.999);
    let max = durations[durations.len() - 1];
    RunStats {
        min_ns: durations[0].as_nanos(),
        mean_ns,
        p50_ns: p50.as_nanos(),
        p95_ns: p95.as_nanos(),
        p99_ns: p99.as_nanos(),
        p999_ns: p999.as_nanos(),
        max_ns: max.as_nanos(),
        min_us: durations[0].as_micros(),
        mean_us,
        p50_us: p50.as_micros(),
        p95_us: p95.as_micros(),
        p99_us: p99.as_micros(),
        p999_us: p999.as_micros(),
        max_us: max.as_micros(),
        deadline_misses: durations
            .iter()
            .filter(|duration| **duration > budget)
            .count(),
        realtime_factor_p99: p99.as_secs_f64() / budget.as_secs_f64(),
        realtime_factor_max: max.as_secs_f64() / budget.as_secs_f64(),
    }
}

fn print_focus_header() {
    println!(
        "focus,case,sample_rate,block_size,channels,tracks,plugins_per_track,iterations,min_ns,mean_ns,p50_ns,p95_ns,p99_ns,p999_ns,max_ns,min_us,mean_us,p50_us,p95_us,p99_us,p999_us,max_us"
    );
}

#[allow(
    clippy::too_many_arguments,
    reason = "CSV row helper: one argument per output column"
)]
fn print_focus_row(
    focus: Focus,
    case_name: &str,
    args: &Args,
    channels: usize,
    tracks: usize,
    plugins_per_track: usize,
    iterations: usize,
    stats: &RunStats,
) {
    println!(
        "{:?},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{}",
        focus,
        case_name,
        args.sample_rate,
        args.block_size,
        channels,
        tracks,
        plugins_per_track,
        iterations,
        stats.min_ns,
        stats.mean_ns,
        stats.p50_ns,
        stats.p95_ns,
        stats.p99_ns,
        stats.p999_ns,
        stats.max_ns,
        stats.min_us,
        stats.mean_us,
        stats.p50_us,
        stats.p95_us,
        stats.p99_us,
        stats.p999_us,
        stats.max_us,
    );
}

fn benchmark_flush(args: &Args) -> Result<(), String> {
    let iterations = measured_blocks(args);
    let samples = args.block_size * CHANNELS;
    let mut normal = generated_input(samples, 0);
    let mut tiny = vec![f32::MIN_POSITIVE * 0.25; samples];
    let mut zero = vec![0.0; samples];

    let cases = [
        ("normal", &mut normal),
        ("subnormal", &mut tiny),
        ("zero", &mut zero),
    ];

    print_focus_header();
    for (case_name, buffer) in cases {
        let mut durations = Vec::with_capacity(iterations);
        for iter in 0..iterations {
            let index = iter % buffer.len();
            buffer[index] = -buffer[index];
            let start = Instant::now();
            sotf_host::simd::flush_denormals_inplace(black_box(buffer));
            durations.push(start.elapsed());
        }
        let stats = summarize(&mut durations, Duration::MAX);
        print_focus_row(
            Focus::Flush,
            case_name,
            args,
            CHANNELS,
            1,
            0,
            iterations,
            &stats,
        );
    }
    Ok(())
}

fn benchmark_auto_gain(args: &Args) -> Result<(), String> {
    let iterations = measured_blocks(args);
    let samples = args.block_size * CHANNELS;
    let input = generated_input(samples, 0);
    let mut output = input.clone();
    let enabled = AutoGainParams {
        enabled: true,
        ..AutoGainParams::default()
    };

    print_focus_header();
    for (case_name, params) in [
        ("measure_disabled", AutoGainParams::default()),
        ("measure_enabled", enabled.clone()),
        ("apply_disabled", AutoGainParams::default()),
        ("apply_enabled_stable", enabled),
    ] {
        let mut auto_gain = AutoGain::new(CHANNELS, args.sample_rate, params)?;
        if case_name == "apply_enabled_stable" {
            auto_gain.measure_input(&input)?;
            auto_gain.measure_output(&output)?;
        }

        let mut durations = Vec::with_capacity(iterations);
        for iter in 0..iterations {
            let index = iter % output.len();
            output[index] = -output[index];
            let start = Instant::now();
            match case_name {
                "measure_disabled" | "measure_enabled" => {
                    black_box(&mut auto_gain).measure_input(black_box(&input))?;
                    black_box(&mut auto_gain).measure_output(black_box(&output))?;
                }
                "apply_disabled" | "apply_enabled_stable" => {
                    black_box(&mut auto_gain)
                        .apply_compensation(black_box(&mut output), args.block_size);
                }
                _ => unreachable!(),
            }
            durations.push(start.elapsed());
        }
        let stats = summarize(&mut durations, Duration::MAX);
        print_focus_row(
            Focus::AutoGain,
            case_name,
            args,
            CHANNELS,
            1,
            0,
            iterations,
            &stats,
        );
    }
    Ok(())
}

fn benchmark_scheduling(args: &Args) -> Result<(), String> {
    let iterations = measured_blocks(args);
    let tracks = args.tracks.unwrap_or(64);
    let samples = args.block_size * CHANNELS;
    let input = generated_input(samples, 0);
    let output = vec![0.0; samples];

    print_focus_header();
    for plugins_per_track in [0, 1, 4, 8] {
        let modes = modes(args.mode);
        for mode in modes {
            let case_name = match mode {
                Mode::Serial | Mode::Both => format!("serial_{}plugins", plugins_per_track),
                Mode::Parallel => format!("parallel_{}plugins", plugins_per_track),
                Mode::Adaptive => format!("adaptive_{}plugins", plugins_per_track),
            };
            let mut states = (0..tracks)
                .map(|_| {
                    let mut host = DawHost::new(CHANNELS, args.sample_rate);
                    host.set_compiled_linear_enabled(!args.disable_compiled_linear);
                    for plugin_index in 0..plugins_per_track {
                        host.add_plugin(make_gain(CHANNELS, plugin_index)?)?;
                    }
                    Ok(TrackState {
                        host,
                        input: input.clone(),
                        output: output.clone(),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;

            let mut durations = Vec::with_capacity(iterations);
            for iter in 0..iterations {
                let start = Instant::now();
                process_block(ChainKind::Gain, &mut states, args.block_size, iter, mode)?;
                durations.push(start.elapsed());
            }
            let stats = summarize(&mut durations, Duration::MAX);
            print_focus_row(
                Focus::Scheduling,
                &case_name,
                args,
                CHANNELS,
                tracks,
                plugins_per_track,
                iterations,
                &stats,
            );
        }
    }
    Ok(())
}

fn percentile(sorted: &[Duration], percentile: f64) -> Duration {
    debug_assert!(!sorted.is_empty());
    let index = ((sorted.len() - 1) as f64 * percentile).ceil() as usize;
    sorted[index]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentile_picks_tail_values() {
        let mut durations = [
            Duration::from_micros(30),
            Duration::from_micros(10),
            Duration::from_micros(20),
            Duration::from_micros(40),
        ];
        durations.sort_unstable();

        assert_eq!(percentile(&durations, 0.50), Duration::from_micros(30));
        assert_eq!(percentile(&durations, 0.99), Duration::from_micros(40));
    }

    #[test]
    fn summarize_counts_deadline_misses() {
        let mut durations = [
            Duration::from_micros(50),
            Duration::from_micros(200),
            Duration::from_micros(100),
        ];
        let stats = summarize(&mut durations, Duration::from_micros(100));

        assert_eq!(stats.deadline_misses, 1);
        assert_eq!(stats.max_us, 200);
    }
}
