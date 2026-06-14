use super::PluginFuzzer;
use rand::Rng;
use rand::rngs::StdRng;
use sotf_plugins::{BinauralDecoderPlugin, Plugin, RoomModel};

pub(super) struct BinauralFuzzer {
    pub(super) _sample_rate: u32,
}

impl PluginFuzzer for BinauralFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        let externalization = rng.random_range(0.0..1.0);
        let near_field_strength = rng.random_range(0.0..1.0);
        let diffuse_field_eq = rng.random_bool(0.5);
        let fft_size = 1024;

        let plugin = BinauralDecoderPlugin::new(
            channels,
            fft_size,
            None, // hrtf_path
            true, // enable_optimization
            externalization,
            near_field_strength,
            diffuse_field_eq,
            120.0, // lfe_crossover
            2.0,   // lfe_distance
            1.0,   // lfe_level
            RoomModel::default(),
        );

        let desc = format!(
            "ext={:.2} near={:.2} dfeq={} fft={}",
            externalization, near_field_strength, diffuse_field_eq, fft_size
        );

        (Box::new(plugin), desc)
    }
}
