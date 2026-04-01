//! Buffer format conversion between interleaved (SOTF) and deinterleaved (AU/VST3).
//!
//! SOTF plugins use interleaved buffers: `[L0, R0, L1, R1, ...]`
//! AU uses non-interleaved AudioBufferList: `[[L0, L1, ...], [R0, R1, ...]]`
//! VST3/nih-plug uses non-interleaved channel slices.

/// Interleave separate channel buffers into a single interleaved buffer.
///
/// `channels[c][f]` → `output[f * num_channels + c]`
///
/// # Panics
/// Panics if output is too small or channels have different lengths.
pub fn interleave(channels: &[&[f32]], output: &mut [f32]) {
    if channels.is_empty() {
        return;
    }
    let num_channels = channels.len();
    let num_frames = channels[0].len();
    debug_assert!(
        output.len() >= num_frames * num_channels,
        "Output buffer too small: {} < {}",
        output.len(),
        num_frames * num_channels
    );

    for frame in 0..num_frames {
        let base = frame * num_channels;
        for (ch, channel) in channels.iter().enumerate() {
            output[base + ch] = channel[frame];
        }
    }
}

/// Deinterleave an interleaved buffer into separate channel buffers.
///
/// `input[f * num_channels + c]` → `channels[c][f]`
///
/// # Panics
/// Panics if input is too small for the given frame count and channel count.
pub fn deinterleave(input: &[f32], num_channels: usize, channels: &mut [&mut [f32]]) {
    if num_channels == 0 || channels.is_empty() {
        return;
    }
    let num_frames = channels[0].len();
    debug_assert!(
        input.len() >= num_frames * num_channels,
        "Input buffer too small: {} < {}",
        input.len(),
        num_frames * num_channels
    );

    for frame in 0..num_frames {
        let base = frame * num_channels;
        for (ch, channel) in channels.iter_mut().enumerate() {
            channel[frame] = input[base + ch];
        }
    }
}

/// Pre-allocated scratch buffers for interleave/deinterleave operations.
///
/// Avoids per-frame allocations in the audio callback.
pub struct ScratchBuffers {
    interleaved: Vec<f32>,
    max_channels: usize,
    max_frames: usize,
}

impl ScratchBuffers {
    /// Create scratch buffers for the given maximum channel count and frame count.
    pub fn new(max_channels: usize, max_frames: usize) -> Self {
        Self {
            interleaved: vec![0.0; max_channels * max_frames],
            max_channels,
            max_frames,
        }
    }

    /// Get a mutable slice of the interleaved buffer, sized for the given frame count.
    ///
    /// Returns `None` if the requested size exceeds the pre-allocated capacity.
    pub fn interleaved_mut(&mut self, channels: usize, frames: usize) -> Option<&mut [f32]> {
        let needed = channels * frames;
        if channels > self.max_channels
            || frames > self.max_frames
            || needed > self.interleaved.len()
        {
            return None;
        }
        let slice = &mut self.interleaved[..needed];
        // Zero the buffer before use
        slice.fill(0.0);
        Some(slice)
    }

    /// Get an immutable slice of the interleaved buffer.
    pub fn interleaved(&self, channels: usize, frames: usize) -> Option<&[f32]> {
        let needed = channels * frames;
        if needed > self.interleaved.len() {
            return None;
        }
        Some(&self.interleaved[..needed])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_interleave_stereo() {
        let left = [1.0f32, 2.0, 3.0, 4.0];
        let right = [5.0f32, 6.0, 7.0, 8.0];
        let channels: &[&[f32]] = &[&left, &right];
        let mut output = vec![0.0f32; 8];

        interleave(channels, &mut output);

        assert_eq!(output, [1.0, 5.0, 2.0, 6.0, 3.0, 7.0, 4.0, 8.0]);
    }

    #[test]
    fn test_deinterleave_stereo() {
        let input = [1.0f32, 5.0, 2.0, 6.0, 3.0, 7.0, 4.0, 8.0];
        let mut left = vec![0.0f32; 4];
        let mut right = vec![0.0f32; 4];

        {
            let channels: &mut [&mut [f32]] = &mut [&mut left, &mut right];
            deinterleave(&input, 2, channels);
        }

        assert_eq!(left, [1.0, 2.0, 3.0, 4.0]);
        assert_eq!(right, [5.0, 6.0, 7.0, 8.0]);
    }

    #[test]
    fn test_roundtrip() {
        let original_l = [0.1f32, 0.2, 0.3];
        let original_r = [0.4f32, 0.5, 0.6];
        let channels: &[&[f32]] = &[&original_l, &original_r];

        // Interleave
        let mut interleaved = vec![0.0f32; 6];
        interleave(channels, &mut interleaved);

        // Deinterleave
        let mut recovered_l = vec![0.0f32; 3];
        let mut recovered_r = vec![0.0f32; 3];
        {
            let out_channels: &mut [&mut [f32]] = &mut [&mut recovered_l, &mut recovered_r];
            deinterleave(&interleaved, 2, out_channels);
        }

        assert_eq!(recovered_l, original_l);
        assert_eq!(recovered_r, original_r);
    }

    #[test]
    fn test_scratch_buffers() {
        let mut scratch = ScratchBuffers::new(2, 128);

        let buf = scratch.interleaved_mut(2, 64).unwrap();
        assert_eq!(buf.len(), 128);
        assert!(buf.iter().all(|&x| x == 0.0));

        // Too large should return None
        assert!(scratch.interleaved_mut(2, 256).is_none());
        assert!(scratch.interleaved_mut(4, 128).is_none());
    }
}
