// ============================================================================
// Channel-based Element Renderer
// ============================================================================
//
// Renders channel-based audio elements by mapping IAMF channel layouts
// to SotF speaker configurations. Handles scalable layers by selecting
// the best matching layer for the target output.

use crate::error::{IamfError, IamfResult};
use crate::renderer::ElementRenderer;
use crate::types::*;
use sotf_host::speaker_config::SpeakerConfig;

/// Renders channel-based IAMF elements to the target speaker layout.
pub struct ChannelRenderer {
    /// Selected layer channel count (for diagnostics)
    _layer_channels: usize,
    /// Number of substreams to decode for this layer
    substreams_for_layer: usize,
    /// Coupled substreams for this layer
    coupled_for_layer: usize,
    /// Output channel count
    output_channels: usize,
    /// Channel mapping from element to output layout.
    /// channel_map[element_ch] = output_ch (or usize::MAX to discard)
    channel_map: Vec<usize>,
}

impl ChannelRenderer {
    pub fn new(config: &ScalableChannelConfig, target: &SpeakerConfig) -> IamfResult<Self> {
        if config.layers.is_empty() {
            return Err(IamfError::ParseError("No channel layers".into()));
        }

        // Select the best layer: the highest layer whose channel count <= target
        let target_channels = target.total_channels;
        let mut best_layer_idx = 0;
        for (i, layer) in config.layers.iter().enumerate() {
            if layer.loudspeaker_layout.channel_count() <= target_channels {
                best_layer_idx = i;
            }
        }

        let layer = &config.layers[best_layer_idx];
        let layer_channels = layer.loudspeaker_layout.channel_count();

        // Count total substreams up to and including this layer
        let mut substreams_for_layer = 0;
        let mut coupled_for_layer = 0;
        for l in &config.layers[..=best_layer_idx] {
            substreams_for_layer += l.substream_count as usize;
            coupled_for_layer += l.coupled_substream_count as usize;
        }

        // Build channel map: identity for now (assumes matching order)
        // Real implementation would map IAMF channel order to SotF speaker indices
        let mut channel_map = vec![usize::MAX; layer_channels];
        for (i, slot) in channel_map.iter_mut().enumerate().take(layer_channels.min(target_channels)) {
            *slot = i;
        }

        Ok(Self {
            _layer_channels: layer_channels,
            substreams_for_layer,
            coupled_for_layer,
            output_channels: target_channels,
            channel_map,
        })
    }
}

impl ElementRenderer for ChannelRenderer {
    fn render(
        &mut self,
        substream_pcm: &[Vec<f32>],
        output: &mut [f32],
        num_frames: usize,
    ) -> IamfResult<()> {
        // Clear output
        let out_len = num_frames * self.output_channels;
        output[..out_len].fill(0.0);

        // Reassemble substreams into element channels
        // Coupled substreams produce 2 channels, uncoupled produce 1
        let mut element_ch = 0;

        for (ss_idx, pcm) in substream_pcm
            .iter()
            .enumerate()
            .take(self.substreams_for_layer)
        {
            let is_coupled = ss_idx < self.coupled_for_layer;
            let ss_channels = if is_coupled { 2 } else { 1 };

            for frame in 0..num_frames {
                for ch in 0..ss_channels {
                    let src_idx = frame * ss_channels + ch;
                    if src_idx < pcm.len() && element_ch + ch < self.channel_map.len() {
                        let out_ch = self.channel_map[element_ch + ch];
                        if out_ch < self.output_channels {
                            output[frame * self.output_channels + out_ch] += pcm[src_idx];
                        }
                    }
                }
            }
            element_ch += ss_channels;
        }

        Ok(())
    }

    fn output_channels(&self) -> usize {
        self.output_channels
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_host::speaker_config::get_speaker_config;

    #[test]
    fn test_channel_renderer_stereo_to_5_1() {
        let config = ScalableChannelConfig {
            num_layers: 1,
            layers: vec![ChannelLayer {
                loudspeaker_layout: IamfChannelLayout::Stereo,
                output_gain_is_present: false,
                recon_gain_is_present: false,
                substream_count: 1,
                coupled_substream_count: 1,
                output_gain_db: 0.0,
            }],
        };

        let target = get_speaker_config("5.1").unwrap();
        let mut renderer = ChannelRenderer::new(&config, target).unwrap();
        assert_eq!(renderer.output_channels(), 6);

        // Render one frame of stereo (coupled substream = 2 channels)
        let substream_pcm = vec![vec![0.5_f32, -0.5]]; // L=0.5, R=-0.5
        let mut output = vec![0.0_f32; 6];
        renderer.render(&substream_pcm, &mut output, 1).unwrap();

        // First two output channels should have the stereo content
        assert!((output[0] - 0.5).abs() < 1e-6); // L
        assert!((output[1] - (-0.5)).abs() < 1e-6); // R
        // Other channels should be silent
        assert!(output[2].abs() < 1e-6);
    }
}
