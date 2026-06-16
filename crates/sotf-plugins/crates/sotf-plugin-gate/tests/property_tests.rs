// Property-based tests for sotf-plugin-gate

use proptest::prelude::*;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;
use sotf_plugin_gate::GatePlugin;

proptest! {
    #[test]
    fn process_finite_output(
        sample in -1.0f32..1.0f32,
        threshold in -80.0f32..0.0f32,
        ratio in 1.0f32..100.0f32,
        attack in 0.1f32..50.0f32,
        release in 10.0f32..500.0f32,
        _mix in 0.0f32..1.0f32,
        _range_db in 0.0f32..120.0f32,
    ) {
        let mut p = GatePlugin::new(1, threshold, ratio, attack, 0.0, release);
        p.initialize(48000).unwrap();
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
        threshold in -80.0f32..0.0f32,
        ratio in 1.0f32..100.0f32,
        attack in 0.1f32..50.0f32,
        release in 10.0f32..500.0f32,
        mix in 0.0f32..1.0f32,
    ) {
        let mut p = GatePlugin::new(1, -40.0, 10.0, 1.0, 0.0, 50.0);
        p.initialize(48000).unwrap();

        p.parametric_set_parameter(ParameterId::from("threshold"), ParameterValue::Float(threshold))
            .unwrap();
        let got = p
            .parametric_get_parameter(&ParameterId::from("threshold"))
            .unwrap()
            .as_float()
            .unwrap();
        prop_assert!((got - threshold).abs() < 1e-4, "threshold roundtrip drift");

        p.parametric_set_parameter(ParameterId::from("ratio"), ParameterValue::Float(ratio))
            .unwrap();
        let got = p.parametric_get_parameter(&ParameterId::from("ratio")).unwrap().as_float().unwrap();
        prop_assert!((got - ratio).abs() < 1e-4, "ratio roundtrip drift");

        p.parametric_set_parameter(ParameterId::from("attack"), ParameterValue::Float(attack))
            .unwrap();
        let got = p.parametric_get_parameter(&ParameterId::from("attack")).unwrap().as_float().unwrap();
        prop_assert!((got - attack).abs() < 1e-4, "attack roundtrip drift");

        p.parametric_set_parameter(ParameterId::from("release"), ParameterValue::Float(release))
            .unwrap();
        let got = p.parametric_get_parameter(&ParameterId::from("release")).unwrap().as_float().unwrap();
        prop_assert!((got - release).abs() < 1e-4, "release roundtrip drift");

        p.parametric_set_parameter(ParameterId::from("mix"), ParameterValue::Float(mix))
            .unwrap();
        let got = p.parametric_get_parameter(&ParameterId::from("mix")).unwrap().as_float().unwrap();
        prop_assert!((got - mix).abs() < 1e-4, "mix roundtrip drift");
    }

    #[test]
    fn unity_mix_passthrough(sample in -1.0f32..1.0f32) {
        let mut p = GatePlugin::new(1, -40.0, 10.0, 1.0, 0.0, 50.0);
        p.initialize(48000).unwrap();
        p.parametric_set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.0))
            .unwrap();

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
            "mix=0 should pass signal through unchanged: max_error={}",
            max_error
        );
    }

    #[test]
    fn monotonic_threshold(sample in 0.09f32..0.11f32) {
        // Signal at ~-20 dB. th_open (-30 dB) is below the signal (gate open),
        // th_close (-15 dB) is above (gate closed).
        let th_open = -30.0f32;
        let th_close = -15.0f32;

        let mut p_open = GatePlugin::new(1, th_open, 100.0, 1.0, 0.0, 10.0);
        p_open.initialize(48000).unwrap();
        let mut p_close = GatePlugin::new(1, th_close, 100.0, 1.0, 0.0, 10.0);
        p_close.initialize(48000).unwrap();

        let frames = 512usize;
        let mut buf_open = vec![sample; frames];
        let mut buf_close = vec![sample; frames];
        let ctx = ProcessContext::new(48000, frames);
        p_open.process_in_place(&mut buf_open, &ctx).unwrap();
        p_close.process_in_place(&mut buf_close, &ctx).unwrap();

        let rms_open = buf_open.iter().map(|x| x * x).sum::<f32>().sqrt() / (frames as f32).sqrt();
        let rms_close = buf_close.iter().map(|x| x * x).sum::<f32>().sqrt() / (frames as f32).sqrt();
        prop_assert!(
            rms_close < rms_open * 0.9,
            "closed gate should attenuate more than open gate: open_rms={} close_rms={}",
            rms_open,
            rms_close
        );
    }

    #[test]
    fn monotonic_range_db(sample in 0.0001f32..0.001f32) {
        let threshold = -40.0f32;

        let mut p_low = GatePlugin::new(1, threshold, 100.0, 1.0, 0.0, 10.0);
        p_low.initialize(48000).unwrap();
        p_low
            .parametric_set_parameter(ParameterId::from("range_db"), ParameterValue::Float(0.0))
            .unwrap();

        let mut p_high = GatePlugin::new(1, threshold, 100.0, 1.0, 0.0, 10.0);
        p_high.initialize(48000).unwrap();
        p_high
            .parametric_set_parameter(ParameterId::from("range_db"), ParameterValue::Float(120.0))
            .unwrap();

        let frames = 512usize;
        let mut buf_low = vec![sample; frames];
        let mut buf_high = vec![sample; frames];
        let ctx = ProcessContext::new(48000, frames);
        p_low.process_in_place(&mut buf_low, &ctx).unwrap();
        p_high.process_in_place(&mut buf_high, &ctx).unwrap();

        let rms_low = buf_low.iter().map(|x| x * x).sum::<f32>().sqrt() / (frames as f32).sqrt();
        let rms_high = buf_high.iter().map(|x| x * x).sum::<f32>().sqrt() / (frames as f32).sqrt();
        prop_assert!(
            rms_high < rms_low * 0.9,
            "larger range_db should attenuate quiet signal more: low_rms={} high_rms={}",
            rms_low,
            rms_high
        );
    }
}
