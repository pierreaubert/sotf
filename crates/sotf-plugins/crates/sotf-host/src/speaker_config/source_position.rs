use super::calculate::calculate_panning_gain;
use super::calculate::calculate_panning_gain_with_wraparound;
use super::types::SpeakerConfig;

/// A virtual source position used as input to `compute_vbap_matrix`.
#[derive(Debug, Clone, Copy)]
pub struct SourcePosition {
    pub azimuth_deg: f32,
    pub elevation_deg: f32,
}

impl SourcePosition {
    pub fn new(azimuth_deg: f32, elevation_deg: f32) -> Self {
        Self {
            azimuth_deg,
            elevation_deg,
        }
    }
}

/// Compute a VBAP gain matrix for a batch of virtual sources against a speaker
/// configuration.
///
/// Returns `gains[src][channel]` where:
/// - `gains.len() == sources.len()`,
/// - each row has length `speaker_config.total_channels`,
/// - LFE channels are always zeroed (LFE is handled separately by callers),
/// - other channels use `calculate_panning_gain` (when `wraparound` is `None`)
///   or `calculate_panning_gain_with_wraparound` (when `Some(attenuation)`).
///
/// Rows are NOT energy-normalized — call `normalize_gains_l2` per row if you
/// want energy preservation. Some callers need to override specific channels
/// (e.g. center/LFE) before normalizing.
pub fn compute_vbap_matrix(
    speaker_config: &SpeakerConfig,
    sources: &[SourcePosition],
    wraparound: Option<f32>,
) -> Vec<Vec<f32>> {
    let n_ch = speaker_config.total_channels;
    sources
        .iter()
        .map(|src| {
            let mut row = vec![0.0_f32; n_ch];
            for sp in speaker_config.speakers {
                if sp.is_lfe || sp.channel >= n_ch {
                    continue;
                }
                row[sp.channel] = match wraparound {
                    Some(att) => calculate_panning_gain_with_wraparound(
                        src.azimuth_deg,
                        src.elevation_deg,
                        sp.azimuth,
                        sp.elevation,
                        att,
                    ),
                    None => calculate_panning_gain(
                        src.azimuth_deg,
                        src.elevation_deg,
                        sp.azimuth,
                        sp.elevation,
                    ),
                };
            }
            row
        })
        .collect()
}
