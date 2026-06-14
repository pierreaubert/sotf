use std::ops::AddAssign;

pub(super) trait AudioSample: Copy + Default + AddAssign + Send + Sync + 'static {
    fn scale_add(dst: &mut [Self], src: &[Self]);
}

impl AudioSample for f32 {
    fn scale_add(dst: &mut [Self], src: &[Self]) {
        crate::simd::scale_add_simd(dst, src, 1.0);
    }
}

impl AudioSample for f64 {
    fn scale_add(dst: &mut [Self], src: &[Self]) {
        for (dst, &src) in dst.iter_mut().zip(src.iter()) {
            *dst += src;
        }
    }
}

pub(super) fn ensure_len<T: AudioSample>(buffer: &mut Vec<T>, len: usize) {
    if buffer.len() < len {
        buffer.resize(len, T::default());
    }
}

pub(super) fn write_plugin_failure_passthrough<T: AudioSample>(
    input: &[T],
    output: &mut [T],
    num_frames: usize,
    input_channels: usize,
    output_channels: usize,
) -> usize {
    let output_len = num_frames.saturating_mul(output_channels).min(output.len());
    output[..output_len].fill(T::default());

    if input_channels == 0 || output_channels == 0 {
        return num_frames;
    }

    let copy_channels = input_channels.min(output_channels);
    for frame in 0..num_frames {
        let src_base = frame.saturating_mul(input_channels);
        let dst_base = frame.saturating_mul(output_channels);
        if src_base >= input.len() || dst_base >= output_len {
            break;
        }

        let src_end = (src_base + copy_channels).min(input.len());
        let dst_end = (dst_base + copy_channels).min(output_len);
        let copied = (src_end - src_base).min(dst_end - dst_base);
        output[dst_base..dst_base + copied].copy_from_slice(&input[src_base..src_base + copied]);
    }

    num_frames
}
