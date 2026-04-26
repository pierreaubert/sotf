//! Shared spatial DSP helpers used by spatial SOTF plugins.

pub mod nupc;

/// Validated interleaved buffer sizes for a processing call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InterleavedBufferSizes {
    pub input_samples: usize,
    pub output_samples: usize,
}

/// Compute and validate interleaved input/output buffer sample counts.
pub fn validate_interleaved_io(
    plugin_name: &str,
    num_frames: usize,
    input_channels: usize,
    output_channels: usize,
    input_len: usize,
    output_len: usize,
) -> Result<InterleavedBufferSizes, String> {
    let input_samples = checked_interleaved_samples(plugin_name, num_frames, input_channels)?;
    let output_samples = checked_interleaved_samples(plugin_name, num_frames, output_channels)?;

    if input_len < input_samples {
        return Err(format!(
            "{plugin_name}: input buffer too short: got {input_len} samples, need {input_samples}"
        ));
    }
    if output_len < output_samples {
        return Err(format!(
            "{plugin_name}: output buffer too short: got {output_len} samples, need {output_samples}"
        ));
    }

    Ok(InterleavedBufferSizes {
        input_samples,
        output_samples,
    })
}

/// Compute and validate an interleaved in-place buffer sample count.
pub fn validate_interleaved_in_place(
    plugin_name: &str,
    num_frames: usize,
    channels: usize,
    buffer_len: usize,
) -> Result<usize, String> {
    let required = checked_interleaved_samples(plugin_name, num_frames, channels)?;
    if buffer_len < required {
        return Err(format!(
            "{plugin_name}: buffer too short: got {buffer_len} samples, need {required}"
        ));
    }
    Ok(required)
}

fn checked_interleaved_samples(
    plugin_name: &str,
    num_frames: usize,
    channels: usize,
) -> Result<usize, String> {
    num_frames.checked_mul(channels).ok_or_else(|| {
        format!(
            "{plugin_name}: frame/channel count overflows: {num_frames} frames x {channels} channels"
        )
    })
}
