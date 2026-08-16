// ============================================================================
// Ambisonics Decode Matrix Builders
// ============================================================================
//
// The legacy mode-matching path uses a directly regularized pseudoinverse of
// the physical speaker spherical-harmonic matrix.  The AllRAD path first
// decodes to a deterministic virtual sphere and then remaps every virtual
// speaker to the physical layout with setup-time VBAP.  Both paths produce a
// fixed matrix, so process() has identical realtime behavior.

use crate::spherical_harmonics::{self, channel_count, deg_to_rad, spherical_harmonics_vector};
use nalgebra::DMatrix;
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
    /// Algorithm used to construct this matrix.
    pub algorithm: DecodeAlgorithm,
    /// Number of virtual speakers used by AllRAD, or zero for mode matching.
    pub virtual_speaker_count: usize,
    quality: DecodeQuality,
}

/// Decoder construction algorithm.  The enum is deliberately kept in the
/// matrix metadata so QA can distinguish a true virtual-speaker decode from
/// the compatible legacy direct solve.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecodeAlgorithm {
    ModeMatching,
    AllRad,
}

/// Deterministic virtual-sphere size used for each supported HOA order.
/// The grid is a Fibonacci sphere, which has no pole singularity and provides
/// full-sphere coverage for VBAP remapping.
pub const ALLRAD_VIRTUAL_SPEAKERS: &[usize] = &[0, 64, 96, 128];

#[derive(Debug, Clone, Copy)]
pub struct DecodeQuality {
    pub rank: usize,
    pub largest_singular_value: f64,
    pub smallest_retained_singular_value: f64,
    pub condition_number: f64,
    pub reconstruction_error: f64,
    pub peak_coefficient: f64,
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

    /// Build a true AllRAD decoder: virtual-sphere mode matching followed by
    /// setup-time VBAP projection to the physical layout.
    pub fn build_allrad(
        order: usize,
        speaker_config: &SpeakerConfig,
        apply_max_re: bool,
    ) -> Result<Self, String> {
        validate_order(order)?;
        let ambi_ch = channel_count(order);
        let physical: Vec<_> = speaker_config
            .speakers
            .iter()
            .filter(|speaker| !speaker.is_lfe)
            .collect();
        if physical.is_empty() {
            return Err("No non-LFE speakers in config".into());
        }

        let virtual_count = ALLRAD_VIRTUAL_SPEAKERS
            .get(order)
            .copied()
            .ok_or_else(|| format!("No AllRAD grid for order {order}"))?;
        let directions = fibonacci_sphere(virtual_count);

        // D_virtual = Y_virtual (Y_virtual^T Y_virtual)^-1.  The regularized
        // SVD implementation is shared with the legacy path and is performed
        // only during construction.
        let mut virtual_y = vec![0.0_f64; virtual_count * ambi_ch];
        let mut sh = vec![0.0_f64; ambi_ch];
        for (index, direction) in directions.iter().enumerate() {
            spherical_harmonics_vector(order, direction.azimuth, direction.elevation, &mut sh);
            virtual_y[index * ambi_ch..(index + 1) * ambi_ch].copy_from_slice(&sh);
        }
        let (virtual_decode, mut quality) =
            mode_matching_decode(&virtual_y, virtual_count, ambi_ch)?;

        let max_re = if apply_max_re {
            compute_max_re_weights(order)
        } else {
            vec![1.0; ambi_ch]
        };
        for row in 0..virtual_count {
            for channel in 0..ambi_ch {
                virtual_y[row * ambi_ch + channel] =
                    virtual_decode[row * ambi_ch + channel] * max_re[channel];
            }
        }

        // P[speaker, virtual] is the setup-time VBAP remapping.  Its columns
        // are unit-energy panning vectors; LFE rows stay zero by construction.
        let mut physical_from_virtual =
            vec![0.0_f64; speaker_config.total_channels * virtual_count];
        let has_height = physical.iter().any(|speaker| speaker.elevation.abs() > 1.0);
        let physical_vectors: Vec<_> = physical
            .iter()
            .map(|speaker| (speaker.channel, speaker.to_cartesian()))
            .collect();
        for (virtual_index, direction) in directions.iter().enumerate() {
            let mut gains = vec![0.0_f64; speaker_config.total_channels];
            if has_height && direction.elevation.abs() > 1e-8 {
                vbap_3d(&physical_vectors, direction.vector, &mut gains);
            } else {
                vbap_2d(&physical_vectors, direction.azimuth, &mut gains);
            }
            for (channel, gain) in gains.into_iter().enumerate() {
                physical_from_virtual[channel * virtual_count + virtual_index] = gain;
            }
        }

        // Compose P * D_virtual into the same row-major representation used by
        // the legacy decoder.  The virtual-speaker stage is therefore explicit
        // in construction and inspectable through the metadata below.
        let mut matrix = vec![0.0_f32; speaker_config.total_channels * ambi_ch];
        for channel in 0..speaker_config.total_channels {
            for acn in 0..ambi_ch {
                let mut value = 0.0_f64;
                for virtual_index in 0..virtual_count {
                    value += physical_from_virtual[channel * virtual_count + virtual_index]
                        * virtual_y[virtual_index * ambi_ch + acn];
                }
                matrix[channel * ambi_ch + acn] = value as f32;
            }
        }

        quality.peak_coefficient = matrix
            .iter()
            .map(|value| value.abs() as f64)
            .fold(0.0, f64::max);
        if quality.peak_coefficient > 8.0 || !quality.peak_coefficient.is_finite() {
            return Err(format!(
                "AllRAD decode is ill-conditioned: peak coefficient {:.3} exceeds 8.0",
                quality.peak_coefficient
            ));
        }

        Ok(Self {
            ambi_channels: ambi_ch,
            speaker_count: speaker_config.total_channels,
            matrix,
            max_re_weights: max_re.into_iter().map(|weight| weight as f32).collect(),
            algorithm: DecodeAlgorithm::AllRad,
            virtual_speaker_count: virtual_count,
            quality,
        })
    }

    /// Build a decode matrix for the given Ambisonics order and target speaker layout.
    ///
    /// Uses mode-matching (pseudoinverse of Y matrix) with optional max-rE weighting.
    pub fn build(
        order: usize,
        speaker_config: &SpeakerConfig,
        apply_max_re: bool,
    ) -> Result<Self, String> {
        validate_order(order)?;
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
        let (decode, mut quality) = mode_matching_decode(&y_matrix, num_speakers, ambi_ch)?;

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
        quality.peak_coefficient = matrix
            .iter()
            .map(|value| value.abs() as f64)
            .fold(0.0, f64::max);
        if quality.peak_coefficient > 8.0 {
            return Err(format!(
                "Ambisonics decode is ill-conditioned: peak coefficient {:.3} exceeds 8.0 (rank {}/{})",
                quality.peak_coefficient, quality.rank, ambi_ch
            ));
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
            algorithm: DecodeAlgorithm::ModeMatching,
            virtual_speaker_count: 0,
            quality,
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

    pub fn quality(&self) -> DecodeQuality {
        self.quality
    }
}

fn validate_order(order: usize) -> Result<(), String> {
    if order > crate::spherical_harmonics::MAX_ORDER {
        return Err(format!(
            "Ambisonics order must be at most {}, got {order}",
            crate::spherical_harmonics::MAX_ORDER
        ));
    }
    Ok(())
}

#[derive(Clone, Copy)]
struct Direction {
    vector: [f64; 3],
    azimuth: f64,
    elevation: f64,
}

fn fibonacci_sphere(count: usize) -> Vec<Direction> {
    let golden_angle = std::f64::consts::PI * (3.0 - 5.0_f64.sqrt());
    (0..count)
        .map(|index| {
            let y = 1.0 - 2.0 * (index as f64 + 0.5) / count as f64;
            let radius = (1.0 - y * y).max(0.0).sqrt();
            let azimuth = (index as f64 * golden_angle).rem_euclid(2.0 * std::f64::consts::PI);
            let vector = [radius * azimuth.sin(), radius * azimuth.cos(), y];
            Direction {
                vector,
                azimuth: vector[0].atan2(vector[1]),
                elevation: vector[2].asin(),
            }
        })
        .collect()
}

fn vbap_2d(physical: &[(usize, [f32; 3])], source_azimuth: f64, gains: &mut [f64]) {
    if physical.is_empty() {
        return;
    }
    if physical.len() == 1 {
        gains[physical[0].0] = 1.0;
        return;
    }
    let mut entries: Vec<_> = physical
        .iter()
        .map(|&(channel, vector)| (channel, (vector[0] as f64).atan2(vector[1] as f64)))
        .collect();
    entries.sort_by(|a, b| a.1.total_cmp(&b.1));

    let mut best: Option<([usize; 2], [f64; 2], f64)> = None;
    for index in 0..entries.len() {
        let next = (index + 1) % entries.len();
        let first_azimuth = entries[index].1;
        let second_azimuth = entries[next].1;
        let span = (second_azimuth - first_azimuth).rem_euclid(2.0 * std::f64::consts::PI);
        if span <= 1e-8 || span > std::f64::consts::PI + 1e-8 {
            continue;
        }
        let offset = (source_azimuth - first_azimuth).rem_euclid(2.0 * std::f64::consts::PI);
        if offset > span + 1e-8 {
            continue;
        }
        let v1 = [first_azimuth.sin(), first_azimuth.cos()];
        let v2 = [second_azimuth.sin(), second_azimuth.cos()];
        let source = [source_azimuth.sin(), source_azimuth.cos()];
        let determinant = v1[0] * v2[1] - v2[0] * v1[1];
        if determinant.abs() < 1e-10 {
            continue;
        }
        let raw = [
            (source[0] * v2[1] - v2[0] * source[1]) / determinant,
            (v1[0] * source[1] - source[0] * v1[1]) / determinant,
        ];
        if raw[0] < -1e-7 || raw[1] < -1e-7 {
            continue;
        }
        let norm = (raw[0] * raw[0] + raw[1] * raw[1]).sqrt().max(1e-12);
        let candidate = [raw[0] / norm, raw[1] / norm];
        if best.is_none() || candidate[0].min(candidate[1]) > best.unwrap().2 {
            best = Some((
                [entries[index].0, entries[next].0],
                candidate,
                candidate[0].min(candidate[1]),
            ));
        }
    }

    if let Some((channels, candidate, _)) = best {
        gains[channels[0]] = candidate[0];
        gains[channels[1]] = candidate[1];
    } else {
        let nearest = physical
            .iter()
            .min_by(|(_, a), (_, b)| {
                let da =
                    1.0 - (a[0] as f64 * source_azimuth.sin() + a[1] as f64 * source_azimuth.cos());
                let db =
                    1.0 - (b[0] as f64 * source_azimuth.sin() + b[1] as f64 * source_azimuth.cos());
                da.total_cmp(&db)
            })
            .expect("physical is non-empty");
        gains[nearest.0] = 1.0;
    }
}

fn vbap_3d(physical: &[(usize, [f32; 3])], source: [f64; 3], gains: &mut [f64]) {
    if physical.len() < 3 {
        vbap_2d(physical, source[0].atan2(source[1]), gains);
        return;
    }
    let mut best: Option<([usize; 3], [f64; 3], f64)> = None;
    for i in 0..physical.len() {
        for j in (i + 1)..physical.len() {
            for k in (j + 1)..physical.len() {
                let matrix = [
                    [
                        physical[i].1[0] as f64,
                        physical[j].1[0] as f64,
                        physical[k].1[0] as f64,
                    ],
                    [
                        physical[i].1[1] as f64,
                        physical[j].1[1] as f64,
                        physical[k].1[1] as f64,
                    ],
                    [
                        physical[i].1[2] as f64,
                        physical[j].1[2] as f64,
                        physical[k].1[2] as f64,
                    ],
                ];
                let Some(raw) = solve_3x3(matrix, source) else {
                    continue;
                };
                if raw.iter().any(|gain| *gain < -1e-7) {
                    continue;
                }
                let norm = raw
                    .iter()
                    .map(|gain| gain * gain)
                    .sum::<f64>()
                    .sqrt()
                    .max(1e-12);
                let candidate = [raw[0] / norm, raw[1] / norm, raw[2] / norm];
                let score = candidate.iter().copied().fold(f64::INFINITY, f64::min);
                if best.is_none() || score > best.unwrap().2 {
                    best = Some((
                        [physical[i].0, physical[j].0, physical[k].0],
                        candidate,
                        score,
                    ));
                }
            }
        }
    }
    if let Some((channels, candidate, _)) = best {
        gains[channels[0]] = candidate[0];
        gains[channels[1]] = candidate[1];
        gains[channels[2]] = candidate[2];
        return;
    }
    let nearest = physical
        .iter()
        .max_by(|(_, a), (_, b)| {
            let da = a[0] as f64 * source[0] + a[1] as f64 * source[1] + a[2] as f64 * source[2];
            let db = b[0] as f64 * source[0] + b[1] as f64 * source[1] + b[2] as f64 * source[2];
            da.total_cmp(&db)
        })
        .expect("physical is non-empty");
    gains[nearest.0] = 1.0;
}

fn solve_3x3(matrix: [[f64; 3]; 3], rhs: [f64; 3]) -> Option<[f64; 3]> {
    let determinant = matrix[0][0] * (matrix[1][1] * matrix[2][2] - matrix[1][2] * matrix[2][1])
        - matrix[0][1] * (matrix[1][0] * matrix[2][2] - matrix[1][2] * matrix[2][0])
        + matrix[0][2] * (matrix[1][0] * matrix[2][1] - matrix[1][1] * matrix[2][0]);
    if determinant.abs() < 1e-10 {
        return None;
    }
    let mut result = [0.0; 3];
    for column in 0..3 {
        let mut replaced = matrix;
        for row in 0..3 {
            replaced[row][column] = rhs[row];
        }
        let det = replaced[0][0]
            * (replaced[1][1] * replaced[2][2] - replaced[1][2] * replaced[2][1])
            - replaced[0][1] * (replaced[1][0] * replaced[2][2] - replaced[1][2] * replaced[2][0])
            + replaced[0][2] * (replaced[1][0] * replaced[2][1] - replaced[1][1] * replaced[2][0]);
        result[column] = det / determinant;
    }
    Some(result)
}

/// Compute max-rE weights for a given Ambisonics order.
///
/// max-rE weighting maximises the energy concentration vector magnitude,
/// improving spatial resolution at the cost of some diffuseness.
///
/// For decoder order `N`, the exact degree weight is `P_l(r_E)`, where `r_E`
/// is the largest root of `P_(N+1)` and `P_l` is a Legendre polynomial.
fn compute_max_re_weights(order: usize) -> Vec<f64> {
    let degree_weights: &[f64] = match order {
        0 => &[1.0],
        1 => &[1.0, 0.577_350_269_189_625_8],
        2 => &[1.0, 0.774_596_669_241_483_4, 0.4],
        3 => &[
            1.0,
            0.861_136_311_594_052_6,
            0.612_333_620_718_713_8,
            0.304_746_984_955_207_9,
        ],
        _ => unreachable!("order must be validated against MAX_ORDER"),
    };
    let ambi_ch = channel_count(order);
    let mut weights = Vec::with_capacity(ambi_ch);
    for acn in 0..ambi_ch {
        let (l, _m) = spherical_harmonics::acn_to_degree_index(acn);
        weights.push(degree_weights[l as usize]);
    }
    weights
}

/// Compute the mode-matching decode matrix D [speakers × ambi_ch].
///
/// D = Y × (YᵀY + εI)⁻¹ where Y[s][n] = SH_n(speaker_s_position).
///
/// Tikhonov regularization (εI) handles rank-deficient layouts, e.g. 5.1 where
/// all speakers sit at elevation 0° making the Z-harmonic column zero.
fn mode_matching_decode(
    y: &[f64],
    rows: usize,
    cols: usize,
) -> Result<(Vec<f64>, DecodeQuality), String> {
    let y_matrix = DMatrix::from_row_slice(rows, cols, y);
    let svd = y_matrix.svd(true, true);
    let u = svd.u.ok_or("SVD did not produce U")?;
    let v_t = svd.v_t.ok_or("SVD did not produce V^T")?;
    let sigma_max = svd.singular_values.iter().copied().fold(0.0, f64::max);
    if !sigma_max.is_finite() || sigma_max <= f64::EPSILON {
        return Err("Speaker geometry has no usable spherical-harmonic rank".into());
    }
    let rank_threshold = sigma_max * 1e-7;
    let lambda = sigma_max * 1e-6;
    let rank = svd
        .singular_values
        .iter()
        .filter(|&&sigma| sigma > rank_threshold)
        .count();
    let sigma_min = svd
        .singular_values
        .iter()
        .copied()
        .filter(|&sigma| sigma > rank_threshold)
        .fold(f64::INFINITY, f64::min);

    // D = U diag(sigma/(sigma^2+lambda^2)) V^T.  This is a
    // rank-revealing, scale-relative Tikhonov pseudoinverse transpose and does
    // not square the geometry condition number through normal equations.
    let mut decode = vec![0.0; rows * cols];
    for s in 0..rows {
        for n in 0..cols {
            let mut sum = 0.0;
            for k in 0..svd.singular_values.len() {
                let sigma = svd.singular_values[k];
                if sigma > rank_threshold {
                    let inverse = sigma / (sigma * sigma + lambda * lambda);
                    sum += u[(s, k)] * inverse * v_t[(k, n)];
                }
            }
            decode[s * cols + n] = sum;
        }
    }

    let mut reconstruction_error = 0.0;
    for i in 0..cols {
        for j in 0..cols {
            let reconstructed = (0..rows)
                .map(|s| y[s * cols + i] * decode[s * cols + j])
                .sum::<f64>();
            let expected = if i == j { 1.0 } else { 0.0 };
            reconstruction_error += (reconstructed - expected).powi(2);
        }
    }
    reconstruction_error = (reconstruction_error / cols as f64).sqrt();

    Ok((
        decode,
        DecodeQuality {
            rank,
            largest_singular_value: sigma_max,
            smallest_retained_singular_value: sigma_min,
            condition_number: sigma_max / sigma_min,
            reconstruction_error,
            peak_coefficient: 0.0,
        },
    ))
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
    fn test_underdetermined_layout_reports_rank_loss() {
        let config = get_speaker_config("2.0").expect("2.0 config should exist");
        let matrix = DecodeMatrix::build(2, config, false).unwrap();
        assert!(matrix.quality().rank <= 2);
        assert!(matrix.matrix.iter().all(|value| value.is_finite()));
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
        let expected_by_order: &[&[f64]] = &[
            &[1.0, 1.0 / 3.0_f64.sqrt()],
            &[1.0, (3.0_f64 / 5.0).sqrt(), 0.4],
            &[
                1.0,
                0.861_136_311_594_052_6,
                0.612_333_620_718_713_8,
                0.304_746_984_955_207_9,
            ],
        ];

        for (order_index, expected_degrees) in expected_by_order.iter().enumerate() {
            let order = order_index + 1;
            let weights = compute_max_re_weights(order);
            assert_eq!(weights.len(), channel_count(order));
            for (acn, &weight) in weights.iter().enumerate() {
                let (degree, _) = spherical_harmonics::acn_to_degree_index(acn);
                assert!(
                    (weight - expected_degrees[degree as usize]).abs() < 1e-12,
                    "order={order}, acn={acn}, degree={degree}: {weight}"
                );
            }
        }
    }

    #[test]
    fn shipped_layouts_report_bounded_rank_revealing_quality() {
        for layout in crate::params::TARGET_LAYOUTS {
            let config = get_speaker_config(layout).unwrap();
            for order in 1..=crate::spherical_harmonics::MAX_ORDER {
                let dm = DecodeMatrix::build(order, config, true).unwrap();
                let quality = dm.quality();
                assert!(quality.rank > 0 && quality.rank <= dm.ambi_channels);
                assert!(quality.largest_singular_value.is_finite());
                assert!(quality.peak_coefficient.is_finite());
                assert!(
                    quality.peak_coefficient <= 8.0,
                    "{layout} order {order}: {quality:?}"
                );
                assert!(quality.reconstruction_error.is_finite());
            }
        }
    }

    #[test]
    fn underdetermined_toa_layout_uses_bounded_pseudoinverse() {
        let config = get_speaker_config("7.1.4").unwrap();
        let dm = DecodeMatrix::build(3, config, true).unwrap();
        assert_eq!(dm.ambi_channels, 16);
        assert!(dm.quality().rank < dm.ambi_channels);
        assert!(dm.matrix.iter().all(|value| value.is_finite()));
    }

    #[test]
    fn public_matrix_builder_rejects_unsupported_order_without_panicking() {
        let config = get_speaker_config("9.1.6").unwrap();
        assert!(
            DecodeMatrix::build(crate::spherical_harmonics::MAX_ORDER + 1, config, true).is_err()
        );
    }

    #[test]
    fn dense_directional_response_remains_finite_and_bounded() {
        let config = get_speaker_config("9.1.6").unwrap();
        let dm = DecodeMatrix::build(3, config, true).unwrap();
        let mut encoded = vec![0.0_f64; dm.ambi_channels];
        let mut output = vec![0.0_f32; dm.speaker_count];
        for az_deg in (-180..180).step_by(15) {
            for el_deg in (-75..=75).step_by(15) {
                spherical_harmonics_vector(
                    3,
                    deg_to_rad(az_deg as f64),
                    deg_to_rad(el_deg as f64),
                    &mut encoded,
                );
                let input: Vec<f32> = encoded.iter().map(|value| *value as f32).collect();
                dm.decode_frame(&input, &mut output);
                let energy: f32 = output.iter().map(|value| value * value).sum();
                assert!(energy.is_finite() && energy < 100.0);
            }
        }
    }

    #[test]
    fn allrad_contains_virtual_speaker_and_vbap_stages() {
        let config = get_speaker_config("7.1.4").unwrap();
        let matrix = DecodeMatrix::build_allrad(2, config, false).unwrap();
        assert_eq!(matrix.algorithm, DecodeAlgorithm::AllRad);
        assert_eq!(matrix.virtual_speaker_count, ALLRAD_VIRTUAL_SPEAKERS[2]);
        assert!(matrix.matrix.iter().all(|value| value.is_finite()));
        assert!(matrix.quality().peak_coefficient > 0.0);
    }

    #[test]
    fn allrad_omnidirectional_signal_reaches_every_non_lfe_speaker() {
        let config = get_speaker_config("7.1.4").unwrap();
        let matrix = DecodeMatrix::build_allrad(1, config, false).unwrap();
        let mut output = vec![0.0_f32; config.total_channels];
        matrix.decode_frame(&[1.0, 0.0, 0.0, 0.0], &mut output);
        for speaker in config.speakers.iter().filter(|speaker| !speaker.is_lfe) {
            assert!(
                output[speaker.channel] > 0.01,
                "omni output at {} was {}",
                speaker.label,
                output[speaker.channel]
            );
        }
        assert_eq!(output[3], 0.0); // LFE is never part of AllRAD.
    }

    #[test]
    fn allrad_front_direction_prefers_front_over_rear() {
        let config = get_speaker_config("5.1").unwrap();
        let matrix = DecodeMatrix::build_allrad(1, config, false).unwrap();
        let mut output = vec![0.0_f32; config.total_channels];
        let input = [1.0_f32, 0.0, 0.0, 1.0];
        matrix.decode_frame(&input, &mut output);
        let front_energy = output[0] * output[0] + output[1] * output[1] + output[2] * output[2];
        let rear_energy = output[4] * output[4] + output[5] * output[5];
        assert!(
            front_energy > rear_energy,
            "front={front_energy}, rear={rear_energy}"
        );
    }

    #[test]
    fn allrad_handles_irregular_and_underdetermined_layouts() {
        let config = get_speaker_config("2.0").unwrap();
        let matrix = DecodeMatrix::build_allrad(3, config, true).unwrap();
        assert_eq!(matrix.speaker_count, 2);
        assert!(matrix.matrix.iter().all(|value| value.is_finite()));
        assert!(matrix.quality().rank > 0);
    }

    #[test]
    fn allrad_dense_directional_sweep_is_finite_and_energy_bounded() {
        let config = get_speaker_config("9.1.6").unwrap();
        let matrix = DecodeMatrix::build_allrad(3, config, true).unwrap();
        let mut encoded = vec![0.0_f64; matrix.ambi_channels];
        let mut output = vec![0.0_f32; matrix.speaker_count];
        for azimuth in (-180..180).step_by(20) {
            for elevation in (-80..=80).step_by(20) {
                spherical_harmonics_vector(
                    3,
                    deg_to_rad(azimuth as f64),
                    deg_to_rad(elevation as f64),
                    &mut encoded,
                );
                let input: Vec<f32> = encoded.iter().map(|value| *value as f32).collect();
                matrix.decode_frame(&input, &mut output);
                let energy: f32 = output.iter().map(|value| value * value).sum();
                assert!(energy.is_finite() && energy < 100.0);
            }
        }
    }
}
