use super::buffer_comparison::BufferComparison;
use crate::parameters::{ParameterId, ParameterValue};
use crate::plugin::{Plugin, ProcessContext};

/// A harness for testing plugins with varied buffer sizes.
pub fn test_varied_buffer_sizes<P: Plugin>(
    plugin: &mut P,
    sample_rate: f64,
    input: &[f32],
    expected_output: &[f32],
) {
    let buffer_sizes = [1, 16, 32, 64, 128, 256, 512, 1024, 13, 127]; // Includes non-power-of-two
    let num_channels_in = plugin.input_channels();
    let num_channels_out = plugin.output_channels();
    let total_frames = input.len() / num_channels_in;

    for &block_size in &buffer_sizes {
        plugin.reset();
        let mut output = vec![0.0; expected_output.len()];
        let mut frames_processed = 0;

        while frames_processed < total_frames {
            let num_frames = (block_size).min(total_frames - frames_processed);
            let ctx = ProcessContext::new(sample_rate as u32, num_frames);

            let in_slice = &input[frames_processed * num_channels_in
                ..(frames_processed + num_frames) * num_channels_in];
            let out_slice = &mut output[frames_processed * num_channels_out
                ..(frames_processed + num_frames) * num_channels_out];

            plugin.process(in_slice, out_slice, &ctx).unwrap();
            frames_processed += num_frames;
        }

        assert!(
            BufferComparison::compare_rms(&output, expected_output, 1e-5),
            "Failed for block size {}",
            block_size
        );
    }
}

/// A utility to test parameter automation ramps.
pub fn test_parameter_ramp(
    plugin: &mut dyn Plugin,
    param_id: &ParameterId,
    start_val: f32,
    end_val: f32,
    duration_frames: usize,
    sample_rate: f64,
) {
    let channels = plugin.input_channels();
    let output_channels = plugin.output_channels();
    let input = vec![0.5; duration_frames * channels];
    let mut output = vec![0.0; duration_frames * output_channels];

    // We'll process in small blocks to allow parameter updates at block boundaries
    let block_size = 64;
    let warmup_frames = (sample_rate * 0.1).round() as usize;
    let warmup_input = vec![0.5; block_size * channels];
    let mut warmup_output = vec![0.0; block_size * output_channels];
    let mut warmed_frames = 0;

    plugin
        .set_parameter(param_id.clone(), ParameterValue::Float(start_val))
        .unwrap();

    while warmed_frames < warmup_frames {
        let num_frames = block_size.min(warmup_frames - warmed_frames);
        let ctx = ProcessContext::new(sample_rate as u32, num_frames);
        plugin
            .process(
                &warmup_input[..num_frames * channels],
                &mut warmup_output[..num_frames * output_channels],
                &ctx,
            )
            .unwrap();
        warmed_frames += num_frames;
    }

    let mut frames_processed = 0;

    while frames_processed < duration_frames {
        let num_frames = (block_size).min(duration_frames - frames_processed);

        // Calculate current ramp value
        let progress = frames_processed as f32 / duration_frames as f32;
        let val = start_val + (end_val - start_val) * progress;

        plugin
            .set_parameter(param_id.clone(), ParameterValue::Float(val))
            .unwrap();

        let ctx = ProcessContext::new(sample_rate as u32, num_frames);

        let in_slice =
            &input[frames_processed * channels..(frames_processed + num_frames) * channels];
        let out_slice = &mut output
            [frames_processed * output_channels..(frames_processed + num_frames) * output_channels];

        plugin.process(in_slice, out_slice, &ctx).unwrap();
        frames_processed += num_frames;
    }

    // Check for artifacts (sudden jumps in output) per interleaved channel.
    for frame in 1..duration_frames {
        for ch in 0..output_channels {
            let i = frame * output_channels + ch;
            let prev_i = (frame - 1) * output_channels + ch;
            let diff = (output[i] - output[prev_i]).abs();
            assert!(
                diff < 0.1,
                "Artifact detected at frame {} channel {}: jump of {}",
                frame,
                ch,
                diff
            );
        }
    }
}
