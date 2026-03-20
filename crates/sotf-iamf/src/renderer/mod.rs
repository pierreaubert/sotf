// ============================================================================
// IAMF Renderer
// ============================================================================
//
// Renders decoded audio element substreams to the target speaker layout.
// Handles channel-based (layout mapping) and scene-based (Ambisonics) elements.

pub mod channel;
pub mod scene;

use crate::error::IamfResult;
use crate::types::*;
use sotf_host::speaker_config::SpeakerConfig;

/// Trait for rendering an audio element to the output layout.
pub trait ElementRenderer: Send {
    /// Render decoded substream PCM into the output buffer.
    ///
    /// `substream_pcm`: decoded samples per substream, each Vec is interleaved
    /// `output`: target interleaved output buffer [frames × output_channels]
    /// `num_frames`: number of frames to render
    fn render(
        &mut self,
        substream_pcm: &[Vec<f32>],
        output: &mut [f32],
        num_frames: usize,
    ) -> IamfResult<()>;

    /// Number of output channels
    fn output_channels(&self) -> usize;
}

/// Create a renderer for an audio element.
pub fn create_renderer(
    element: &AudioElement,
    _codec_config: &CodecConfig,
    target_layout: &SpeakerConfig,
) -> IamfResult<Box<dyn ElementRenderer>> {
    match &element.element_config {
        ElementConfig::Channel(config) => {
            let renderer = channel::ChannelRenderer::new(config, target_layout)?;
            Ok(Box::new(renderer))
        }
        ElementConfig::Scene(config) => {
            let renderer = scene::SceneRenderer::new(config, target_layout)?;
            Ok(Box::new(renderer))
        }
    }
}
