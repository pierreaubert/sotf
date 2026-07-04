use std::sync::{Mutex, OnceLock};

const EQ_CURVE_POINT_COUNT: usize = 240;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct EqCurveRenderData {
    pub combined_response: Vec<f64>,
    pub band_responses: Vec<Vec<f64>>,
}

#[derive(Debug)]
pub struct EqCurveRenderCache {
    signature: String,
    data: EqCurveRenderData,
}

impl EqCurveRenderCache {
    pub fn empty() -> Self {
        Self {
            signature: String::new(),
            data: EqCurveRenderData::default(),
        }
    }

    pub fn get_or_build<F, B>(
        &mut self,
        signature: String,
        freq_points: &[f64],
        band_count: usize,
        mut combined_response_at: F,
        mut band_response_at: B,
    ) -> EqCurveRenderData
    where
        F: FnMut(f64) -> f64,
        B: FnMut(usize, f64) -> f64,
    {
        if self.signature != signature {
            self.data = EqCurveRenderData {
                combined_response: freq_points
                    .iter()
                    .map(|&freq| combined_response_at(freq))
                    .collect(),
                band_responses: (0..band_count)
                    .map(|band_idx| {
                        freq_points
                            .iter()
                            .map(|&freq| band_response_at(band_idx, freq))
                            .collect()
                    })
                    .collect(),
            };
            self.signature = signature;
        }

        self.data.clone()
    }
}

pub fn eq_frequency_points() -> &'static [f64] {
    static EQ_FREQUENCY_POINTS: OnceLock<Vec<f64>> = OnceLock::new();
    EQ_FREQUENCY_POINTS
        .get_or_init(|| {
            let log_min = sotf_host::AUDIBLE_MIN_FREQ.ln();
            let log_max = sotf_host::AUDIBLE_MAX_FREQ.ln();

            (0..EQ_CURVE_POINT_COUNT)
                .map(|i| {
                    let t = i as f64 / (EQ_CURVE_POINT_COUNT - 1) as f64;
                    (log_min + t * (log_max - log_min)).exp()
                })
                .collect()
        })
        .as_slice()
}

pub fn eq_curve_cache() -> &'static Mutex<EqCurveRenderCache> {
    static EQ_CURVE_RENDER_CACHE: OnceLock<Mutex<EqCurveRenderCache>> = OnceLock::new();
    EQ_CURVE_RENDER_CACHE.get_or_init(|| Mutex::new(EqCurveRenderCache::empty()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    #[test]
    fn eq_frequency_points_reuses_static_grid() {
        let first = eq_frequency_points();
        let second = eq_frequency_points();

        assert_eq!(first.len(), EQ_CURVE_POINT_COUNT);
        assert_eq!(first.as_ptr(), second.as_ptr());
        assert!(first.windows(2).all(|pair| pair[0] < pair[1]));
    }

    #[test]
    fn eq_curve_cache_rebuilds_only_when_signature_changes() {
        let mut cache = EqCurveRenderCache::empty();
        let freqs = [20.0, 100.0, 1_000.0];
        let builds = Cell::new(0usize);

        let first = cache.get_or_build(
            "a".to_string(),
            &freqs,
            2,
            |freq| {
                builds.set(builds.get() + 1);
                freq
            },
            |band, freq| freq + band as f64,
        );
        let second = cache.get_or_build(
            "a".to_string(),
            &freqs,
            2,
            |freq| {
                builds.set(builds.get() + 1);
                freq * 2.0
            },
            |band, freq| freq * 2.0 + band as f64,
        );
        let third = cache.get_or_build(
            "b".to_string(),
            &freqs,
            2,
            |freq| {
                builds.set(builds.get() + 1);
                freq * 3.0
            },
            |band, freq| freq * 3.0 + band as f64,
        );

        assert_eq!(first, second);
        assert_ne!(second, third);
        assert_eq!(builds.get(), freqs.len() * 2);
    }
}
