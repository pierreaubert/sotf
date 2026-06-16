#![allow(clippy::field_reassign_with_default)]
#[cfg(test)]
mod tests {
        use sotf_host::{ParametricInPlacePlugin, ParameterId, ParameterValue, ProcessContext};
    use sotf_plugin_denoiser::{DenoiserPlugin, DenoiserPluginParams};

    #[test]
    fn test_polyphonic_mode_activation() {
        let mut params = DenoiserPluginParams::default();
        params.polyphonic_detection = true;

        let mut denoiser = DenoiserPlugin::from_params(2, params);

        // Check if parameter is set correctly
        let param_val = denoiser.parametric_get_parameter(&ParameterId::from("polyphonic_detection"));
        assert_eq!(param_val, Some(ParameterValue::Bool(true)));

        // Initialize
        denoiser.initialize(44100).unwrap();

        // Process a silence buffer (should not crash)
        let mut buffer = vec![0.0; 2048 * 2]; // 2048 frames * 2 channels
        let context = ProcessContext::new(44100, 2048);

        // First run (latency fill)
        denoiser.process_in_place(&mut buffer, &context).unwrap();

        // Process a sine wave (signal)
        for i in 0..2048 {
            let t = i as f32 / 44100.0;
            let sine = (2.0 * std::f32::consts::PI * 440.0 * t).sin();
            buffer[i * 2] = sine;
            buffer[i * 2 + 1] = sine;
        }

        denoiser.process_in_place(&mut buffer, &context).unwrap();

        // Check monitoring data
        let data = denoiser.get_data();
        assert!(data.is_some());
    }
}
