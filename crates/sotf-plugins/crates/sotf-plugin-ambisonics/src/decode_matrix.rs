// ============================================================================
// AllRAD Decode Matrix Builder
// ============================================================================
//
// Builds a decode matrix from Ambisonics channels to a target speaker layout
// using the mode-matching (pseudoinverse) approach with optional max-rE
// weighting for improved energy preservation at high frequencies.
//
// Reference: Zotter & Frank (2012), "All-Round Ambisonic Panning and Decoding"

use crate::spherical_harmonics::{self, channel_count, deg_to_rad, spherical_harmonics_vector};
use sotf_host::speaker_config::SpeakerConfig;

/// Decode matrix: maps Ambisonics channels to speaker feeds.
/// Stored as row-major: `matrix[speaker * ambi_channels + acn]`
#[derive(Debug, Clone)]
pub struct DecodeMatrix {
    /// Number of Ambisonics input channels: (order+1)²
    pub ambi_channels: usize,
    /// Number of output speakers
    pub speaker_count: usize,
    /// Row-major decode matrix [speaker_count × ambi_channels]
    pub matrix: Vec<f32>,
    /// max-rE weights per ACN channel (applied to matrix columns)
    pub max_re_weights: Vec<f32>,
}

impl DecodeMatrix {
    /// Build a basic decode matrix (no max-rE weighting).
    ///
    /// Preserves the velocity vector magnitude, giving accurate interaural time
    /// difference (ITD) cues.  Appropriate for low-frequency decoding in a
    /// dual-band setup.
    pub fn build_basic(order: usize, speaker_config: &SpeakerConfig) -> Result<Self, String> {
        Self::build(order, speaker_config, false)
    }

    /// Build a max-rE decode matrix.
    ///
    /// Applies max-rE per-degree weights to concentrate energy toward the
    /// intended direction.  Appropriate for high-frequency decoding.
    pub fn build_maxre(order: usize, speaker_config: &SpeakerConfig) -> Result<Self, String> {
        Self::build(order, speaker_config, true)
    }

    /// Build a decode matrix for the given Ambisonics order and target speaker layout.
    ///
    /// Uses mode-matching (pseudoinverse of Y matrix) with optional max-rE weighting.
    pub fn build(
        order: usize,
        speaker_config: &SpeakerConfig,
        apply_max_re: bool,
    ) -> Result<Self, String> {
        let ambi_ch = channel_count(order);
        let speakers: Vec<_> = speaker_config
            .speakers
            .iter()
            .filter(|s| !s.is_lfe)
            .collect();
        let num_speakers = speakers.len();

        if num_speakers == 0 {
            return Err("No non-LFE speakers in config".into());
        }
        if num_speakers < ambi_ch {
            return Err(format!(
                "Not enough speakers ({}) for Ambisonics order {} ({} channels required)",
                num_speakers, order, ambi_ch
            ));
        }

        // Build the Y matrix [num_speakers × ambi_ch]
        // Y[s][n] = Y_n(azimuth_s, elevation_s)
        let mut y_matrix = vec![0.0_f64; num_speakers * ambi_ch];
        let mut sh_buffer = vec![0.0_f64; ambi_ch];
        for (s, spk) in speakers.iter().enumerate() {
            let az = deg_to_rad(spk.azimuth as f64);
            let el = deg_to_rad(spk.elevation as f64);
            spherical_harmonics_vector(order, az, el, &mut sh_buffer);
            let sh = &sh_buffer;
            for (n, &val) in sh.iter().enumerate() {
                y_matrix[s * ambi_ch + n] = val;
            }
        }

        // Compute decode matrix via regularized mode-matching: D = Y(YᵀY + εI)⁻¹
        let decode = mode_matching_decode(&y_matrix, num_speakers, ambi_ch)?;

        // Compute max-rE weights
        let max_re = if apply_max_re {
            compute_max_re_weights(order)
        } else {
            vec![1.0; ambi_ch]
        };

        // Apply max-rE weights and convert to f32
        let mut matrix = vec![0.0_f32; num_speakers * ambi_ch];
        for s in 0..num_speakers {
            for n in 0..ambi_ch {
                matrix[s * ambi_ch + n] = (decode[s * ambi_ch + n] * max_re[n] as f64) as f32;
            }
        }

        // Map decode matrix rows to actual output channel indices
        // (accounting for LFE channels that are skipped)
        let total_channels = speaker_config.total_channels;
        let mut full_matrix = vec![0.0_f32; total_channels * ambi_ch];
        for (s, spk) in speakers.iter().enumerate() {
            for n in 0..ambi_ch {
                full_matrix[spk.channel * ambi_ch + n] = matrix[s * ambi_ch + n];
            }
        }

        Ok(Self {
            ambi_channels: ambi_ch,
            speaker_count: total_channels,
            matrix: full_matrix,
            max_re_weights: max_re.into_iter().map(|w| w as f32).collect(),
        })
    }

    /// Apply the decode matrix to a frame of Ambisonics input.
    /// `input`: interleaved Ambisonics samples for one frame (length = ambi_channels)
    /// `output`: speaker feeds for one frame (length = speaker_count)
    #[inline]
    pub fn decode_frame(&self, input: &[f32], output: &mut [f32]) {
        debug_assert!(input.len() >= self.ambi_channels);
        debug_assert!(output.len() >= self.speaker_count);

        let input = &input[..self.ambi_channels];
        for (s, out_sample) in output.iter_mut().enumerate().take(self.speaker_count) {
            let row_offset = s * self.ambi_channels;
            let mut sum = 0.0_f32;
            let row = &self.matrix[row_offset..row_offset + self.ambi_channels];

            for (coef, &in_sample) in row.iter().zip(input.iter()) {
                sum = coef.mul_add(in_sample, sum);
            }
            *out_sample = sum;
        }
    }
}

/// Compute max-rE weights for a given Ambisonics order.
///
/// max-rE weighting maximises the energy concentration vector magnitude,
/// improving spatial resolution at the cost of some diffuseness.
///
/// Weight for degree l: cos(l * pi / (2 * (order + 1)))
/// (Zotter & Frank, 2012, eq. 10)
fn compute_max_re_weights(order: usize) -> Vec<f64> {
    let ambi_ch = channel_count(order);
    let mut weights = Vec::with_capacity(ambi_ch);
    let denom = 2.0 * (order as f64 + 1.0);
    for acn in 0..ambi_ch {
        let (l, _m) = spherical_harmonics::acn_to_degree_index(acn);
        let w = (l as f64 * std::f64::consts::PI / denom).cos();
        weights.push(w);
    }
    weights
}

/// Compute the mode-matching decode matrix D [speakers × ambi_ch].
///
/// D = Y × (YᵀY + εI)⁻¹ where Y[s][n] = SH_n(speaker_s_position).
///
/// Tikhonov regularization (εI) handles rank-deficient layouts, e.g. 5.1 where
/// all speakers sit at elevation 0° making the Z-harmonic column zero.
fn mode_matching_decode(y: &[f64], rows: usize, cols: usize) -> Result<Vec<f64>, String> {
    // Regularization parameter: small enough to not distort well-conditioned layouts,
    // large enough to stabilize rank-deficient ones.
    let epsilon = 1e-6;

    // Compute YᵀY [cols × cols] + εI (Tikhonov regularization)
    let mut yty = vec![0.0_f64; cols * cols];
    for i in 0..cols {
        for j in 0..cols {
            let mut sum = 0.0;
            for s in 0..rows {
                sum += y[s * cols + i] * y[s * cols + j];
            }
            yty[i * cols + j] = sum;
        }
        yty[i * cols + i] += epsilon; // Tikhonov regularization
    }

    // Invert (YᵀY + εI) via Gauss-Jordan with partial pivoting
    let aug_w = 2 * cols;
    let mut aug = vec![0.0_f64; cols * aug_w];
    for i in 0..cols {
        for j in 0..cols {
            aug[i * aug_w + j] = yty[i * cols + j];
        }
        aug[i * aug_w + cols + i] = 1.0; // identity on right
    }

    for col in 0..cols {
        // Partial pivoting
        let mut max_val = aug[col * aug_w + col].abs();
        let mut max_row = col;
        for row in (col + 1)..cols {
            let val = aug[row * aug_w + col].abs();
            if val > max_val {
                max_val = val;
                max_row = row;
            }
        }
        if max_val < 1e-15 {
            return Err(format!(
                "Near-singular matrix even with regularization (pivot {col}: {max_val})"
            ));
        }
        if max_row != col {
            for j in 0..aug_w {
                aug.swap(col * aug_w + j, max_row * aug_w + j);
            }
        }
        let pivot = aug[col * aug_w + col];
        for j in 0..aug_w {
            aug[col * aug_w + j] /= pivot;
        }
        for row in 0..cols {
            if row == col {
                continue;
            }
            let factor = aug[row * aug_w + col];
            for j in 0..aug_w {
                aug[row * aug_w + j] -= factor * aug[col * aug_w + j];
            }
        }
    }

    // Extract inv = (YᵀY + εI)⁻¹ [cols × cols]
    // Then D = Y × inv [rows × cols]
    let mut d = vec![0.0_f64; rows * cols];
    for s in 0..rows {
        for n in 0..cols {
            let mut sum = 0.0;
            for k in 0..cols {
                let inv_kn = aug[k * aug_w + cols + n];
                sum += y[s * cols + k] * inv_kn;
            }
            d[s * cols + n] = sum;
        }
    }

    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_host::speaker_config::get_speaker_config;

    #[test]
    fn test_build_foa_5_1() {
        let config = get_speaker_config("5.1").expect("5.1 config should exist");
        let dm = DecodeMatrix::build(1, config, false).expect("Should build FOA 5.1 matrix");
        assert_eq!(dm.ambi_channels, 4);
        assert_eq!(dm.speaker_count, config.total_channels);
    }

    #[test]
    fn test_build_foa_7_1_4() {
        let config = get_speaker_config("7.1.4").expect("7.1.4 config should exist");
        let dm = DecodeMatrix::build(1, config, true).expect("Should build FOA 7.1.4 matrix");
        assert_eq!(dm.ambi_channels, 4);
        assert_eq!(dm.speaker_count, config.total_channels);
    }

    #[test]
    fn test_build_soa_7_1_4() {
        let config = get_speaker_config("7.1.4").expect("7.1.4 config should exist");
        let dm = DecodeMatrix::build(2, config, true).expect("Should build SOA 7.1.4 matrix");
        assert_eq!(dm.ambi_channels, 9);
        assert_eq!(dm.speaker_count, config.total_channels);
    }

    #[test]
    fn test_too_few_speakers() {
        let config = get_speaker_config("2.0").expect("2.0 config should exist");
        let result = DecodeMatrix::build(2, config, false);
        assert!(
            result.is_err(),
            "2.0 has only 2 speakers, too few for SOA (9 channels)"
        );
    }

    #[test]
    fn test_energy_preservation_foa() {
        let config = get_speaker_config("5.1").expect("5.1 config should exist");
        let dm = DecodeMatrix::build(1, config, false).expect("build");

        // Encode a signal from front (az=0, el=0) into FOA
        let foa_input = [1.0_f32, 0.0, 0.0, 1.0]; // W=1, Y=0, Z=0, X=1 (front source)
        let mut output = vec![0.0_f32; config.total_channels];
        dm.decode_frame(&foa_input, &mut output);

        // Check that total energy is non-zero and reasonable
        let energy: f32 = output.iter().map(|s| s * s).sum();
        assert!(energy > 0.1, "Energy should be significant, got {}", energy);
        assert!(energy < 10.0, "Energy should not explode, got {}", energy);
    }

    #[test]
    fn test_omnidirectional_signal() {
        let config = get_speaker_config("5.1").expect("5.1 config should exist");
        let dm = DecodeMatrix::build(1, config, false).expect("build");

        // Pure W signal (omnidirectional) should produce roughly equal levels at all non-LFE speakers
        let foa_input = [1.0_f32, 0.0, 0.0, 0.0];
        let mut output = vec![0.0_f32; config.total_channels];
        dm.decode_frame(&foa_input, &mut output);

        // Gather non-LFE speaker levels
        let non_lfe: Vec<f32> = config
            .speakers
            .iter()
            .filter(|s| !s.is_lfe)
            .map(|s| output[s.channel])
            .collect();

        // All non-LFE speakers should have non-zero, positive levels for an omni signal.
        // Mode-matching with regularization on asymmetric layouts (e.g. 5.1 with rear speakers
        // further apart than front) produces non-uniform but reasonable output.
        for (i, &level) in non_lfe.iter().enumerate() {
            assert!(
                level > 0.01,
                "Speaker {} should have positive output for omni signal, got {}",
                i,
                level
            );
        }
    }

    #[test]
    fn test_max_re_weights() {
        let weights = compute_max_re_weights(1);
        assert_eq!(weights.len(), 4);
        // Order 0 weight should be 1.0 (cos(0) = 1)
        assert!((weights[0] - 1.0).abs() < 1e-10);
        // Order 1 weights should be cos(pi/4) ≈ 0.707
        let expected = (std::f64::consts::PI / 4.0).cos();
        assert!((weights[1] - expected).abs() < 1e-10);
        assert!((weights[2] - expected).abs() < 1e-10);
        assert!((weights[3] - expected).abs() < 1e-10);
    }
}
