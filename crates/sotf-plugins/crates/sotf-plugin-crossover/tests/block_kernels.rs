use sotf_host::lr4_crossover::Lr4Crossover;
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_host::{CountingAlloc, assert_no_allocs};
use sotf_plugin_crossover::{CrossoverPlugin, CrossoverPluginParams, PerChannelOpMode};

#[global_allocator]
static ALLOCATOR: CountingAlloc = CountingAlloc;

const SAMPLE_RATE: u32 = 48_000;

fn signal(frames: usize, channels: usize) -> Vec<f32> {
    (0..frames * channels)
        .map(|index| {
            let frame = index / channels;
            let channel = index % channels;
            ((frame as f32 * 0.071 + channel as f32 * 0.37).sin()
                + (frame as f32 * 0.013 - channel as f32 * 0.19).cos())
                * 0.2
        })
        .collect()
}

#[test]
fn two_way_block_kernel_is_bit_exact_with_scalar_lr24_reference() {
    for channels in [1, 2, 8] {
        for mode in ["lowpass", "highpass", "both"] {
            let frames = 2_113;
            let input = signal(frames, channels);
            let mut plugin = CrossoverPlugin::new(channels, "LR24", 1_000.0, mode).unwrap();
            plugin.initialize(SAMPLE_RATE).unwrap();
            let mut actual = vec![0.0; frames * plugin.output_channels()];
            plugin
                .process(
                    &input,
                    &mut actual,
                    &ProcessContext::new(SAMPLE_RATE, frames),
                )
                .unwrap();

            let mut reference = Lr4Crossover::new(1_000.0, SAMPLE_RATE as f32, channels);
            let output_channels = if mode == "both" {
                channels * 2
            } else {
                channels
            };
            let mut expected = vec![0.0; frames * output_channels];
            for frame in 0..frames {
                for channel in 0..channels {
                    let (low, high) = reference.process(input[frame * channels + channel], channel);
                    match mode {
                        "lowpass" => expected[frame * output_channels + channel] = low,
                        "highpass" => expected[frame * output_channels + channel] = high,
                        "both" => {
                            expected[frame * output_channels + channel] = low;
                            expected[frame * output_channels + channels + channel] = high;
                        }
                        _ => unreachable!(),
                    }
                }
            }
            assert_eq!(actual, expected, "{channels} channels, {mode}");
        }
    }
}

#[test]
fn per_channel_block_kernel_is_bit_exact_with_scalar_cells() {
    let modes = [
        PerChannelOpMode::Lowpass,
        PerChannelOpMode::Highpass,
        PerChannelOpMode::Mute,
        PerChannelOpMode::Passthrough,
    ];
    let frequencies = [120.0, 2_500.0, 800.0, 4_000.0];
    let frames = 2_113;
    let input = signal(frames, modes.len());
    let mut plugin =
        CrossoverPlugin::new_per_channel("LR24", frequencies.to_vec(), modes.to_vec()).unwrap();
    plugin.initialize(SAMPLE_RATE).unwrap();
    let mut actual = vec![0.0; input.len()];
    plugin
        .process(
            &input,
            &mut actual,
            &ProcessContext::new(SAMPLE_RATE, frames),
        )
        .unwrap();

    let mut cells: Vec<_> = frequencies
        .iter()
        .map(|frequency| Lr4Crossover::new(*frequency, SAMPLE_RATE as f32, 1))
        .collect();
    let mut expected = vec![0.0; input.len()];
    for channel in 0..modes.len() {
        for frame in 0..frames {
            let sample = input[frame * modes.len() + channel];
            let (low, high) = cells[channel].process(sample, 0);
            expected[frame * modes.len() + channel] = match modes[channel] {
                PerChannelOpMode::Lowpass => low,
                PerChannelOpMode::Highpass => high,
                PerChannelOpMode::Mute => 0.0,
                PerChannelOpMode::Passthrough => sample,
            };
        }
    }
    assert_eq!(actual, expected);
}

fn render_partitioned(mut plugin: CrossoverPlugin, input: &[f32], parts: &[usize]) -> Vec<f32> {
    plugin.initialize(SAMPLE_RATE).unwrap();
    let input_channels = plugin.input_channels();
    let output_channels = plugin.output_channels();
    let frames = input.len() / input_channels;
    let mut output = vec![0.0; frames * output_channels];
    let mut frame = 0;
    let mut part = 0;
    while frame < frames {
        let count = parts[part % parts.len()].min(frames - frame);
        plugin
            .process(
                &input[frame * input_channels..(frame + count) * input_channels],
                &mut output[frame * output_channels..(frame + count) * output_channels],
                &ProcessContext::new(SAMPLE_RATE, count),
            )
            .unwrap();
        frame += count;
        part += 1;
    }
    output
}

#[test]
fn every_block_kernel_is_callback_partition_invariant() {
    fn make_case(index: usize) -> CrossoverPlugin {
        match index {
            0 => CrossoverPlugin::new(2, "LR24", 1_000.0, "both").unwrap(),
            1 => CrossoverPlugin::new_multiway(2, "LR24", 200.0, "both", &[1_200.0, 6_000.0])
                .unwrap(),
            2 => CrossoverPlugin::from_params(
                2,
                &CrossoverPluginParams {
                    crossover_type: "FIR".into(),
                    frequency: 200.0,
                    output: "both".into(),
                    extra_frequencies: vec![1_200.0, 6_000.0],
                    fir_taps: Some(63),
                    channel_frequencies_hz: vec![],
                    channel_modes: vec![],
                },
            )
            .unwrap(),
            3 => CrossoverPlugin::new_per_channel(
                "LR24",
                vec![120.0, 2_500.0],
                vec![PerChannelOpMode::Lowpass, PerChannelOpMode::Highpass],
            )
            .unwrap(),
            _ => unreachable!(),
        }
    }

    let frames = 2_113;
    for index in 0..4 {
        let plugin = make_case(index);
        let input = signal(frames, plugin.input_channels());
        let expected = render_partitioned(plugin, &input, &[frames]);
        let actual = render_partitioned(make_case(index), &input, &[1, 3, 17, 64, 511, 2, 127]);
        assert_eq!(actual, expected);
    }
}

#[test]
fn steady_block_kernels_allocate_nothing() {
    let mut cases = [
        CrossoverPlugin::new(2, "LR24", 1_000.0, "both").unwrap(),
        CrossoverPlugin::new_multiway(2, "LR24", 200.0, "both", &[1_200.0, 6_000.0]).unwrap(),
        CrossoverPlugin::new_per_channel(
            "LR24",
            vec![120.0, 2_500.0],
            vec![PerChannelOpMode::Lowpass, PerChannelOpMode::Highpass],
        )
        .unwrap(),
        CrossoverPlugin::from_params(
            2,
            &CrossoverPluginParams {
                crossover_type: "FIR".into(),
                frequency: 200.0,
                output: "both".into(),
                extra_frequencies: vec![1_200.0, 6_000.0],
                fir_taps: Some(63),
                channel_frequencies_hz: vec![],
                channel_modes: vec![],
            },
        )
        .unwrap(),
    ];
    for plugin in &mut cases {
        plugin.initialize(SAMPLE_RATE).unwrap();
        let frames = 512;
        let input = signal(frames, plugin.input_channels());
        let mut output = vec![0.0; frames * plugin.output_channels()];
        let context = ProcessContext::new(SAMPLE_RATE, frames);
        assert_no_allocs("crossover block kernel", || {
            plugin.process(&input, &mut output, &context).unwrap();
        });
    }
}

#[test]
fn fir_memory_report_is_exact_and_monotonic() {
    let two_way = CrossoverPlugin::from_params(
        2,
        &CrossoverPluginParams {
            crossover_type: "FIR".into(),
            frequency: 1_000.0,
            output: "both".into(),
            extra_frequencies: vec![],
            fir_taps: Some(63),
            channel_frequencies_hz: vec![],
            channel_modes: vec![],
        },
    )
    .unwrap();
    let four_way = CrossoverPlugin::from_params(
        2,
        &CrossoverPluginParams {
            crossover_type: "FIR".into(),
            frequency: 200.0,
            output: "both".into(),
            extra_frequencies: vec![1_200.0, 6_000.0],
            fir_taps: Some(63),
            channel_frequencies_hz: vec![],
            channel_modes: vec![],
        },
    )
    .unwrap();
    let two = two_way.fir_memory_report().unwrap();
    let four = four_way.fir_memory_report().unwrap();
    assert_eq!((two.coefficient_bytes, two.history_bytes), (252, 520));
    assert_eq!(
        (two.alignment_bytes, two.scratch_bytes, two.total_bytes),
        (0, 16, 788)
    );
    assert_eq!((four.coefficient_bytes, four.history_bytes), (756, 1_560));
    assert_eq!(
        (four.alignment_bytes, four.scratch_bytes, four.total_bytes),
        (744, 72, 3_132)
    );
    assert!(four.total_bytes > two.total_bytes);
    assert!(
        CrossoverPlugin::new(2, "LR24", 1_000.0, "both")
            .unwrap()
            .fir_memory_report()
            .is_none()
    );
}
