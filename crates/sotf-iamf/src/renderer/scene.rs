// ============================================================================
// Scene-based (Ambisonics) Element Renderer
// ============================================================================
//
// Renders scene-based IAMF audio elements using the Ambisonics decoder
// from sotf-plugin-ambisonics.

use crate::error::{IamfError, IamfResult};
use crate::renderer::ElementRenderer;
use crate::types::*;
use sotf_host::speaker_config::SpeakerConfig;

/// Renders Ambisonics (scene-based) IAMF elements to the target speaker layout.
pub struct SceneRenderer {
    /// Number of Ambisonics output channels (ACN ordering)
    ambi_channels: usize,
    /// Target output channel count
    output_channels: usize,
    /// Decode matrix [output_channels × ambi_channels], row-major
    decode_matrix: Vec<f32>,
    /// Ambisonics mode (mono or projection)
    mode: AmbisonicsMode,
    /// Channel mapping from ACN to substream channels
    channel_mapping: Vec<u8>,
    /// Demixing matrix for projection mode
    demixing_matrix: Vec<f32>,
    /// Number of coupled substreams
    coupled_substream_count: usize,
    /// Number of substreams
    substream_count: usize,
    /// Pre-allocated reassembly buffer (avoids per-render allocation)
    ambi_buf: Vec<f32>,
}

impl SceneRenderer {
    pub fn new(config: &AmbisonicsConfig, target: &SpeakerConfig) -> IamfResult<Self> {
        let ambi_channels = config.output_channel_count as usize;

        // Determine Ambisonics order from channel count: (order+1)² = channels
        let order = ((ambi_channels as f64).sqrt() as usize).saturating_sub(1);
        if (order + 1) * (order + 1) != ambi_channels {
            return Err(IamfError::ParseError(format!(
                "Invalid Ambisonics channel count {ambi_channels}: not a perfect square"
            )));
        }

        // Build decode matrix using our AllRAD decoder
        let dm = sotf_plugin_ambisonics::decode_matrix::DecodeMatrix::build(
            order, target, true, // always use max-rE for IAMF
        )
        .map_err(|e| {
            IamfError::ParseError(format!("Failed to build Ambisonics decode matrix: {e}"))
        })?;

        Ok(Self {
            ambi_channels,
            output_channels: target.total_channels,
            decode_matrix: dm.matrix,
            mode: config.ambisonics_mode,
            channel_mapping: config.channel_mapping.clone(),
            demixing_matrix: config.demixing_matrix.clone(),
            coupled_substream_count: config.coupled_substream_count as usize,
            substream_count: config.substream_count as usize,
            ambi_buf: Vec::new(), // sized on first render
        })
    }

    /// Reassemble substream PCM into ACN-ordered Ambisonics channels,
    /// writing into `self.ambi_buf`.
    fn reassemble_ambisonics(&mut self, substream_pcm: &[Vec<f32>], num_frames: usize) {
        let needed = num_frames * self.ambi_channels;
        self.ambi_buf.resize(needed, 0.0);
        self.ambi_buf[..needed].fill(0.0);

        match self.mode {
            AmbisonicsMode::Mono => {
                // Each substream maps to one ACN channel via channel_mapping
                let mut ss_idx = 0;
                for _ in 0..self.coupled_substream_count {
                    if ss_idx < substream_pcm.len() {
                        let pcm = &substream_pcm[ss_idx];
                        // Coupled: 2 channels per substream
                        for frame in 0..num_frames {
                            for ch in 0..2 {
                                let mapping_idx = ss_idx * 2 + ch;
                                if mapping_idx < self.channel_mapping.len() {
                                    let acn = self.channel_mapping[mapping_idx] as usize;
                                    if acn < self.ambi_channels {
                                        let src = frame * 2 + ch;
                                        if src < pcm.len() {
                                            self.ambi_buf[frame * self.ambi_channels + acn] =
                                                pcm[src];
                                        }
                                    }
                                }
                            }
                        }
                    }
                    ss_idx += 1;
                }
                let uncoupled = self.substream_count - self.coupled_substream_count;
                for _ in 0..uncoupled {
                    if ss_idx < substream_pcm.len() {
                        let pcm = &substream_pcm[ss_idx];
                        let mapping_idx = self.coupled_substream_count * 2
                            + (ss_idx - self.coupled_substream_count);
                        if mapping_idx < self.channel_mapping.len() {
                            let acn = self.channel_mapping[mapping_idx] as usize;
                            if acn < self.ambi_channels {
                                for frame in 0..num_frames {
                                    if frame < pcm.len() {
                                        self.ambi_buf[frame * self.ambi_channels + acn] =
                                            pcm[frame];
                                    }
                                }
                            }
                        }
                    }
                    ss_idx += 1;
                }
            }
            AmbisonicsMode::Projection => {
                // Assemble substream channels first
                let coupled_ch = self.coupled_substream_count * 2;
                let uncoupled_ch = self.substream_count - self.coupled_substream_count;
                let ss_channels = coupled_ch + uncoupled_ch;

                let mut ss_buf = vec![0.0_f32; num_frames * ss_channels];

                let mut ch_idx = 0;
                for (ss_idx, pcm) in substream_pcm.iter().enumerate().take(self.substream_count) {
                    let is_coupled = ss_idx < self.coupled_substream_count;
                    let n_ch = if is_coupled { 2 } else { 1 };
                    for frame in 0..num_frames {
                        for c in 0..n_ch {
                            let src = frame * n_ch + c;
                            if src < pcm.len() && ch_idx + c < ss_channels {
                                ss_buf[frame * ss_channels + ch_idx + c] = pcm[src];
                            }
                        }
                    }
                    ch_idx += n_ch;
                }

                // Apply demixing matrix: ambi[acn] = sum(demix[acn][ss_ch] * ss[ss_ch])
                for frame in 0..num_frames {
                    for acn in 0..self.ambi_channels {
                        let mut sum = 0.0_f32;
                        for ss_ch in 0..ss_channels {
                            let matrix_idx = acn * ss_channels + ss_ch;
                            if matrix_idx < self.demixing_matrix.len() {
                                sum += self.demixing_matrix[matrix_idx]
                                    * ss_buf[frame * ss_channels + ss_ch];
                            }
                        }
                        self.ambi_buf[frame * self.ambi_channels + acn] = sum;
                    }
                }
            }
        }
    }
}

impl ElementRenderer for SceneRenderer {
    fn render(
        &mut self,
        substream_pcm: &[Vec<f32>],
        output: &mut [f32],
        num_frames: usize,
    ) -> IamfResult<()> {
        let out_len = num_frames * self.output_channels;
        output[..out_len].fill(0.0);

        // Reassemble ACN-ordered Ambisonics channels (uses pre-allocated buffer)
        self.reassemble_ambisonics(substream_pcm, num_frames);

        // Apply decode matrix: output[spk] = sum(D[spk][acn] * ambi[acn])
        // The bounds check is removed from the inner loop — the matrix is
        // guaranteed to be exactly output_channels × ambi_channels by construction
        // in DecodeMatrix::build(). This allows LLVM to auto-vectorize the inner loop.
        let matrix = &self.decode_matrix;
        let ambi_ch = self.ambi_channels;
        let out_ch = self.output_channels;

        for frame in 0..num_frames {
            let in_offset = frame * ambi_ch;
            let out_offset = frame * out_ch;
            for spk in 0..out_ch {
                let row_start = spk * ambi_ch;
                let mut sum = 0.0_f32;
                for acn in 0..ambi_ch {
                    sum = matrix[row_start + acn].mul_add(self.ambi_buf[in_offset + acn], sum);
                }
                output[out_offset + spk] = sum;
            }
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
    fn test_scene_renderer_foa_mono() {
        // FOA (4 ACN channels) with mono mode
        let config = AmbisonicsConfig {
            ambisonics_mode: AmbisonicsMode::Mono,
            output_channel_count: 4,
            substream_count: 4,
            coupled_substream_count: 0,
            channel_mapping: vec![0, 1, 2, 3], // identity
            demixing_matrix: Vec::new(),
        };

        let target = get_speaker_config("5.1").unwrap();
        let mut renderer = SceneRenderer::new(&config, target).unwrap();
        assert_eq!(renderer.output_channels(), 6);

        // Render silence
        let substream_pcm: Vec<Vec<f32>> = vec![vec![0.0]; 4];
        let mut output = vec![0.0_f32; 6];
        renderer.render(&substream_pcm, &mut output, 1).unwrap();

        for s in &output {
            assert!(s.abs() < 1e-6);
        }
    }

    #[test]
    fn test_scene_renderer_omni_signal() {
        let config = AmbisonicsConfig {
            ambisonics_mode: AmbisonicsMode::Mono,
            output_channel_count: 4,
            substream_count: 4,
            coupled_substream_count: 0,
            channel_mapping: vec![0, 1, 2, 3],
            demixing_matrix: Vec::new(),
        };

        let target = get_speaker_config("5.1").unwrap();
        let mut renderer = SceneRenderer::new(&config, target).unwrap();

        // Pure W signal (omnidirectional): ACN 0 = 1.0, rest = 0
        let substream_pcm: Vec<Vec<f32>> = vec![vec![1.0], vec![0.0], vec![0.0], vec![0.0]];
        let mut output = vec![0.0_f32; 6];
        renderer.render(&substream_pcm, &mut output, 1).unwrap();

        // Non-LFE speakers should have non-zero output
        let non_lfe: Vec<f32> = target
            .speakers
            .iter()
            .filter(|s| !s.is_lfe)
            .map(|s| output[s.channel])
            .collect();
        for &level in &non_lfe {
            assert!(level.abs() > 0.01, "Expected non-zero output, got {level}");
        }
    }

    #[test]
    fn test_scene_renderer_invalid_channel_count() {
        // 5 is not a perfect square -> invalid.
        let config = AmbisonicsConfig {
            ambisonics_mode: AmbisonicsMode::Mono,
            output_channel_count: 5,
            substream_count: 5,
            coupled_substream_count: 0,
            channel_mapping: vec![0, 1, 2, 3, 4],
            demixing_matrix: Vec::new(),
        };

        let target = get_speaker_config("5.1").unwrap();
        assert!(SceneRenderer::new(&config, target).is_err());
    }

    #[test]
    fn test_scene_renderer_projection_smoke() {
        // 1st-order projection: 4 ACN channels from 4 substream channels.
        let config = AmbisonicsConfig {
            ambisonics_mode: AmbisonicsMode::Projection,
            output_channel_count: 4,
            substream_count: 4,
            coupled_substream_count: 0,
            channel_mapping: vec![0, 1, 2, 3],
            // Identity demixing matrix: 4 x 4
            demixing_matrix: vec![
                1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
            ],
        };

        let target = get_speaker_config("5.1").unwrap();
        let mut renderer = SceneRenderer::new(&config, target).unwrap();
        assert_eq!(renderer.output_channels(), 6);

        // W=1, others 0 -> same omni behavior as mono mode with identity matrix.
        let substream_pcm: Vec<Vec<f32>> = vec![vec![1.0], vec![0.0], vec![0.0], vec![0.0]];
        let mut output = vec![0.0_f32; 6];
        renderer.render(&substream_pcm, &mut output, 1).unwrap();

        let non_lfe: Vec<f32> = target
            .speakers
            .iter()
            .filter(|s| !s.is_lfe)
            .map(|s| output[s.channel])
            .collect();
        for &level in &non_lfe {
            assert!(level.abs() > 0.01, "Expected non-zero output, got {level}");
        }
    }

    #[test]
    fn test_scene_renderer_output_channels() {
        let config = AmbisonicsConfig {
            ambisonics_mode: AmbisonicsMode::Mono,
            output_channel_count: 4,
            substream_count: 4,
            coupled_substream_count: 0,
            channel_mapping: vec![0, 1, 2, 3],
            demixing_matrix: Vec::new(),
        };

        let target = get_speaker_config("5.1").unwrap();
        let renderer = SceneRenderer::new(&config, target).unwrap();
        assert_eq!(renderer.output_channels(), 6);
    }

    #[test]
    fn test_reassemble_ambisonics_mono_coupled() {
        // Two coupled substreams = 4 channels, mapped to ACN 0..3.
        let config = AmbisonicsConfig {
            ambisonics_mode: AmbisonicsMode::Mono,
            output_channel_count: 4,
            substream_count: 2,
            coupled_substream_count: 2,
            channel_mapping: vec![0, 1, 2, 3],
            demixing_matrix: Vec::new(),
        };

        let target = get_speaker_config("5.1").unwrap();
        let mut renderer = SceneRenderer::new(&config, target).unwrap();

        // Coupled substreams are interleaved: [W,Y, Z,X] for one frame.
        let ss0 = vec![1.0_f32, 0.0]; // W, Y
        let ss1 = vec![0.0_f32, 1.0]; // Z, X
        let mut output = vec![0.0_f32; 6];
        renderer.render(&[ss0, ss1], &mut output, 1).unwrap();

        // All four ACN channels are populated; W dominates omnidirectional.
        let non_lfe: Vec<f32> = target
            .speakers
            .iter()
            .filter(|s| !s.is_lfe)
            .map(|s| output[s.channel])
            .collect();
        for &level in &non_lfe {
            assert!(level.abs() > 0.01, "Expected non-zero output, got {level}");
        }
    }
}
