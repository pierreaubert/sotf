//! Helpers for plugin integration tests.
//!
//! Requires the `plugin` feature.

use sotf_host::plugin::{InPlacePlugin, ProcessContext};

/// A tiny host fixture that records the output of a single in-place plugin.
#[derive(Debug)]
pub struct SinglePluginFixture<P> {
    plugin: P,
    sample_rate: u32,
}

impl<P: InPlacePlugin> SinglePluginFixture<P> {
    pub fn new(mut plugin: P, sample_rate: u32) -> Self {
        plugin.initialize(sample_rate).expect("initialize failed");
        Self {
            plugin,
            sample_rate,
        }
    }

    /// Process an interleaved buffer in-place and return the number of frames processed.
    pub fn process_in_place(&mut self, buffer: &mut [f32], frames: usize) -> usize {
        self.plugin
            .process_in_place(buffer, &ProcessContext::new(self.sample_rate, frames))
            .expect("process failed")
    }

    /// Convenience: process a mono or stereo buffer and assert all outputs are finite.
    pub fn process_finite(&mut self, buffer: &mut [f32], frames: usize) {
        let processed = self.process_in_place(buffer, frames);
        assert_eq!(processed, frames);
        assert!(
            buffer.iter().all(|s| s.is_finite()),
            "plugin produced non-finite samples"
        );
    }
}

/// Round-trip every parameter returned by `plugin.parameters()` through `set_parameter` and `get_parameter`.
pub fn roundtrip_all_parameters<P: InPlacePlugin>(plugin: &mut P, sample_rate: u32) {
    use sotf_host::parameters::ParameterId;

    plugin.initialize(sample_rate).expect("initialize failed");
    let param_ids: Vec<String> = plugin
        .parameters()
        .iter()
        .map(|p| p.id.to_string())
        .collect();

    for id in param_ids {
        let id = ParameterId::from(id.as_str());
        // Try to read the current value; if it fails, the parameter is not readable.
        if let Some(value) = plugin.get_parameter(&id) {
            // Set it back to the same value; should succeed for any legal value.
            plugin
                .set_parameter(id.clone(), value)
                .expect("round-trip of legal value failed");
        }
    }
}
