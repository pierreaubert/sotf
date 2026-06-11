// ============================================================================
// IAMF Mixer
// ============================================================================
//
// Combines rendered audio elements according to a mix presentation,
// applying element gains and output mix gain.
// Uses SIMD-accelerated accumulation via sotf-host when available.

use crate::error::IamfResult;
use crate::types::*;

use sotf_host::simd::{apply_gain_simd, scale_add_simd};

/// Mix state for a single sub-mix
pub struct MixState {
    pub output_channels: usize,
    pub output_layout: IamfChannelLayout,
    /// Per-element gains in linear scale
    pub element_gains: Vec<f32>,
    /// Output mix gain in linear scale
    pub output_gain: f32,
}

impl MixState {
    pub fn from_sub_mix(sub_mix: &SubMix) -> Self {
        let element_gains: Vec<f32> = sub_mix
            .element_mix_configs
            .iter()
            .map(|e| db_to_linear(e.mix_gain.default_mix_gain_db))
            .collect();

        Self {
            output_channels: sub_mix.output_layout.channel_count(),
            output_layout: sub_mix.output_layout,
            element_gains,
            output_gain: db_to_linear(sub_mix.output_mix_gain.default_mix_gain_db),
        }
    }

    /// Apply a parameter block to update mix gains.
    pub fn apply_parameter_block(&mut self, param_block: &ParameterBlock, sub_mix: &SubMix) {
        // Check if this parameter_id matches any element mix gain
        for (i, emc) in sub_mix.element_mix_configs.iter().enumerate() {
            if emc.mix_gain.parameter_id == param_block.parameter_id
                && let Some(subblock) = param_block.subblocks.first()
                && let ParameterData::MixGain {
                    start_point_value, ..
                } = &subblock.param_data
            {
                self.element_gains[i] = db_to_linear(*start_point_value);
            }
        }

        // Check output mix gain
        if sub_mix.output_mix_gain.parameter_id == param_block.parameter_id
            && let Some(subblock) = param_block.subblocks.first()
            && let ParameterData::MixGain {
                start_point_value, ..
            } = &subblock.param_data
        {
            self.output_gain = db_to_linear(*start_point_value);
        }
    }

    /// Mix from pre-allocated element output buffers (avoids Vec<Vec<f32>> allocation).
    ///
    /// `element_out_bufs`: pre-allocated buffers, one per element (parallel to element_gains)
    /// `output`: final mix output, interleaved [frames × output_channels]
    /// `num_frames`: number of frames
    pub fn mix_from_bufs(
        &self,
        element_out_bufs: &[Vec<f32>],
        output: &mut [f32],
        num_frames: usize,
    ) -> IamfResult<()> {
        let out_len = num_frames * self.output_channels;
        output[..out_len].fill(0.0);

        // SIMD-accelerated element accumulation: output += elem * gain
        for (elem_idx, elem_out) in element_out_bufs.iter().enumerate() {
            let gain = self.element_gains.get(elem_idx).copied().unwrap_or(1.0);

            let src_len = elem_out.len().min(out_len);
            scale_add_simd(&mut output[..src_len], &elem_out[..src_len], gain);
        }

        // SIMD-accelerated output gain
        if (self.output_gain - 1.0).abs() > 1e-6 {
            apply_gain_simd(&mut output[..out_len], self.output_gain);
        }

        Ok(())
    }

    /// Mix multiple element outputs into a single interleaved output buffer.
    ///
    /// `element_outputs`: one output buffer per audio element, each interleaved
    /// `output`: final mix output, interleaved [frames × output_channels]
    /// `num_frames`: number of frames
    pub fn mix(
        &self,
        element_outputs: &[Vec<f32>],
        output: &mut [f32],
        num_frames: usize,
    ) -> IamfResult<()> {
        self.mix_from_bufs(element_outputs, output, num_frames)
    }
}

fn db_to_linear(db: f32) -> f32 {
    sotf_host::db_to_linear(db)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_db_to_linear() {
        assert!((db_to_linear(0.0) - 1.0).abs() < 1e-6);
        assert!((db_to_linear(-6.0) - 0.501187).abs() < 1e-3);
        assert!((db_to_linear(6.0) - 1.995262).abs() < 1e-3);
    }

    #[test]
    fn test_mix_single_element() {
        let sub_mix = SubMix {
            num_audio_elements: 1,
            element_mix_configs: vec![ElementMixConfig {
                audio_element_id: 0,
                mix_gain: MixGainConfig {
                    parameter_id: 0,
                    default_mix_gain_db: 0.0,
                },
            }],
            output_mix_gain: MixGainConfig {
                parameter_id: 1,
                default_mix_gain_db: 0.0,
            },
            output_layout: IamfChannelLayout::Stereo,
            loudness: LoudnessInfo {
                info_type: 0,
                integrated_loudness: -23.0,
                digital_peak: -1.0,
                true_peak: None,
            },
        };

        let state = MixState::from_sub_mix(&sub_mix);
        assert_eq!(state.output_channels, 2);
        assert!((state.output_gain - 1.0).abs() < 1e-6);

        // Mix a single stereo element
        let elem_output = vec![0.5_f32, -0.5, 0.25, -0.25]; // 2 frames
        let element_outputs = vec![elem_output];
        let mut output = vec![0.0_f32; 4];
        state.mix(&element_outputs, &mut output, 2).unwrap();

        assert!((output[0] - 0.5).abs() < 1e-6);
        assert!((output[1] - (-0.5)).abs() < 1e-6);
    }

    #[test]
    fn test_mix_with_gain() {
        let sub_mix = SubMix {
            num_audio_elements: 1,
            element_mix_configs: vec![ElementMixConfig {
                audio_element_id: 0,
                mix_gain: MixGainConfig {
                    parameter_id: 0,
                    default_mix_gain_db: -6.0, // ~0.5x
                },
            }],
            output_mix_gain: MixGainConfig {
                parameter_id: 1,
                default_mix_gain_db: 0.0,
            },
            output_layout: IamfChannelLayout::Stereo,
            loudness: LoudnessInfo {
                info_type: 0,
                integrated_loudness: -23.0,
                digital_peak: -1.0,
                true_peak: None,
            },
        };

        let state = MixState::from_sub_mix(&sub_mix);
        let elem = vec![1.0_f32, -1.0];
        let mut output = vec![0.0_f32; 2];
        state.mix(&[elem], &mut output, 1).unwrap();

        // -6 dB ≈ 0.501
        assert!((output[0] - 0.501187).abs() < 1e-3);
        assert!((output[1] - (-0.501187)).abs() < 1e-3);
    }

    #[test]
    fn test_apply_parameter_block_element_gain() {
        let sub_mix = SubMix {
            num_audio_elements: 1,
            element_mix_configs: vec![ElementMixConfig {
                audio_element_id: 0,
                mix_gain: MixGainConfig {
                    parameter_id: 42,
                    default_mix_gain_db: 0.0,
                },
            }],
            output_mix_gain: MixGainConfig {
                parameter_id: 99,
                default_mix_gain_db: 0.0,
            },
            output_layout: IamfChannelLayout::Stereo,
            loudness: LoudnessInfo {
                info_type: 0,
                integrated_loudness: -23.0,
                digital_peak: -1.0,
                true_peak: None,
            },
        };

        let mut state = MixState::from_sub_mix(&sub_mix);
        // 6 dB ≈ 1.995 linear
        let pb = ParameterBlock {
            parameter_id: 42,
            duration: 10,
            constant_subblock_duration: 10,
            subblocks: vec![ParameterSubblock {
                subblock_duration: 10,
                param_data: ParameterData::MixGain {
                    animation_type: AnimationType::Step,
                    start_point_value: 6.0,
                    end_point_value: 6.0,
                    control_point_value: 0.0,
                    control_point_relative_time: 0.0,
                },
            }],
        };
        state.apply_parameter_block(&pb, &sub_mix);
        assert!((state.element_gains[0] - 1.995262).abs() < 1e-3);
    }

    #[test]
    fn test_apply_parameter_block_output_gain() {
        let sub_mix = SubMix {
            num_audio_elements: 0,
            element_mix_configs: vec![],
            output_mix_gain: MixGainConfig {
                parameter_id: 7,
                default_mix_gain_db: 0.0,
            },
            output_layout: IamfChannelLayout::Stereo,
            loudness: LoudnessInfo {
                info_type: 0,
                integrated_loudness: -23.0,
                digital_peak: -1.0,
                true_peak: None,
            },
        };

        let mut state = MixState::from_sub_mix(&sub_mix);
        let pb = ParameterBlock {
            parameter_id: 7,
            duration: 10,
            constant_subblock_duration: 10,
            subblocks: vec![ParameterSubblock {
                subblock_duration: 10,
                param_data: ParameterData::MixGain {
                    animation_type: AnimationType::Step,
                    start_point_value: -6.0,
                    end_point_value: -6.0,
                    control_point_value: 0.0,
                    control_point_relative_time: 0.0,
                },
            }],
        };
        state.apply_parameter_block(&pb, &sub_mix);
        assert!((state.output_gain - 0.501187).abs() < 1e-3);
    }

    #[test]
    fn test_mix_multiple_elements() {
        let sub_mix = SubMix {
            num_audio_elements: 2,
            element_mix_configs: vec![
                ElementMixConfig {
                    audio_element_id: 0,
                    mix_gain: MixGainConfig {
                        parameter_id: 0,
                        default_mix_gain_db: 0.0,
                    },
                },
                ElementMixConfig {
                    audio_element_id: 1,
                    mix_gain: MixGainConfig {
                        parameter_id: 1,
                        default_mix_gain_db: 0.0,
                    },
                },
            ],
            output_mix_gain: MixGainConfig {
                parameter_id: 2,
                default_mix_gain_db: 0.0,
            },
            output_layout: IamfChannelLayout::Stereo,
            loudness: LoudnessInfo {
                info_type: 0,
                integrated_loudness: -23.0,
                digital_peak: -1.0,
                true_peak: None,
            },
        };

        let state = MixState::from_sub_mix(&sub_mix);
        let elem_a = vec![1.0_f32, 0.0];
        let elem_b = vec![0.0_f32, 1.0];
        let mut output = vec![0.0_f32; 2];
        state
            .mix_from_bufs(&[elem_a, elem_b], &mut output, 1)
            .unwrap();

        assert!((output[0] - 1.0).abs() < 1e-6);
        assert!((output[1] - 1.0).abs() < 1e-6);
    }

    #[test]
    fn test_mix_with_output_gain() {
        let sub_mix = SubMix {
            num_audio_elements: 1,
            element_mix_configs: vec![ElementMixConfig {
                audio_element_id: 0,
                mix_gain: MixGainConfig {
                    parameter_id: 0,
                    default_mix_gain_db: 0.0,
                },
            }],
            output_mix_gain: MixGainConfig {
                parameter_id: 1,
                default_mix_gain_db: -6.0,
            },
            output_layout: IamfChannelLayout::Stereo,
            loudness: LoudnessInfo {
                info_type: 0,
                integrated_loudness: -23.0,
                digital_peak: -1.0,
                true_peak: None,
            },
        };

        let state = MixState::from_sub_mix(&sub_mix);
        let elem = vec![1.0_f32, 1.0];
        let mut output = vec![0.0_f32; 2];
        state.mix_from_bufs(&[elem], &mut output, 1).unwrap();

        assert!((output[0] - 0.501187).abs() < 1e-3);
        assert!((output[1] - 0.501187).abs() < 1e-3);
    }

    #[test]
    fn test_mix_missing_gain_defaults_to_unity() {
        // Element buffer present but no matching gain entry.
        let sub_mix = SubMix {
            num_audio_elements: 1,
            element_mix_configs: vec![ElementMixConfig {
                audio_element_id: 0,
                mix_gain: MixGainConfig {
                    parameter_id: 0,
                    default_mix_gain_db: 0.0,
                },
            }],
            output_mix_gain: MixGainConfig {
                parameter_id: 1,
                default_mix_gain_db: 0.0,
            },
            output_layout: IamfChannelLayout::Stereo,
            loudness: LoudnessInfo {
                info_type: 0,
                integrated_loudness: -23.0,
                digital_peak: -1.0,
                true_peak: None,
            },
        };

        let state = MixState::from_sub_mix(&sub_mix);
        // Provide an extra element buffer with no corresponding gain.
        let elem_a = vec![0.5_f32, 0.5];
        let elem_b = vec![0.25_f32, -0.25];
        let mut output = vec![0.0_f32; 2];
        state
            .mix_from_bufs(&[elem_a, elem_b], &mut output, 1)
            .unwrap();

        // elem_a at unity + elem_b at default unity.
        assert!((output[0] - 0.75).abs() < 1e-6);
        assert!((output[1] - 0.25).abs() < 1e-6);
    }
}
