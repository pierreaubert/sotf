use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_plugin_band_merge::BandMergePlugin;
use sotf_plugin_band_split::BandSplitPlugin;

fn frequencies(bands: usize) -> &'static [f64] {
    match bands {
        2 => &[1_000.0],
        3 => &[400.0, 4_000.0],
        4 => &[200.0, 1_200.0, 6_000.0],
        _ => unreachable!(),
    }
}

fn render_pair(
    sample_rate: u32,
    channels: usize,
    bands: usize,
    kind: &str,
    input: &[f32],
    partitions: &[usize],
) -> Vec<f32> {
    let frames = input.len() / channels;
    let mut split = BandSplitPlugin::new_multiband(channels, frequencies(bands), kind).unwrap();
    let mut merge = BandMergePlugin::new(channels, bands).unwrap();
    split.initialize(sample_rate).unwrap();
    merge.initialize(sample_rate).unwrap();
    let mut output = vec![0.0; input.len()];
    let mut frame = 0;
    let mut partition = 0;
    while frame < frames {
        let count = partitions[partition % partitions.len()].min(frames - frame);
        let input_start = frame * channels;
        let input_end = (frame + count) * channels;
        let mut split_block = vec![0.0; count * channels * bands];
        split
            .process(
                &input[input_start..input_end],
                &mut split_block,
                &ProcessContext::new(sample_rate, count),
            )
            .unwrap();
        merge
            .process(
                &split_block,
                &mut output[input_start..input_end],
                &ProcessContext::new(sample_rate, count),
            )
            .unwrap();
        frame += count;
        partition += 1;
    }
    output
}

fn transfer_at(signal: &[f32], input: &[f32], sample_rate: u32, frequency: f32) -> (f32, f32, f32) {
    let start = signal.len() / 2;
    let mut in_re = 0.0_f64;
    let mut in_im = 0.0_f64;
    let mut out_re = 0.0_f64;
    let mut out_im = 0.0_f64;
    let mut out_energy = 0.0_f64;
    for frame in start..signal.len() {
        let phase = std::f64::consts::TAU * frequency as f64 * frame as f64 / sample_rate as f64;
        let c = phase.cos();
        let s = phase.sin();
        in_re += input[frame] as f64 * c;
        in_im -= input[frame] as f64 * s;
        out_re += signal[frame] as f64 * c;
        out_im -= signal[frame] as f64 * s;
        out_energy += (signal[frame] as f64).powi(2);
    }
    let input_magnitude = in_re.hypot(in_im);
    let output_magnitude = out_re.hypot(out_im);
    let gain_db = 20.0 * (output_magnitude / input_magnitude).log10();
    let phase = (out_im.atan2(out_re) - in_im.atan2(in_re)) as f32;

    // A settled LTI crossover output must be explained by one sinusoid at the
    // excitation frequency. This rejects modulation/nonlinear residue while
    // allowing the documented Linkwitz-Riley phase rotation.
    let count = (signal.len() - start) as f64;
    let amplitude = 2.0 * output_magnitude / count;
    let output_phase = out_im.atan2(out_re);
    let residual = (start..signal.len())
        .map(|frame| {
            let angle =
                std::f64::consts::TAU * frequency as f64 * frame as f64 / sample_rate as f64;
            let predicted = amplitude * (angle + output_phase).cos();
            let error = signal[frame] as f64 - predicted;
            error * error
        })
        .sum::<f64>();
    let correlation = (1.0 - residual / out_energy.max(1.0e-30)).clamp(-1.0, 1.0) as f32;
    (gain_db as f32, phase, correlation)
}

#[test]
fn paired_split_merge_is_partition_invariant_for_impulses_and_noise() {
    for sample_rate in [32_000, 48_000, 96_000, 192_000] {
        for channels in [1, 2, 6, 12] {
            for bands in 2..=4 {
                for kind in ["LR24", "LR48"] {
                    let frames = 2_113;
                    let mut state = 0x1234_5678_u32;
                    let mut input = vec![0.0_f32; frames * channels];
                    for frame in 0..frames {
                        for channel in 0..channels {
                            state ^= state << 13;
                            state ^= state >> 17;
                            state ^= state << 5;
                            let noise = (state as i32 as f32) / i32::MAX as f32 * 0.05;
                            input[frame * channels + channel] = noise;
                        }
                    }
                    for channel in 0..channels {
                        input[(17 + channel * 7) * channels + channel] += 0.5;
                    }

                    let contiguous =
                        render_pair(sample_rate, channels, bands, kind, &input, &[frames]);
                    let irregular = render_pair(
                        sample_rate,
                        channels,
                        bands,
                        kind,
                        &input,
                        &[1, 3, 17, 64, 5, 511, 2, 127],
                    );
                    assert_eq!(
                        contiguous, irregular,
                        "{kind}, {bands} bands, {channels} ch, {sample_rate} Hz"
                    );
                    assert!(contiguous.iter().all(|sample| sample.is_finite()));
                }
            }
        }
    }
}

#[test]
fn paired_split_merge_has_stable_gain_phase_and_tonal_correlation() {
    for sample_rate in [32_000, 48_000, 96_000, 192_000] {
        for bands in 2..=4 {
            for kind in ["LR24", "LR48"] {
                for frequency in [80.0, 700.0, 5_000.0, sample_rate as f32 * 0.20] {
                    // Half a second makes the analysis window exactly one
                    // quarter-second. Every probe below has an integer number
                    // of cycles in that window at all supported rates, so the
                    // oracle is not weakened by spectral leakage.
                    let frames = sample_rate as usize / 2;
                    let input: Vec<f32> = (0..frames)
                        .map(|frame| {
                            (std::f32::consts::TAU * frequency * frame as f32 / sample_rate as f32)
                                .sin()
                                * 0.25
                        })
                        .collect();
                    let output = render_pair(
                        sample_rate,
                        1,
                        bands,
                        kind,
                        &input,
                        &[3, 1, 257, 19, 1_024, 7],
                    );
                    let (gain_db, phase, correlation) =
                        transfer_at(&output, &input, sample_rate, frequency);
                    assert!(gain_db.is_finite() && phase.is_finite());
                    assert!(
                        gain_db.abs() < 0.2,
                        "{sample_rate} Hz {kind} {bands}-band gain error at {frequency} Hz: {gain_db} dB"
                    );
                    assert!(
                        correlation > 0.999,
                        "{sample_rate} Hz {kind} {bands}-band tonal correlation at {frequency} Hz: {correlation}"
                    );
                }
            }
        }
    }
}
