//! Macro-based wrapper that generates nih-plug Plugin implementations for SOTF plugins.

/// Generate a complete nih-plug plugin struct from SOTF plugin metadata.
///
/// This macro creates a struct that:
/// - Implements `nih_plug::Plugin`, `Vst3Plugin`, and `ClapPlugin`
/// - Creates the SOTF plugin via `plugins_bridge::create_plugin()`
/// - Handles interleave/deinterleave for buffer conversion
/// - Syncs nih-plug parameters to SOTF plugin parameters
#[macro_export]
macro_rules! sotf_nih_plugin {
    (
        $struct_name:ident,
        plugin_type: $plugin_type:literal,
        name: $name:literal,
        clap_id: $clap_id:literal,
        vst3_class_id: $vst3_id:expr,
        channels: $channels:literal
    ) => {
        pub struct $struct_name {
            params: std::sync::Arc<$crate::params::DynamicParams>,
            inner: Option<Box<dyn sotf_host::plugin::Plugin>>,
            bridge: $crate::PluginBridgeWrapper,
            interleaved_in: Vec<f32>,
            interleaved_out: Vec<f32>,
            sample_rate: u32,
        }

        impl Default for $struct_name {
            fn default() -> Self {
                let specs = $crate::wrapper::get_param_specs($plugin_type);
                let bridge_inner = plugins_bridge::param_bridge::ParamBridge::new(specs);

                // Build param infos for DynamicParams
                let mut infos = Vec::new();
                for i in 0..bridge_inner.count() {
                    if let Some(info) = bridge_inner.info(i) {
                        infos.push(info);
                    }
                }

                // If no ParamSpec params, try creating a temp plugin for its parameters()
                if infos.is_empty() {
                    if let Ok(plugin) = plugins_bridge::create_plugin(
                        $plugin_type,
                        $channels,
                        48000,
                        "{}",
                    ) {
                        for param in plugin.parameters() {
                            let (min, max, default) = match (
                                &param.min_value,
                                &param.max_value,
                                &param.default_value,
                            ) {
                                (
                                    Some(sotf_host::parameters::ParameterValue::Float(min)),
                                    Some(sotf_host::parameters::ParameterValue::Float(max)),
                                    sotf_host::parameters::ParameterValue::Float(def),
                                ) => (*min as f64, *max as f64, *def as f64),
                                _ => (0.0, 1.0, 0.0),
                            };

                            infos.push(plugins_bridge::param_bridge::BridgedParamInfo {
                                id: param.id.0.clone(),
                                name: param.name.clone(),
                                unit: param.unit.clone(),
                                min_value: min,
                                max_value: max,
                                default_value: default,
                                steps: 0,
                                logarithmic: param.logarithmic,
                                group: String::new(),
                            });
                        }
                    }
                }

                Self {
                    params: $crate::params::DynamicParams::from_infos(&infos),
                    inner: None,
                    bridge: $crate::PluginBridgeWrapper::new(bridge_inner),
                    interleaved_in: Vec::new(),
                    interleaved_out: Vec::new(),
                    sample_rate: 48000,
                }
            }
        }

        impl nih_plug::prelude::Plugin for $struct_name {
            const NAME: &'static str = $name;
            const VENDOR: &'static str = "SOTF / Spinorama";
            const URL: &'static str = "https://spinorama.org";
            const EMAIL: &'static str = "";
            const VERSION: &'static str = env!("CARGO_PKG_VERSION");
            const AUDIO_IO_LAYOUTS: &'static [nih_plug::prelude::AudioIOLayout] = &[
                nih_plug::prelude::AudioIOLayout {
                    main_input_channels: std::num::NonZeroU32::new($channels),
                    main_output_channels: std::num::NonZeroU32::new($channels),
                    ..nih_plug::prelude::AudioIOLayout::const_default()
                },
            ];

            type SysExMessage = ();
            type BackgroundTask = ();

            fn params(&self) -> std::sync::Arc<dyn nih_plug::prelude::Params> {
                self.params.clone()
            }

            fn initialize(
                &mut self,
                _audio_io_layout: &nih_plug::prelude::AudioIOLayout,
                buffer_config: &nih_plug::prelude::BufferConfig,
                _context: &mut impl nih_plug::prelude::InitContext<Self>,
            ) -> bool {
                self.sample_rate = buffer_config.sample_rate as u32;
                let channels: usize = $channels;

                match plugins_bridge::create_plugin(
                    $plugin_type,
                    channels,
                    self.sample_rate,
                    "{}",
                ) {
                    Ok(mut plugin) => {
                        if let Err(e) = plugin.initialize(self.sample_rate) {
                            log::error!("Failed to initialize {}: {e}", $plugin_type);
                            return false;
                        }

                        let max_frames = buffer_config.max_buffer_size as usize;
                        self.interleaved_in = vec![0.0; max_frames * channels];
                        self.interleaved_out = vec![0.0; max_frames * channels];
                        self.inner = Some(plugin);
                        true
                    }
                    Err(e) => {
                        log::error!("Failed to create {}: {e}", $plugin_type);
                        false
                    }
                }
            }

            fn process(
                &mut self,
                buffer: &mut nih_plug::prelude::Buffer,
                _aux: &mut nih_plug::prelude::AuxiliaryBuffers,
                _context: &mut impl nih_plug::prelude::ProcessContext<Self>,
            ) -> nih_plug::prelude::ProcessStatus {
                let plugin = match self.inner.as_mut() {
                    Some(p) => p,
                    None => return nih_plug::prelude::ProcessStatus::Error("Not initialized"),
                };

                let num_frames = buffer.samples();
                let num_channels = buffer.channels();

                // Sync nih-plug params → SOTF plugin
                self.bridge.sync_params_to_plugin(&self.params, plugin.as_mut());

                // Interleave input
                let channel_slices = buffer.as_slice();
                for frame in 0..num_frames {
                    for ch in 0..num_channels {
                        self.interleaved_in[frame * num_channels + ch] =
                            channel_slices[ch][frame];
                    }
                }

                // Process
                let ctx = sotf_host::plugin::ProcessContext {
                    sample_rate: self.sample_rate,
                    num_frames,
                };
                if plugin
                    .process(
                        &self.interleaved_in[..num_frames * num_channels],
                        &mut self.interleaved_out[..num_frames * num_channels],
                        &ctx,
                    )
                    .is_err()
                {
                    return nih_plug::prelude::ProcessStatus::Error("Processing failed");
                }

                // Deinterleave output
                let channel_slices = buffer.as_slice();
                for frame in 0..num_frames {
                    for ch in 0..num_channels {
                        channel_slices[ch][frame] =
                            self.interleaved_out[frame * num_channels + ch];
                    }
                }

                nih_plug::prelude::ProcessStatus::Normal
            }

            fn reset(&mut self) {
                if let Some(plugin) = self.inner.as_mut() {
                    plugin.reset();
                }
            }
        }

        impl nih_plug::prelude::Vst3Plugin for $struct_name {
            const VST3_CLASS_ID: [u8; 16] = $vst3_id;
            const VST3_SUBCATEGORIES: &'static [nih_plug::prelude::Vst3SubCategory] =
                &[nih_plug::prelude::Vst3SubCategory::Fx];
        }

        impl nih_plug::prelude::ClapPlugin for $struct_name {
            const CLAP_ID: &'static str = $clap_id;
            const CLAP_FEATURES: &'static [nih_plug::prelude::ClapFeature] =
                &[nih_plug::prelude::ClapFeature::AudioEffect];
            const CLAP_DESCRIPTION: Option<&'static str> = None;
            const CLAP_MANUAL_URL: Option<&'static str> = None;
            const CLAP_SUPPORT_URL: Option<&'static str> = None;
        }
    };
}

/// Get ParamSpec array for a plugin type.
pub fn get_param_specs(plugin_type: &str) -> &'static [sotf_host::param_specs::ParamSpec] {
    use sotf_plugins::param_specs::*;

    match plugin_type {
        "EQ" => eq::GLOBAL_PARAMS,
        "Compressor" => compressor::PARAMS,
        "Limiter" => limiter::PARAMS,
        "Gate" => gate::PARAMS,
        "Gain" => gain::PARAMS,
        "Delay" => delay::PARAMS,
        "Expander" => expander::PARAMS,
        "Crossfeed" => crossfeed::PARAMS,
        "FletcherMunson" => fletcher_munson::PARAMS,
        "LoudnessCompensation" => loudness_compensation::PARAMS,
        "MultibandCompressor" => multiband_compressor::GLOBAL_PARAMS,
        "MultibandExpander" => multiband_expander::GLOBAL_PARAMS,
        "Upmixer" => upmixer::PARAMS,
        "XTC" => xtc::PARAMS,
        "Binaural" => binaural::PARAMS,
        _ => &[],
    }
}
