use super::consts::MAX_PLUGIN_IPC_CHANNELS;
use super::consts::MAX_PLUGIN_IPC_FRAMES;
use super::invalid::invalid_input;
use super::plugin_ipc_header::audio_base_offset;
use std::io;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginIpcLayout {
    pub sample_rate: u32,
    pub max_frames: u32,
    pub input_channels: u32,
    pub output_channels: u32,
}

impl PluginIpcLayout {
    pub fn new(
        sample_rate: u32,
        max_frames: u32,
        input_channels: u32,
        output_channels: u32,
    ) -> io::Result<Self> {
        if sample_rate == 0 {
            return Err(invalid_input("sample_rate must be non-zero"));
        }
        if max_frames == 0 || max_frames > MAX_PLUGIN_IPC_FRAMES {
            return Err(invalid_input(format!(
                "max_frames must be in 1..={MAX_PLUGIN_IPC_FRAMES}, got {max_frames}"
            )));
        }
        if input_channels > MAX_PLUGIN_IPC_CHANNELS || output_channels > MAX_PLUGIN_IPC_CHANNELS {
            return Err(invalid_input(format!(
                "channel counts must be <= {MAX_PLUGIN_IPC_CHANNELS}, got in={input_channels} out={output_channels}"
            )));
        }
        if input_channels == 0 && output_channels == 0 {
            return Err(invalid_input(
                "at least one input or output channel is required",
            ));
        }

        Ok(Self {
            sample_rate,
            max_frames,
            input_channels,
            output_channels,
        })
    }

    pub(super) fn input_samples(self) -> usize {
        self.max_frames as usize * self.input_channels as usize
    }

    pub(super) fn output_samples(self) -> usize {
        self.max_frames as usize * self.output_channels as usize
    }
}

pub(super) fn total_size(layout: PluginIpcLayout) -> io::Result<usize> {
    let input_bytes = layout
        .input_samples()
        .checked_mul(std::mem::size_of::<f32>())
        .ok_or_else(|| invalid_input("input buffer size overflow"))?;
    let output_bytes = layout
        .output_samples()
        .checked_mul(std::mem::size_of::<f32>())
        .ok_or_else(|| invalid_input("output buffer size overflow"))?;
    audio_base_offset()
        .checked_add(input_bytes)
        .and_then(|size| size.checked_add(output_bytes))
        .ok_or_else(|| invalid_input("shared-memory size overflow"))
}
