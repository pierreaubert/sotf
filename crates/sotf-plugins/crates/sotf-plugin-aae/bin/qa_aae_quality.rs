//! Deterministic offline acoustic-quality measurements for AAE.
//!
//! This is intentionally separate from callback timing/allocation QA. It renders
//! complete signals, allocates analysis storage, and prints tab-separated records
//! suitable for comparing releases in CI.

use sotf_host::speaker_config::get_speaker_config;
use sotf_host::{Plugin, ProcessContext};
use sotf_plugin_aae::quality::{
    confusion_matrix, distortion_metrics, echo_density, gain_pumping, modulation_sideband_db,
    octave_band, schroeder_rt60, spatial_metrics, transfer_at_frequency,
};
use sotf_plugin_aae::{AaeData, AaePlugin, params::AaePluginParams};
use std::f32::consts::TAU;

#[derive(Clone, Copy)]
struct MatrixCase {
    preset: &'static str,
    layout: &'static str,
    sample_rate: u32,
    partition: usize,
}

const MATRIX: &[MatrixCase] = &[
    MatrixCase {
        preset: "small",
        layout: "5.1",
        sample_rate: 44_100,
        partition: 64,
    },
    MatrixCase {
        preset: "medium",
        layout: "9.1.6",
        sample_rate: 48_000,
        partition: 257,
    },
    MatrixCase {
        preset: "large",
        layout: "5.1",
        sample_rate: 48_000,
        partition: 1_024,
    },
    MatrixCase {
        preset: "cathedral",
        layout: "9.1.6",
        sample_rate: 44_100,
        partition: 257,
    },
];

fn main() {
    println!("metric\tpreset\tlayout\trate\tpartition\tdetail\tvalue");
    for case in MATRIX {
        measure_room(*case);
    }
    measure_lfe_crossover();
    measure_modulation_and_distortion();
    measure_detector();
    println!("AAE offline acoustic-quality QA: PASS");
    println!(
        "External listening/corpus validation is not simulated; run the documented protocol in quality-validation.md"
    );
}

fn wet_params(case: MatrixCase) -> AaePluginParams {
    AaePluginParams {
        speaker_config: case.layout.into(),
        room_preset: case.preset.into(),
        dry_level: 0.0,
        er_level: 1.0,
        late_level: 1.0,
        lfe_level: 0.0,
        pre_delay_ms: 0.0,
        content_aware: false,
        auto_gain_enabled: false,
        ..Default::default()
    }
}

fn render(params: AaePluginParams, sample_rate: u32, partition: usize, input: &[f32]) -> Vec<f32> {
    let mut plugin = AaePlugin::try_from_params(params).unwrap();
    plugin.initialize(sample_rate).unwrap();
    let channels = plugin.output_channels();
    let frames = input.len() / 2;
    let mut output = vec![0.0; frames * channels];
    let mut position = 0;
    while position < frames {
        let end = (position + partition).min(frames);
        plugin
            .process(
                &input[position * 2..end * 2],
                &mut output[position * channels..end * channels],
                &ProcessContext::new(sample_rate, end - position),
            )
            .unwrap();
        position = end;
    }
    output
}

fn channel(interleaved: &[f32], channels: usize, channel: usize) -> Vec<f32> {
    interleaved
        .chunks_exact(channels)
        .map(|frame| frame[channel])
        .collect()
}

fn measure_room(case: MatrixCase) {
    let frames = case.sample_rate as usize * 3;
    let mut impulse = vec![0.0_f32; frames * 2];
    impulse[0] = 0.5;
    impulse[1] = 0.5;
    let rendered = render(wet_params(case), case.sample_rate, case.partition, &impulse);
    let config = get_speaker_config(case.layout).unwrap();
    let response = channel(&rendered, config.total_channels, 0);
    for center in [250.0, 1_000.0, 4_000.0] {
        let band = octave_band(&response, case.sample_rate, center);
        let estimate = schroeder_rt60(&band, case.sample_rate, -5.0, -25.0)
            .unwrap_or_else(|| panic!("missing {center} Hz decay estimate for {}", case.preset));
        assert!(
            (0.15..=8.0).contains(&estimate.rt60_seconds),
            "implausible decay estimate: {case_desc} {center} Hz {estimate:?}",
            case_desc = case.preset
        );
        assert!(
            estimate.r_squared > 0.25,
            "unstable decay regression: {estimate:?}"
        );
        println!(
            "rt60\t{}\t{}\t{}\t{}\t{}Hz,r2={:.3}\t{:.4}",
            case.preset,
            case.layout,
            case.sample_rate,
            case.partition,
            center,
            estimate.r_squared,
            estimate.rt60_seconds
        );
    }

    let density = echo_density(
        &response,
        case.sample_rate,
        (case.sample_rate as usize / 100).max(64),
        (case.sample_rate as usize / 400).max(16),
    );
    assert!(
        density.peak_normalized_density > 0.25,
        "echo density never builds: {density:?}"
    );
    println!(
        "echo_density\t{}\t{}\t{}\t{}\tmixing_time_s={:?}\t{:.4}",
        case.preset,
        case.layout,
        case.sample_rate,
        case.partition,
        density.mixing_time_seconds,
        density.peak_normalized_density
    );

    let directions: Vec<Option<[f64; 3]>> = config
        .speakers
        .iter()
        .map(|speaker| {
            if speaker.is_lfe {
                None
            } else {
                let azimuth = f64::from(speaker.azimuth).to_radians();
                let elevation = f64::from(speaker.elevation).to_radians();
                Some([
                    elevation.cos() * azimuth.cos(),
                    elevation.cos() * azimuth.sin(),
                    elevation.sin(),
                ])
            }
        })
        .collect();
    let late_start = case.sample_rate as usize / 4 * config.total_channels;
    let spatial =
        spatial_metrics(&rendered[late_start..], config.total_channels, &directions).unwrap();
    assert!(
        spatial.normalized_energy_entropy > 0.35,
        "spatial collapse: {spatial:?}"
    );
    assert!(
        spatial.mean_absolute_coherence < 0.95,
        "fully coherent tail: {spatial:?}"
    );
    assert!(spatial.energy_vector_magnitude <= 1.000_001);
    println!(
        "spatial\t{}\t{}\t{}\t{}\tentropy={:.4},coherence={:.4},vector={:.4}\t{:.4}",
        case.preset,
        case.layout,
        case.sample_rate,
        case.partition,
        spatial.normalized_energy_entropy,
        spatial.mean_absolute_coherence,
        spatial.energy_vector_magnitude,
        spatial.diffuseness
    );
}

fn steady_sine(frequency: f32, amplitude: f32, sample_rate: u32, seconds: usize) -> Vec<f32> {
    (0..sample_rate as usize * seconds)
        .flat_map(|index| {
            let sample = amplitude * (TAU * frequency * index as f32 / sample_rate as f32).sin();
            [sample, sample]
        })
        .collect()
}

fn measure_lfe_crossover() {
    let sample_rate = 48_000;
    let mut responses = Vec::new();
    for frequency in [60.0_f32, 120.0, 250.0, 1_000.0] {
        let input = steady_sine(frequency, 0.2, sample_rate, 2);
        let params = AaePluginParams {
            speaker_config: "5.1".into(),
            dry_level: 0.0,
            er_level: 1.0,
            late_level: 1.0,
            lfe_level: 1.0,
            content_aware: false,
            mod_depth: 0.0,
            er_mod_depth: 0.0,
            ..Default::default()
        };
        let rendered = render(params, sample_rate, 257, &input);
        let start = sample_rate as usize;
        let mono_input = channel(&input[start * 2..], 2, 0);
        let lfe = channel(&rendered[start * 6..], 6, 3);
        let transfer =
            transfer_at_frequency(&mono_input, &lfe, sample_rate, f64::from(frequency)).unwrap();
        assert!(transfer.gain_db.is_finite() && transfer.phase_degrees.is_finite());
        responses.push((frequency, transfer.gain_db));
        println!(
            "lfe_transfer\tmedium\t5.1\t48000\t257\t{}Hz,phase_deg={:.3}\t{:.4}",
            frequency, transfer.phase_degrees, transfer.gain_db
        );
    }
    let gain_60 = responses[0].1;
    let gain_1000 = responses[3].1;
    assert!(
        gain_1000 < gain_60 - 25.0,
        "LR4 LFE rejection missing: {responses:?}"
    );
}

fn measure_modulation_and_distortion() {
    let sample_rate = 48_000;
    let input = steady_sine(1_000.0, 0.2, sample_rate, 4);
    let params = AaePluginParams {
        dry_level: 0.0,
        er_level: 0.0,
        late_level: 1.0,
        lfe_level: 0.0,
        content_aware: false,
        mod_depth: 1.0,
        ..Default::default()
    };
    let rendered = render(params, sample_rate, 257, &input);
    let output = channel(&rendered[sample_rate as usize * 6..], 6, 0);
    let sideband = modulation_sideband_db(
        &output,
        sample_rate,
        1_000.0,
        &[0.25, 0.5, 0.75, 1.0, 1.5, 2.0],
    )
    .unwrap();
    assert!(sideband.is_finite() && sideband < 6.0);
    println!("modulation_sideband\tmedium\t5.1\t48000\t257\tmax_0.25-2Hz\t{sideband:.4}");

    let distortion = distortion_metrics(&output, sample_rate, 1_000.0, None).unwrap();
    assert!(
        distortion.thd_db < 0.0,
        "path dominated by harmonics: {distortion:?}"
    );
    println!(
        "thd\tmedium\t5.1\t48000\t257\t1kHz\t{:.4}",
        distortion.thd_db
    );

    let two_tone: Vec<f32> = (0..sample_rate as usize * 2)
        .flat_map(|index| {
            let time = index as f64 / sample_rate as f64;
            let sample =
                (0.1 * ((std::f64::consts::TAU * 700.0 * time).sin()
                    + (std::f64::consts::TAU * 1_200.0 * time).sin())) as f32;
            [sample, sample]
        })
        .collect();
    let rendered = render(
        AaePluginParams {
            dry_level: 1.0,
            er_level: 0.0,
            late_level: 0.0,
            lfe_level: 0.0,
            content_aware: false,
            ..Default::default()
        },
        sample_rate,
        257,
        &two_tone,
    );
    let output = channel(&rendered[sample_rate as usize * 6..], 6, 0);
    let distortion = distortion_metrics(&output, sample_rate, 700.0, Some(1_200.0)).unwrap();
    assert!(
        distortion.imd_db < -60.0,
        "linear direct path IMD regression: {distortion:?}"
    );
    println!(
        "imd\tmedium\t5.1\t48000\t257\t700+1200Hz\t{:.4}",
        distortion.imd_db
    );

    let hot = steady_sine(1_000.0, 2.0, sample_rate, 1);
    let mut plugin = AaePlugin::try_from_params(AaePluginParams {
        dry_level: 1.0,
        er_level: 1.0,
        late_level: 1.0,
        lfe_level: 1.0,
        content_aware: false,
        ..Default::default()
    })
    .unwrap();
    plugin.initialize(sample_rate).unwrap();
    let mut output = vec![0.0; sample_rate as usize * plugin.output_channels()];
    plugin
        .process(
            &hot,
            &mut output,
            &ProcessContext::new(sample_rate, sample_rate as usize),
        )
        .unwrap();
    let data = plugin.get_data().unwrap();
    let data = data.downcast_ref::<AaeData>().unwrap();
    assert!(
        data.output_limiter_gain < 1.0,
        "hot render did not exercise limiter"
    );
    assert!(output.iter().all(|sample| sample.abs() <= 1.000_001));
    println!(
        "limiter_activity\tmedium\t5.1\t48000\t48000\tfinal_gain\t{:.6}",
        data.output_limiter_gain
    );
}

fn detector_fixture(kind: usize, sample_rate: u32) -> Vec<f32> {
    (0..sample_rate as usize)
        .flat_map(|index| {
            let time = index as f32 / sample_rate as f32;
            let voiced = (TAU * 180.0 * time).sin() * (0.55 + 0.35 * (TAU * 4.0 * time).sin());
            match kind {
                0 => [0.25 * voiced, 0.25 * voiced],
                1 => [0.30 * voiced, 0.12 * voiced],
                2 => {
                    let tone = 0.2 * (TAU * 440.0 * time).sin();
                    [tone, tone]
                }
                3 => {
                    let hit = if index % (sample_rate as usize / 8) < 8 {
                        0.8
                    } else {
                        0.0
                    };
                    [hit, hit]
                }
                4 => [0.25 * voiced, -0.25 * voiced],
                _ => [
                    0.2 * (TAU * 311.0 * time).sin(),
                    0.2 * (TAU * 487.0 * time).sin(),
                ],
            }
        })
        .collect()
}

fn measure_detector() {
    let sample_rate = 48_000;
    let expected = [true, true, false, false, false, false];
    let mut observed = Vec::new();
    let mut gains = Vec::new();
    for fixture in 0..expected.len() {
        let input = detector_fixture(fixture, sample_rate);
        let mut plugin = AaePlugin::try_from_params(AaePluginParams {
            dry_level: 0.0,
            er_level: 1.0,
            late_level: 1.0,
            lfe_level: 0.0,
            content_aware: true,
            dialogue_attenuation_db: 6.0,
            ..Default::default()
        })
        .unwrap();
        plugin.initialize(sample_rate).unwrap();
        let block = sample_rate as usize / 20;
        let mut any_active = false;
        let mut fixture_gains = Vec::new();
        for chunk in input.chunks_exact(block * 2) {
            let mut output = vec![0.0; block * plugin.output_channels()];
            plugin
                .process(chunk, &mut output, &ProcessContext::new(sample_rate, block))
                .unwrap();
            let data = plugin.get_data().unwrap();
            let data = data.downcast_ref::<AaeData>().unwrap();
            any_active |= data.dialogue_active;
            fixture_gains.push(data.dialogue_duck_gain);
        }
        observed.push(any_active);
        gains.extend(fixture_gains);
    }
    let matrix = confusion_matrix(&expected, &observed).unwrap();
    // This deterministic synthetic baseline deliberately includes centered
    // steady music/percussion that the lightweight detector can confuse with
    // speech. Corpus/listening validation is an external gate; the 0.5 floor
    // prevents silent regression while leaving that known limitation visible.
    assert!(
        matrix.precision() >= 0.5,
        "detector precision regression: {matrix:?}"
    );
    assert!(
        matrix.recall() >= 0.8,
        "detector recall regression: {matrix:?}"
    );
    let (total_variation, maximum_step) = gain_pumping(&gains);
    assert!(
        maximum_step < 6.1,
        "abrupt detector gain step: {maximum_step} dB"
    );
    println!(
        "detector\tmatrix\tmatrix\t48000\t2400\tprecision={:.3},recall={:.3},pump_total_db={:.3},pump_max_db={:.3}\t{}",
        matrix.precision(),
        matrix.recall(),
        total_variation,
        maximum_step,
        matrix.true_positive + matrix.true_negative
    );
}
