// Property-based tests for sotf-plugin-transient-shaper

use proptest::prelude::*;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePluginAdapter;
use sotf_host::plugin::{InPlacePlugin, ProcessContext};
use sotf_plugin_transient_shaper::{TransientShaperPlugin, TransientShaperPluginParams};

proptest! {
    #[test]
    fn process_finite_output(
        sample in -1.0f32..1.0f32,
        attack in -100.0f32..100.0f32,
        sustain in -100.0f32..100.0f32,
        sensitivity in -12.0f32..12.0f32,
        output_gain in -12.0f32..12.0f32,
        mix in 0.0f32..1.0f32,
    ) {
        let mut p = ParametricInPlacePluginAdapter::new(TransientShaperPlugin::new(1));
        p.initialize(48000).unwrap();
        p.set_parameter(ParameterId::from("attack"), ParameterValue::Float(attack))
            .unwrap();
        p.set_parameter(ParameterId::from("sustain"), ParameterValue::Float(sustain))
            .unwrap();
        p.set_parameter(ParameterId::from("sensitivity"), ParameterValue::Float(sensitivity))
            .unwrap();
        p.set_parameter(ParameterId::from("output_gain"), ParameterValue::Float(output_gain))
            .unwrap();
        p.set_parameter(ParameterId::from("mix"), ParameterValue::Float(mix))
            .unwrap();

        let frames = 256usize;
        let mut buf = vec![sample; frames];
        let ctx = ProcessContext::new(48000, frames);
        p.process_in_place(&mut buf, &ctx).unwrap();
        prop_assert!(
            buf.iter().all(|o| o.is_finite()),
            "process_in_place produced non-finite output"
        );
    }

    #[test]
    fn parameter_roundtrip(
        attack in -100.0f32..100.0f32,
        sustain in -100.0f32..100.0f32,
        sensitivity in -12.0f32..12.0f32,
        output_gain in -12.0f32..12.0f32,
        mix in 0.0f32..1.0f32,
    ) {
        let mut p = ParametricInPlacePluginAdapter::new(TransientShaperPlugin::new(1));
        p.initialize(48000).unwrap();

        p.set_parameter(ParameterId::from("attack"), ParameterValue::Float(attack))
            .unwrap();
        let got = p.get_parameter(&ParameterId::from("attack")).unwrap().as_float().unwrap();
        prop_assert!((got - attack).abs() < 1e-4, "attack roundtrip drift");

        p.set_parameter(ParameterId::from("sustain"), ParameterValue::Float(sustain))
            .unwrap();
        let got = p.get_parameter(&ParameterId::from("sustain")).unwrap().as_float().unwrap();
        prop_assert!((got - sustain).abs() < 1e-4, "sustain roundtrip drift");

        p.set_parameter(ParameterId::from("sensitivity"), ParameterValue::Float(sensitivity))
            .unwrap();
        let got = p
            .get_parameter(&ParameterId::from("sensitivity"))
            .unwrap()
            .as_float()
            .unwrap();
        prop_assert!((got - sensitivity).abs() < 1e-4, "sensitivity roundtrip drift");

        p.set_parameter(ParameterId::from("output_gain"), ParameterValue::Float(output_gain))
            .unwrap();
        let got = p
            .get_parameter(&ParameterId::from("output_gain"))
            .unwrap()
            .as_float()
            .unwrap();
        prop_assert!((got - output_gain).abs() < 1e-4, "output_gain roundtrip drift");

        p.set_parameter(ParameterId::from("mix"), ParameterValue::Float(mix))
            .unwrap();
        let got = p.get_parameter(&ParameterId::from("mix")).unwrap().as_float().unwrap();
        prop_assert!((got - mix).abs() < 1e-4, "mix roundtrip drift");
    }

    #[test]
    fn bypass_preserves_input(sample in -1.0f32..1.0f32) {
        let mut p = ParametricInPlacePluginAdapter::new(TransientShaperPlugin::from_validated_params(
            1,
            TransientShaperPluginParams {
                attack: 0.0,
                sustain: 0.0,
                sensitivity_db: 0.0,
                output_gain_db: 0.0,
                mix: 0.0,
            },
        ));
        p.initialize(48000).unwrap();

        // Warm up the mix smoother to converge to 0
        let mut warmup = vec![0.0f32; 4800];
        let ctx_warm = ProcessContext::new(48000, warmup.len());
        p.process_in_place(&mut warmup, &ctx_warm).unwrap();

        let frames = 128usize;
        let mut buf = vec![sample; frames];
        let ctx = ProcessContext::new(48000, frames);
        p.process_in_place(&mut buf, &ctx).unwrap();

        let max_error = buf.iter().map(|o| (o - sample).abs()).fold(0.0f32, f32::max);
        prop_assert!(
            max_error < 1e-4,
            "bypassed plugin should preserve input: max_error={}",
            max_error
        );
    }

    #[test]
    fn output_gain_monotonic(sample in 0.05f32..0.2f32, gain_db in 0.0f32..12.0f32) {
        let mut p_low = ParametricInPlacePluginAdapter::new(TransientShaperPlugin::from_validated_params(
            1,
            TransientShaperPluginParams {
                attack: 0.0,
                sustain: 0.0,
                sensitivity_db: -12.0,
                output_gain_db: 0.0,
                mix: 0.0,
            },
        ));
        p_low.initialize(48000).unwrap();

        let mut p_high = ParametricInPlacePluginAdapter::new(TransientShaperPlugin::from_validated_params(
            1,
            TransientShaperPluginParams {
                attack: 0.0,
                sustain: 0.0,
                sensitivity_db: -12.0,
                output_gain_db: gain_db,
                mix: 0.0,
            },
        ));
        p_high.initialize(48000).unwrap();

        let frames = 128usize;
        let mut buf_low = vec![sample; frames];
        let mut buf_high = vec![sample; frames];
        let ctx = ProcessContext::new(48000, frames);
        p_low.process_in_place(&mut buf_low, &ctx).unwrap();
        p_high.process_in_place(&mut buf_high, &ctx).unwrap();

        let rms_low = buf_low.iter().map(|x| x * x).sum::<f32>().sqrt() / (frames as f32).sqrt();
        let rms_high = buf_high.iter().map(|x| x * x).sum::<f32>().sqrt() / (frames as f32).sqrt();

        let expected_ratio = 10.0f32.powf(gain_db / 20.0);
        let actual_ratio = rms_high / (rms_low + 1e-12);
        prop_assert!(
            (actual_ratio - expected_ratio).abs() < 0.01,
            "output gain mismatch: expected ratio {} got {}",
            expected_ratio,
            actual_ratio
        );
    }
}
