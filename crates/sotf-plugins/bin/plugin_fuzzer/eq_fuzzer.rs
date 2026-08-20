use super::PluginFuzzer;
use rand::RngExt;
use rand::rngs::StdRng;
use sotf_plugins::{EqPlugin, EqPluginParams, ParametricPluginAdapter, Plugin};

pub(super) struct EqFuzzer {
    pub(super) sample_rate: u32,
}

const FILTER_GAIN_LIMIT_DB: f64 = 24.0;

fn compensated_gain(db_gain: f64, loudness_gain: f64) -> f64 {
    (db_gain - loudness_gain).clamp(-FILTER_GAIN_LIMIT_DB, FILTER_GAIN_LIMIT_DB)
}

impl PluginFuzzer for EqFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        use math_audio_iir_fir::{Biquad, BiquadFilterType, Peq, peq_loudness_gain};
        use sotf_plugins::BiquadFilterConfig;

        // Generate 1-5 random filters
        let num_filters = rng.random_range(1..=5);
        let mut filters = Vec::new();

        for _ in 0..num_filters {
            let filter_type = match rng.random_range(0..3) {
                0 => "peak",
                1 => "lowshelf",
                _ => "highshelf",
            };

            let freq = rng.random_range(20.0..20000.0);
            let q = rng.random_range(0.1..10.0);
            let db_gain = rng.random_range(-20.0..20.0);

            filters.push(BiquadFilterConfig {
                filter_type: filter_type.to_string(),
                freq,
                q,
                db_gain,
                order: 2,
                topology: Default::default(),
                lambda: None,
                kautz_sections: Vec::new(),
            });
        }

        // Convert to Biquad structs to calculate loudness gain
        let peq: Peq = filters
            .iter()
            .map(|f| {
                let filter_type = match f.filter_type.as_str() {
                    "peak" => BiquadFilterType::Peak,
                    "lowshelf" => BiquadFilterType::Lowshelf,
                    "highshelf" => BiquadFilterType::Highshelf,
                    _ => BiquadFilterType::Peak,
                };
                let biquad =
                    Biquad::new(filter_type, f.freq, self.sample_rate as f64, f.q, f.db_gain);
                (1.0, biquad)
            })
            .collect();

        // Calculate loudness gain and compensate
        let loudness_gain = peq_loudness_gain(&peq, "k");

        // Apply compensation by reducing all filter gains
        for filter in &mut filters {
            filter.db_gain = compensated_gain(filter.db_gain, loudness_gain);
        }

        // Build parameter description
        let mut desc = format!(
            "filters={} loudness_comp={:.2}dB [",
            filters.len(),
            loudness_gain
        );
        for (i, f) in filters.iter().enumerate() {
            if i > 0 {
                desc.push_str(", ");
            }
            desc.push_str(&format!(
                "{}:{:.0}Hz q={:.2} gain={:.2}dB",
                f.filter_type, f.freq, f.q, f.db_gain
            ));
        }
        desc.push(']');

        let params = EqPluginParams {
            filters,
            channel_filters: None,
            ..Default::default()
        };
        let plugin = EqPlugin::from_params(channels, self.sample_rate, params).unwrap();
        (Box::new(ParametricPluginAdapter::new(plugin)), desc)
    }
}

#[cfg(test)]
mod tests {
    use super::compensated_gain;

    #[test]
    fn loudness_compensation_keeps_filter_gain_in_eq_range() {
        assert_eq!(compensated_gain(20.0, -10.0), 24.0);
        assert_eq!(compensated_gain(-20.0, 10.0), -24.0);
        assert_eq!(compensated_gain(3.0, 1.0), 2.0);
    }
}
