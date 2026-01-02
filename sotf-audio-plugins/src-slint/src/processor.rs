//! EQ Plugin Audio Processor
//!
//! Processes audio through 4-band parametric EQ using biquad filters.

use crate::parameters::{EqParameters, FilterType, NUM_BANDS};
use autoeq_iir::{Biquad, BiquadFilterType};
use plinth_plugin::plinth_core::signals::signal::{Signal, SignalMut};
use plinth_plugin::{Event, ProcessState, Processor, Transport};
use std::sync::Arc;

/// Maximum number of channels supported
const MAX_CHANNELS: usize = 2;

/// EQ Plugin Processor
pub struct EqProcessor {
    /// Plugin parameters
    parameters: Arc<EqParameters>,

    /// Biquad filters for each band and channel
    /// Indexed as [band][channel]
    filters: [[Biquad; MAX_CHANNELS]; NUM_BANDS],

    /// Current sample rate
    sample_rate: f64,
}

impl EqProcessor {
    pub fn new(parameters: Arc<EqParameters>, sample_rate: f64) -> Self {
        let filters = std::array::from_fn(|_| {
            std::array::from_fn(|_| {
                Biquad::new(BiquadFilterType::Peak, 1000.0, sample_rate, 1.0, 0.0)
            })
        });

        let mut processor = Self {
            parameters,
            filters,
            sample_rate,
        };

        // Initialize filters with current parameter values
        processor.update_all_filters();
        processor
    }

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
        let filter_type = Self::convert_filter_type(self.parameters.band_filter_type(band_index));
        let frequency = self.parameters.band_frequency(band_index);
        let q = self.parameters.band_q(band_index);
        let gain_db = self.parameters.band_gain(band_index);

        for channel in 0..MAX_CHANNELS {
            self.filters[band_index][channel] =
                Biquad::new(filter_type, frequency, self.sample_rate, q, gain_db);
        }
    }

    /// Update all filter coefficients
    fn update_all_filters(&mut self) {
        for band_index in 0..NUM_BANDS {
            self.update_filter(band_index);
        }
    }

    /// Convert dB to linear amplitude
    fn db_to_amplitude(db: f64) -> f64 {
        10.0_f64.powf(db / 20.0)
    }
}

impl Processor for EqProcessor {
    fn reset(&mut self) {
        // Reset filter state by recreating them
        self.update_all_filters();
    }

    fn process(
        &mut self,
        buffer: &mut impl SignalMut,
        _aux: Option<&impl Signal>,
        _transport: Option<Transport>,
        events: impl Iterator<Item = Event>,
    ) -> ProcessState {
        // Process parameter change events (currently unused, parameters auto-sync)
        for _event in events {
            // Parameters are updated via the parameter system
        }

        // Update filters (in a more optimized version, we'd track parameter changes)
        self.update_all_filters();

        // Get output gain as linear amplitude
        let output_gain = Self::db_to_amplitude(self.parameters.output_gain()) as f32;

        let num_channels = buffer.channels().min(MAX_CHANNELS);
        let num_frames = buffer.len();

        // Process each channel
        for channel_idx in 0..num_channels {
            let channel = buffer.channel_mut(channel_idx);

            for frame_idx in 0..num_frames {
                let mut value = channel[frame_idx] as f64;

                // Apply each enabled band's filter
                for band_index in 0..NUM_BANDS {
                    if self.parameters.band_enabled(band_index) {
                        value = self.filters[band_index][channel_idx].process(value);
                    }
                }

                // Apply output gain
                channel[frame_idx] = (value as f32) * output_gain;
            }
        }

        ProcessState::Normal
    }

    fn process_events(&mut self, events: impl Iterator<Item = Event>) {
        for _event in events {
            // Parameters are updated via the parameter system
        }
    }
}
