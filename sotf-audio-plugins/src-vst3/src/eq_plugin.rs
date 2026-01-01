//! SOTF EQ Plugin Implementation
//!
//! A 4-band parametric equalizer using nih-plug and autoeq_iir biquad filters.

use crate::editor::{create_editor, create_egui_state};
use crate::eq_params::{FilterType, SotfEqParams, NUM_BANDS};
use autoeq_iir::{Biquad, BiquadFilterType};
use nih_plug::prelude::*;
use nih_plug_egui::EguiState;
use std::sync::Arc;

/// SOTF Parametric EQ Plugin
pub struct SotfEq {
    /// Plugin parameters
    params: Arc<SotfEqParams>,

    /// Egui state for the editor
    egui_state: Arc<EguiState>,

    /// Biquad filters for each band and channel
    /// Indexed as [band][channel]
    filters: [[Biquad; 2]; NUM_BANDS],

    /// Current sample rate
    sample_rate: f32,
}

impl Default for SotfEq {
    fn default() -> Self {
        Self {
            params: Arc::new(SotfEqParams::default()),
            egui_state: create_egui_state(),
            filters: std::array::from_fn(|_| {
                std::array::from_fn(|_| Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 0.0))
            }),
            sample_rate: 48000.0,
        }
    }
}

impl SotfEq {
    /// Convert our FilterType enum to autoeq_iir's BiquadFilterType
    fn convert_filter_type(filter_type: FilterType) -> BiquadFilterType {
        match filter_type {
            FilterType::Peak => BiquadFilterType::Peak,
            FilterType::LowShelf => BiquadFilterType::Lowshelf,
            FilterType::HighShelf => BiquadFilterType::Highshelf,
            FilterType::LowPass => BiquadFilterType::Lowpass,
            FilterType::HighPass => BiquadFilterType::Highpass,
        }
    }

    /// Update filter coefficients for a specific band
    fn update_filter(&mut self, band_index: usize) {
        let band = self.params.band(band_index);
        let filter_type = Self::convert_filter_type(band.filter_type.value());
        let frequency = band.frequency.value() as f64;
        let q = band.q.value() as f64;
        let gain_db = band.gain_db.value() as f64;

        for channel in 0..2 {
            self.filters[band_index][channel] = Biquad::new(
                filter_type,
                frequency,
                self.sample_rate as f64,
                q,
                gain_db,
            );
        }
    }

    /// Update all filter coefficients
    fn update_all_filters(&mut self) {
        for band_index in 0..NUM_BANDS {
            self.update_filter(band_index);
        }
    }
}

impl Plugin for SotfEq {
    const NAME: &'static str = "SOTF EQ";
    const VENDOR: &'static str = "SOTF";
    const URL: &'static str = "https://github.com/pierreaubert/sotf";
    const EMAIL: &'static str = "pierre@spinorama.org";
    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    // Support stereo audio
    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const MIDI_OUTPUT: MidiConfig = MidiConfig::None;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        create_editor(self.params.clone(), self.egui_state.clone())
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sample_rate = buffer_config.sample_rate;
        self.update_all_filters();
        true
    }

    fn reset(&mut self) {
        // Reset filter state by recreating them
        self.update_all_filters();
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        // Check if any parameters changed and update filters
        // In a more optimized version, we'd track parameter changes
        self.update_all_filters();

        // Get output gain as linear amplitude
        let output_gain = util::db_to_gain(self.params.output_gain.value());

        // Process each sample
        for channel_samples in buffer.iter_samples() {
            let num_channels = channel_samples.len().min(2);

            for (channel_idx, sample) in channel_samples.into_iter().take(num_channels).enumerate()
            {
                let mut value = *sample as f64;

                // Apply each enabled band's filter
                for band_index in 0..NUM_BANDS {
                    let band = self.params.band(band_index);
                    if band.enabled.value() {
                        value = self.filters[band_index][channel_idx].process(value);
                    }
                }

                // Apply output gain
                *sample = (value as f32) * output_gain;
            }
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for SotfEq {
    const CLAP_ID: &'static str = "org.spinorama.sotf-eq";
    const CLAP_DESCRIPTION: Option<&'static str> =
        Some("4-band parametric equalizer with graphical visualization");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[
        ClapFeature::AudioEffect,
        ClapFeature::Stereo,
        ClapFeature::Mono,
        ClapFeature::Equalizer,
    ];
}

impl Vst3Plugin for SotfEq {
    const VST3_CLASS_ID: [u8; 16] = *b"SOTFEq4BandV001\0";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Fx, Vst3SubCategory::Eq];
}
