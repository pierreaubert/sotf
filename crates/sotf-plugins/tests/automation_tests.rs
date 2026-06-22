#![cfg(any(feature = "qa", debug_assertions))]

mod tests {
    use sotf_plugins::{
        CompressorPlugin, GainPlugin, ParameterId, ParametricInPlacePlugin,
        ParametricInPlacePluginAdapter, ParametricPlugin, ParametricPluginAdapter,
        test_parameter_ramp,
    };

    #[test]
    fn test_gain_automation_ramp() {
        let sample_rate = 48000.0;
        let mut inner = GainPlugin::new(2, 0.0);
        inner.plugin_initialize(sample_rate as u32).unwrap();
        let mut plugin = ParametricPluginAdapter::new(inner);

        // Ramp gain from 0dB to -24dB over 0.5 seconds
        test_parameter_ramp(
            &mut plugin,
            &ParameterId::from("gain_db"),
            0.0,
            -24.0,
            24000,
            sample_rate,
        );
    }

    #[test]
    fn test_compressor_threshold_automation_ramp() {
        let sample_rate = 48000.0;
        let mut inner = CompressorPlugin::new(2);
        inner.initialize(sample_rate as u32).unwrap();
        let mut plugin = ParametricInPlacePluginAdapter::new(inner);

        // Ramp threshold from 0dB down to -40dB
        test_parameter_ramp(
            &mut plugin,
            &ParameterId::from("threshold"),
            0.0,
            -40.0,
            24000,
            sample_rate,
        );
    }

    #[test]
    fn test_peq_gain_automation_ramp() {
        use math_audio_iir_fir::{Biquad, BiquadFilterType};
        let sample_rate = 48000.0;
        let f = vec![Biquad::new(
            BiquadFilterType::Peak,
            1000.0,
            sample_rate,
            1.0,
            0.0,
        )];
        let mut inner = sotf_plugins::EqPlugin::new(2, f);
        inner.plugin_initialize(sample_rate as u32).unwrap();
        let mut plugin = ParametricPluginAdapter::new(inner);

        // Ramp band 0 gain from 0dB to 12dB
        test_parameter_ramp(
            &mut plugin,
            &ParameterId::from("band_0_gain"),
            0.0,
            12.0,
            24000,
            sample_rate,
        );
    }
}
