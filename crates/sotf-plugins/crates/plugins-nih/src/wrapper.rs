//! Macro-based wrapper that generates nih-plug Plugin implementations for SOTF plugins.

#[doc(hidden)]
#[macro_export]
macro_rules! sotf_nih_sample_accurate {
    ("LinearPhaseEQ") => {
        true
    };
    ($other:literal) => {
        false
    };
}

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
        plugin_type: $plugin_type:tt,
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
            #[cfg(feature = "linear-phase-eq")]
            structural_fingerprint: u64,
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

                // If no ParamSpec params, or when LinearPhaseEQ needs its
                // dynamic band schema in addition to global specs, inspect a
                // temporary uninitialized instance on the control thread.
                if (infos.is_empty() || matches!($plugin_type, "LinearPhaseEQ"))
                    && let Ok(plugin) = plugins_bridge::create_plugin(
                        $plugin_type,
                        $channels,
                        48000,
                        if matches!($plugin_type, "LinearPhaseEQ") {
                            // Discover the complete preset-compatible band
                            // schema while the ParamSpec-owned num_filters
                            // entry retains its normal default.
                            r#"{"num_filters":10}"#
                        } else {
                            "{}"
                        },
                    )
                {
                    for param in plugin.parameters() {
                        if infos.iter().any(|info| info.id == param.id.as_str()) {
                            continue;
                        }
                        if let Some(info) = $crate::wrapper::bridged_info_from_parameter(&param) {
                            infos.push(info);
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
                    #[cfg(feature = "linear-phase-eq")]
                    structural_fingerprint: 0,
                }
            }
        }

        impl nih_plug::prelude::Plugin for $struct_name {
            const NAME: &'static str = $name;
            const VENDOR: &'static str = "SOTF / Spinorama";
            const URL: &'static str = "https://spinorama.org";
            const EMAIL: &'static str = "";
            const VERSION: &'static str = env!("CARGO_PKG_VERSION");
            // nih-plug splits process buffers at host automation boundaries
            // when this is enabled. The per-slice sync below therefore stamps
            // AsyncTimelinePlugin events at the exact absolute frame instead
            // of collapsing automation to the original callback start.
            const SAMPLE_ACCURATE_AUTOMATION: bool =
                $crate::sotf_nih_sample_accurate!($plugin_type);
            const AUDIO_IO_LAYOUTS: &'static [nih_plug::prelude::AudioIOLayout] =
                &[nih_plug::prelude::AudioIOLayout {
                    main_input_channels: std::num::NonZeroU32::new($channels),
                    main_output_channels: std::num::NonZeroU32::new($channels),
                    ..nih_plug::prelude::AudioIOLayout::const_default()
                }];

            type SysExMessage = ();
            type BackgroundTask = ();

            fn params(&self) -> std::sync::Arc<dyn nih_plug::prelude::Params> {
                self.params.clone()
            }

            fn initialize(
                &mut self,
                _audio_io_layout: &nih_plug::prelude::AudioIOLayout,
                buffer_config: &nih_plug::prelude::BufferConfig,
                context: &mut impl nih_plug::prelude::InitContext<Self>,
            ) -> bool {
                self.sample_rate = buffer_config.sample_rate as u32;
                let channels: usize = $channels;
                let max_frames = buffer_config.max_buffer_size as usize;

                let config = {
                    #[cfg(feature = "linear-phase-eq")]
                    {
                        if matches!($plugin_type, "LinearPhaseEQ") {
                            match self.params.linear_phase_eq_config_json() {
                                Ok(config) => config,
                                Err(error) => {
                                    log::error!(
                                        "Failed to build {} configuration: {error}",
                                        $plugin_type
                                    );
                                    return false;
                                }
                            }
                        } else {
                            "{}".to_string()
                        }
                    }
                    #[cfg(not(feature = "linear-phase-eq"))]
                    {
                        "{}".to_string()
                    }
                };

                match plugins_bridge::create_plugin(
                    $plugin_type,
                    channels,
                    self.sample_rate,
                    &config,
                ) {
                    Ok(mut plugin) => {
                        if matches!($plugin_type, "LinearPhaseEQ") {
                            plugin = match sotf_host::AsyncTimelinePlugin::new(
                                plugin,
                                self.sample_rate,
                                max_frames,
                            ) {
                                Ok(adapter) => Box::new(adapter),
                                Err(e) => {
                                    log::error!(
                                        "Failed to initialize {} adapter: {e}",
                                        $plugin_type
                                    );
                                    return false;
                                }
                            };
                        } else if let Err(e) = plugin.initialize(self.sample_rate) {
                            log::error!("Failed to initialize {}: {e}", $plugin_type);
                            return false;
                        }

                        let latency = match u32::try_from(plugin.latency_samples()) {
                            Ok(latency) => latency,
                            Err(_) => {
                                log::error!("{} latency does not fit the host ABI", $plugin_type);
                                return false;
                            }
                        };
                        context.set_latency_samples(latency);

                        self.interleaved_in = vec![0.0; max_frames * channels];
                        self.interleaved_out = vec![0.0; max_frames * channels];
                        #[cfg(feature = "linear-phase-eq")]
                        if matches!($plugin_type, "LinearPhaseEQ") {
                            self.structural_fingerprint = self.params.structural_fingerprint();
                        }
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

                #[cfg(feature = "linear-phase-eq")]
                if matches!($plugin_type, "LinearPhaseEQ") {
                    if self.params.structural_fingerprint() != self.structural_fingerprint {
                        // Structural state is hidden/non-automatable and is
                        // reconstructed by initialize(). If a host restores it
                        // while active, fail silent and request lifecycle
                        // reactivation instead of silently diverging or rebuilding
                        // and destroying plugin resources on the render thread.
                        for channel in buffer.as_slice() {
                            channel.fill(0.0);
                        }
                        return nih_plug::prelude::ProcessStatus::Error(
                            "Structural parameter state changed; reactivate plugin",
                        );
                    }
                }

                // Sync nih-plug params → SOTF plugin
                self.bridge
                    .sync_params_to_plugin(&self.params, plugin.as_mut());

                // Interleave input
                let channel_slices = buffer.as_slice();
                for frame in 0..num_frames {
                    for ch in 0..num_channels {
                        self.interleaved_in[frame * num_channels + ch] = channel_slices[ch][frame];
                    }
                }

                // Process
                let ctx = sotf_host::plugin::ProcessContext::new(self.sample_rate, num_frames);
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
                        channel_slices[ch][frame] = self.interleaved_out[frame * num_channels + ch];
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

/// Convert runtime plugin metadata to NIH's format without erasing integer,
/// boolean, or structural/realtime semantics.
pub fn bridged_info_from_parameter(
    parameter: &sotf_host::parameters::Parameter,
) -> Option<plugins_bridge::param_bridge::BridgedParamInfo> {
    use sotf_host::param_specs::UpdateMode;
    use sotf_host::parameters::ParameterValue;

    let (min_value, max_value, default_value, steps) = match (
        &parameter.min_value,
        &parameter.max_value,
        &parameter.default_value,
    ) {
        (
            Some(ParameterValue::Float(min)),
            Some(ParameterValue::Float(max)),
            ParameterValue::Float(default),
        ) => (*min as f64, *max as f64, *default as f64, 0),
        (
            Some(ParameterValue::Int(min)),
            Some(ParameterValue::Int(max)),
            ParameterValue::Int(default),
        ) => (
            *min as f64,
            *max as f64,
            *default as f64,
            u32::try_from(i64::from(*max) - i64::from(*min) + 1).ok()?,
        ),
        (None, None, ParameterValue::Bool(default)) => {
            (0.0, 1.0, if *default { 1.0 } else { 0.0 }, 1)
        }
        _ => return None,
    };
    Some(plugins_bridge::param_bridge::BridgedParamInfo {
        id: parameter.id.to_string(),
        name: parameter.name.clone(),
        unit: parameter.unit.clone(),
        min_value,
        max_value,
        default_value,
        steps,
        logarithmic: parameter.logarithmic,
        realtime: parameter.update_mode == UpdateMode::Realtime,
        group: parameter.group.clone(),
    })
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
        "FletcherMunson" => loudness_compensation::PARAMS,
        "LoudnessCompensation" => loudness_compensation::PARAMS,
        "MultibandCompressor" => multiband_compressor::GLOBAL_PARAMS,
        "MultibandExpander" => multiband_expander::GLOBAL_PARAMS,
        "Upmixer" => upmixer::PARAMS,
        "AAE" => aae::PARAMS,
        "XTC" => xtc::PARAMS,
        "Binaural" => binaural::PARAMS,
        "Denoiser" => denoiser::PARAMS,
        "SpeechDenoiser" => speech_denoiser::PARAMS,
        "HissReducer" => hiss_reducer::PARAMS,
        "Declick" => declick::PARAMS,
        "BandSplit" => band_split::PARAMS,
        "BandMerge" => band_merge::PARAMS,
        "AEC" => aec::PARAMS,
        "Beamformer" => beamformer::PARAMS,
        "LinearPhaseEQ" => linear_phase_eq::PARAMS,
        "SpectralCompressor" => spectral_compressor::PARAMS,
        "AmbisonicsDecoder" => ambisonics::PARAMS,
        _ => &[],
    }
}
