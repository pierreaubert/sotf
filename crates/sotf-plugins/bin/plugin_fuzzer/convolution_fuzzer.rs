use super::PluginFuzzer;
use rand::RngExt;
use rand::rngs::StdRng;
use sotf_plugins::{
    ConvolutionPlugin, ConvolutionPluginParams, ParametricInPlacePluginAdapter, Plugin,
};

pub(super) struct ConvolutionFuzzer {
    pub(super) sample_rate: u32,
}

impl PluginFuzzer for ConvolutionFuzzer {
    fn create_plugin(&self, channels: usize, rng: &mut StdRng) -> (Box<dyn Plugin>, String) {
        // Create a very short random IR
        let ir_len = 128;
        let path = std::env::temp_dir().join(format!(
            "sotf-convolution-fuzz-{}-{}.wav",
            std::process::id(),
            rng.random::<u64>()
        ));
        let data_len = (ir_len * 2) as u32;
        let mut wav = Vec::with_capacity(44 + data_len as usize);
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + data_len).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt \x10\0\0\0\x01\0\x01\0");
        wav.extend_from_slice(&self.sample_rate.to_le_bytes());
        wav.extend_from_slice(&(self.sample_rate * 2).to_le_bytes());
        wav.extend_from_slice(b"\x02\0\x10\0data");
        wav.extend_from_slice(&data_len.to_le_bytes());
        for _ in 0..ir_len {
            let sample = rng.random_range(-16_384_i16..=16_384_i16);
            wav.extend_from_slice(&sample.to_le_bytes());
        }
        std::fs::write(&path, wav).expect("write convolution fuzz IR");

        let params = ConvolutionPluginParams {
            ir_file: path.to_string_lossy().into_owned(),
            mix: rng.random_range(0.1..1.0),
            gain_db: rng.random_range(-12.0..0.0),
            use_nupc: true,
            zero_latency_head: false,
            head_taps: 128,
        };

        let plugin = ConvolutionPlugin::from_params(channels, self.sample_rate, params.clone())
            .expect("Failed to create ConvolutionPlugin");
        std::fs::remove_file(path).ok();

        let desc = format!("ir_len={} mix={:.2}", ir_len, params.mix);

        (Box::new(ParametricInPlacePluginAdapter::new(plugin)), desc)
    }
}
