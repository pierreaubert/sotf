//! Chroma feature extraction.
//!
//! Ported from bliss-audio chroma.rs — already pure Rust, no aubio dependency.
//! Computes 13 interval/triad features from the chromagram.

use super::utils::{hz_to_octs_inplace, normalize, stft};
use ndarray::{Array, Array1, Array2, Axis, Zip, arr1, arr2, s};
use oxiblas_ndarray::blas::{dot_view, matmul};

const WINDOW_SIZE: usize = 8192;
const MAX_VALUE: f32 = 1.0;
const MIN_VALUE: f32 = 0.0;
const MAX_L2_INTERVAL: f32 = 0.25;
const MAX_L2_TRIAD: f32 = 0.025;
const MAX_TRIAD_INTERVAL_RATIO: f32 = std::f32::consts::FRAC_PI_2;

/// Error type for chroma analysis.
#[derive(Debug, Clone)]
pub struct ChromaError(pub String);

impl std::fmt::Display for ChromaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "chroma error: {}", self.0)
    }
}

impl std::error::Error for ChromaError {}

/// Compute 13 chroma interval features from the full song samples.
///
/// Returns a Vec of 13 normalized features (6 interval classes + 4 triads + 2 L2 norms + 1 ratio).
pub fn compute_chroma_features(samples: &[f32], sample_rate: u32) -> Result<Vec<f32>, ChromaError> {
    let n_chroma = 12u32;

    let mut spectrum = stft(samples, WINDOW_SIZE, 2205);
    let tuning = estimate_tuning(sample_rate, &spectrum, WINDOW_SIZE, 0.01, 12)?;
    let chroma = chroma_stft(sample_rate, &mut spectrum, WINDOW_SIZE, n_chroma, tuning)?;

    let mut raw_features = chroma_interval_features(&chroma)?;

    let (mut interval_class, mut interval_class_mode) =
        raw_features.view_mut().split_at(Axis(0), 6);

    let l2_norm_interval_class = dot_view(&interval_class.view(), &interval_class.view()).sqrt();
    let l2_norm_interval_class_mode =
        dot_view(&interval_class_mode.view(), &interval_class_mode.view()).sqrt();

    if l2_norm_interval_class > 0. {
        interval_class /= l2_norm_interval_class;
    }
    if l2_norm_interval_class_mode > 0. {
        interval_class_mode /= l2_norm_interval_class_mode;
    }

    let mut features: Vec<f32> = raw_features
        .mapv_into_any(|x| normalize(x as f32, MIN_VALUE, MAX_VALUE))
        .to_vec();

    let normalized_l2_norm_interval_class =
        (2. * (l2_norm_interval_class as f32 - 0.) / (MAX_L2_INTERVAL - 0.) - 1.).min(1.);
    features.push(normalized_l2_norm_interval_class);

    let normalized_l2_norm_interval_class_mode =
        (2. * (l2_norm_interval_class_mode as f32 - 0.) / (MAX_L2_TRIAD - 0.) - 1.).min(1.);
    features.push(normalized_l2_norm_interval_class_mode);

    let angle = (20. * l2_norm_interval_class_mode).atan2(l2_norm_interval_class + 1e-12_f64);
    let normalized_ratio = 2. * (angle as f32 - 0.) / (MAX_TRIAD_INTERVAL_RATIO - 0.) - 1.;
    features.push(normalized_ratio);

    Ok(features)
}

fn chroma_interval_features(chroma: &Array2<f64>) -> Result<Array1<f64>, ChromaError> {
    let chroma = normalize_feature_sequence(&chroma.mapv(|x| (x * 15.).exp()));
    let templates = arr2(&[
        [1, 1, 1, 1, 1, 1, 1, 1, 1, 1],
        [1, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 1, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 1, 0, 0, 0, 0, 1, 1, 0],
        [0, 0, 0, 1, 0, 0, 1, 0, 0, 1],
        [0, 0, 0, 0, 1, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 1, 0, 0, 1, 0],
        [0, 0, 0, 0, 0, 0, 1, 1, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 1],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
        [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
    ]);
    let interval_feature_matrix = extract_interval_features(&chroma, &templates);
    interval_feature_matrix.mean_axis(Axis(1)).ok_or_else(|| {
        ChromaError("Tried to run chroma on empty array. Need at least one sample.".to_string())
    })
}

fn extract_interval_features(chroma: &Array2<f64>, templates: &Array2<i32>) -> Array2<f64> {
    let mut f_intervals: Array2<f64> = Array::zeros((chroma.shape()[1], templates.shape()[1]));
    for (template, mut f_interval) in templates
        .axis_iter(Axis(1))
        .zip(f_intervals.axis_iter_mut(Axis(1)))
    {
        for shift in 0..12 {
            let mut vec: Vec<i32> = template.to_vec();
            vec.rotate_right(shift);
            let rolled = arr1(&vec);
            let power = Zip::from(chroma.t())
                .and_broadcast(&rolled)
                .map_collect(|&f, &s| f.powi(s))
                .map_axis_mut(Axis(1), |x| x.product());
            f_interval += &power;
        }
    }
    f_intervals.t().to_owned()
}

fn normalize_feature_sequence(feature: &Array2<f64>) -> Array2<f64> {
    let mut normalized_sequence = feature.to_owned();
    for mut column in normalized_sequence.columns_mut() {
        let mut sum = column.mapv(|x| x.abs()).sum();
        if sum < 0.0001 {
            sum = 1.;
        }
        column /= sum;
    }
    normalized_sequence
}

fn chroma_filter(
    sample_rate: u32,
    n_fft: usize,
    n_chroma: u32,
    tuning: f64,
) -> Result<Array2<f64>, ChromaError> {
    let ctroct = 5.0;
    let octwidth = 2.;
    let n_chroma_float = f64::from(n_chroma);
    let n_chroma2 = (n_chroma_float / 2.0).round() as u32;
    let n_chroma2_float = f64::from(n_chroma2);

    let frequencies = Array::linspace(0., f64::from(sample_rate), n_fft + 1);

    let mut freq_bins = frequencies;
    hz_to_octs_inplace(&mut freq_bins, tuning, n_chroma);
    freq_bins.mapv_inplace(|x| x * n_chroma_float);
    freq_bins[0] = freq_bins[1] - 1.5 * n_chroma_float;

    let mut binwidth_bins = Array::ones(freq_bins.raw_dim());
    binwidth_bins.slice_mut(s![0..freq_bins.len() - 1]).assign(
        &(&freq_bins.slice(s![1..]) - &freq_bins.slice(s![..-1]))
            .mapv(|x| if x <= 1. { 1. } else { x }),
    );

    let mut d: Array2<f64> = Array::zeros((n_chroma as usize, freq_bins.len()));
    for (idx, mut row) in d.rows_mut().into_iter().enumerate() {
        row.fill(idx as f64);
    }
    d = -d + &freq_bins;

    d.mapv_inplace(|x| {
        (x + n_chroma2_float + 10. * n_chroma_float) % n_chroma_float - n_chroma2_float
    });
    d = d / binwidth_bins;
    d.mapv_inplace(|x| (-0.5 * (2. * x) * (2. * x)).exp());

    let mut wts = d;
    for mut col in wts.columns_mut() {
        let mut sum = col.mapv(|x| x * x).sum().sqrt();
        if sum < f64::MIN_POSITIVE {
            sum = 1.;
        }
        col /= sum;
    }

    freq_bins.mapv_inplace(|x| (-0.5 * ((x / n_chroma_float - ctroct) / octwidth).powi(2)).exp());
    wts *= &freq_bins;

    // np.roll by -3
    let mut b = Array2::zeros(wts.dim());
    b.slice_mut(s![-3.., ..]).assign(&wts.slice(s![..3, ..]));
    b.slice_mut(s![..-3, ..]).assign(&wts.slice(s![3.., ..]));

    wts = b;
    let non_aliased = 1 + n_fft / 2;
    Ok(wts.slice_move(s![.., ..non_aliased]))
}

fn pip_track(
    sample_rate: u32,
    spectrum: &Array2<f64>,
    n_fft: usize,
) -> Result<(Vec<f64>, Vec<f64>), ChromaError> {
    let sample_rate_float = f64::from(sample_rate);
    let fmin = 150.0_f64;
    let fmax = 4000.0_f64.min(sample_rate_float / 2.0);
    let threshold = 0.1;

    let fft_freqs = Array::linspace(0., sample_rate_float / 2., 1 + n_fft / 2);

    let length = spectrum.len_of(Axis(0));

    let freq_mask: Vec<bool> = fft_freqs
        .iter()
        .map(|&f| (fmin <= f) && (f < fmax))
        .collect();

    let ref_value = spectrum.map_axis(Axis(0), |x| {
        let first: f64 = *x.first().expect("empty spectrum axis");
        x.fold(first, |acc, &elem| if acc > elem { acc } else { elem }) * threshold
    });

    let taken_columns = freq_mask.iter().filter(|&&x| x).count();
    let mut pitches = Vec::with_capacity(taken_columns * length);
    let mut mags = Vec::with_capacity(taken_columns * length);

    // Issue #5: guard against empty freq mask (e.g. very low sample rates
    // where the first FFT bin is already above fmax).
    let beginning = match freq_mask.iter().position(|&b| b) {
        Some(b) => b,
        None => return Ok((pitches, mags)),
    };
    let end = match freq_mask.iter().rposition(|&b| b) {
        Some(e) => e,
        None => return Ok((pitches, mags)),
    };

    let zipped = Zip::indexed(spectrum.slice(s![beginning..end - 3, ..]))
        .and(spectrum.slice(s![beginning + 1..end - 2, ..]))
        .and(spectrum.slice(s![beginning + 2..end - 1, ..]));

    zipped.for_each(|(i, j), &before_elem, &elem, &after_elem| {
        if elem > ref_value[j] && after_elem <= elem && before_elem < elem {
            let avg = 0.5 * (after_elem - before_elem);
            let mut shift = 2. * elem - after_elem - before_elem;
            if shift.abs() < f64::MIN_POSITIVE {
                shift += 1.;
            }
            shift = avg / shift;
            pitches.push(((i + beginning + 1) as f64 + shift) * sample_rate_float / n_fft as f64);
            mags.push(elem + 0.5 * avg * shift);
        }
    });

    Ok((pitches, mags))
}

fn pitch_tuning(
    frequencies: &mut Array1<f64>,
    resolution: f64,
    bins_per_octave: u32,
) -> Result<f64, ChromaError> {
    if frequencies.is_empty() {
        return Ok(0.0);
    }
    hz_to_octs_inplace(frequencies, 0.0, 12);
    frequencies.mapv_inplace(|x| f64::from(bins_per_octave) * x % 1.0);
    frequencies.mapv_inplace(|x| if x >= 0.5 { x - 1. } else { x });

    let indexes = ((frequencies.to_owned() - -0.5) / resolution).mapv(|x| x as usize);
    let mut counts: Array1<usize> = Array::zeros(((0.5 - -0.5) / resolution) as usize);
    for &idx in indexes.iter() {
        if idx < counts.len() {
            counts[idx] += 1;
        }
    }
    let max_index = counts
        .iter()
        .enumerate()
        .max_by_key(|&(_, v)| *v)
        .map(|(i, _)| i)
        .ok_or_else(|| ChromaError("empty counts in pitch_tuning".to_string()))?;

    Ok((-50. + (100. * resolution * max_index as f64)) / 100.)
}

fn estimate_tuning(
    sample_rate: u32,
    spectrum: &Array2<f64>,
    n_fft: usize,
    resolution: f64,
    bins_per_octave: u32,
) -> Result<f64, ChromaError> {
    let (pitch, mag) = pip_track(sample_rate, spectrum, n_fft)?;

    let (filtered_pitch, filtered_mag): (Vec<f64>, Vec<f64>) = pitch
        .iter()
        .zip(&mag)
        .filter(|&(&p, _)| p > 0.)
        .map(|(x, y)| (*x, *y))
        .unzip();

    if filtered_pitch.is_empty() {
        return Ok(0.);
    }

    // Compute median of magnitudes
    let mut sorted_mags = filtered_mag.clone();
    sorted_mags.sort_by(|a, b| a.total_cmp(b));
    let threshold = if sorted_mags.len() % 2 == 0 {
        (sorted_mags[sorted_mags.len() / 2 - 1] + sorted_mags[sorted_mags.len() / 2]) / 2.0
    } else {
        sorted_mags[sorted_mags.len() / 2]
    };

    let mut pitch_arr: Array1<f64> = filtered_pitch
        .iter()
        .zip(&filtered_mag)
        .filter_map(|(&p, &m)| if m >= threshold { Some(p) } else { None })
        .collect::<Vec<f64>>()
        .into();

    pitch_tuning(&mut pitch_arr, resolution, bins_per_octave)
}

fn chroma_stft(
    sample_rate: u32,
    spectrum: &mut Array2<f64>,
    n_fft: usize,
    n_chroma: u32,
    tuning: f64,
) -> Result<Array2<f64>, ChromaError> {
    spectrum.par_mapv_inplace(|x| x * x);
    let mut raw_chroma = chroma_filter(sample_rate, n_fft, n_chroma, tuning)?;

    raw_chroma = matmul(&raw_chroma, spectrum);
    for mut row in raw_chroma.columns_mut() {
        let mut sum = row.mapv(|x| x.abs()).sum();
        if sum < f64::MIN_POSITIVE {
            sum = 1.;
        }
        row /= sum;
    }
    Ok(raw_chroma)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chroma_features_length() {
        // Generate a simple tone
        let sr = 22050u32;
        let duration = 5.0;
        let n = (sr as f32 * duration) as usize;
        let signal: Vec<f32> = (0..n)
            .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / sr as f32).sin())
            .collect();

        let features = compute_chroma_features(&signal, sr).unwrap();
        assert_eq!(features.len(), 13);
    }

    #[test]
    fn test_normalize_feature_sequence_basic() {
        let array = arr2(&[[0.1, 0.3, 0.4, 0.], [1.1, 0.53, 1.01, 0.]]);
        let expected = arr2(&[
            [0.08333333, 0.36144578, 0.28368794, 0.],
            [0.91666667, 0.63855422, 0.71631206, 0.],
        ]);

        let normalized = normalize_feature_sequence(&array);

        for (expected, actual) in normalized.iter().zip(expected.iter()) {
            assert!((expected - actual).abs() < 1e-6);
        }
    }

    #[test]
    fn test_chroma_empty_freq_mask_low_sr() {
        // Issue #5: very low sample rates can produce an empty freq mask
        // because Nyquist is below fmin (150 Hz).  This should not panic.
        let sr = 200u32; // Nyquist = 100 Hz < fmin = 150 Hz
        // Need at least WINDOW_SIZE (8192) + 1 samples for reflect_pad.
        let signal = vec![0.5f32; 8200];
        let result = compute_chroma_features(&signal, sr);
        assert!(
            result.is_ok(),
            "low-SR chroma should not error: {:?}",
            result
        );
    }
}

#[test]
fn test_pitch_tuning_empty_returns_zero() {
    let mut freqs = ndarray::Array1::zeros((0,));
    let tuning = pitch_tuning(&mut freqs, 0.01, 12).unwrap();
    assert_eq!(tuning, 0.0);
}
