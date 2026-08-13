use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;
use sotf_host::{CountingAlloc, assert_no_allocs};
use sotf_plugin_speech_denoiser::{RNNOISE_BAND_COUNT, SpeechDenoiserData, SpeechDenoiserPlugin};
use std::time::{Duration, Instant};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    for channels in [1, 2] {
        run_layout(channels);
    }
}

fn run_layout(channels: usize) {
    let mut plugin = SpeechDenoiserPlugin::new(channels);
    plugin.initialize(48_000).unwrap();
    let max_frames = 4_093;
    let mut buffer = vec![0.0; max_frames * channels];
    for frame in 0..max_frames {
        let sample = (2.0 * std::f32::consts::PI * 1_700.0 * frame as f32 / 48_000.0).sin()
            * (0.2 + 0.15 * (frame as f32 * 0.013).sin());
        for channel in 0..channels {
            buffer[frame * channels + channel] = if channel == 0 { sample } else { -0.7 * sample };
        }
    }

    let cold_context = ProcessContext::new(48_000, 512);
    assert_no_allocs("Speech Denoiser cold callback", || {
        plugin
            .process_in_place(&mut buffer[..512 * channels], &cold_context)
            .unwrap();
    });
    for frames in [1, 31, 127, 480, 512, 1_024, 4_093] {
        let context = ProcessContext::new(48_000, frames);
        assert_eq!(
            plugin
                .process_in_place(&mut buffer[..frames * channels], &context)
                .unwrap(),
            frames
        );
    }

    let frames = 512;
    let context = ProcessContext::new(48_000, frames);
    let mut timings = Vec::with_capacity(200);
    for iteration in 0..200 {
        if iteration % 25 == 0 {
            plugin
                .set_parameter(
                    ParameterId::from("enabled"),
                    ParameterValue::Bool((iteration / 25) % 2 == 0),
                )
                .unwrap();
        }
        let start = Instant::now();
        plugin
            .process_in_place(&mut buffer[..frames * channels], &context)
            .unwrap();
        timings.push(start.elapsed());
    }
    timings.sort_unstable();
    let percentile = |percent: usize| timings[(timings.len() - 1) * percent / 100];
    let deadline = Duration::from_secs_f64(frames as f64 / 48_000.0);
    let max = *timings.last().unwrap();
    assert!(
        max < deadline,
        "{channels}ch callback missed deadline: {max:?}"
    );
    assert!(buffer.iter().all(|sample| sample.is_finite()));
    let analyzer = plugin
        .get_data()
        .unwrap()
        .downcast::<SpeechDenoiserData>()
        .unwrap();
    assert!(analyzer.model_frames > 0);
    assert_eq!(analyzer.band_gains.len(), RNNOISE_BAND_COUNT);
    assert!((0.0..=1.0).contains(&analyzer.vad_probability));
    assert!(
        analyzer
            .band_gains
            .iter()
            .all(|gain| gain.is_finite() && (0.0..=1.0).contains(gain))
    );
    println!(
        "Speech Denoiser {channels}ch: p50={:?}, p95={:?}, p99={:?}, max={max:?}, zero cold allocations",
        percentile(50),
        percentile(95),
        percentile(99),
    );
}
