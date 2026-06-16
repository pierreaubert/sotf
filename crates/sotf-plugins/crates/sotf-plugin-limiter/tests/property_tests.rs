// Property-based tests for sotf-plugin-limiter

use proptest::prelude::*;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;
use sotf_plugin_limiter::LimiterPlugin;

proptest! {
    #[test]
    fn process_finite_output(
        sample in -1.0f32..1.0f32,
        threshold in -20.0f32..0.0f32,
        release in 10.0f32..1000.0f32,
        lookahead in 0.0f32..20.0f32,
        _mix in 0.0f32..1.0f32,
        soft in prop::bool::ANY,
    ) {
        let mut p = LimiterPlugin::new(1, threshold, release, lookahead, soft);
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
    fn no_nan_with_random_settings(
        sample in -1.0f32..1.0f32,
        threshold in -20.0f32..0.0f32,
        release in 10.0f32..1000.0f32,
        lookahead in 0.0f32..20.0f32,
        mix in 0.0f32..1.0f32,
        soft in prop::bool::ANY,
        true_peak in prop::bool::ANY,
        feed_forward in prop::bool::ANY,
    ) {
        let mut p = LimiterPlugin::new(1, threshold, release, lookahead, soft);
        p.initialize(48000).unwrap();
        p.set_parameter(ParameterId::from("true_peak"), ParameterValue::Bool(true_peak))
            .unwrap();
        p.set_parameter(ParameterId::from("feed_forward"), ParameterValue::Bool(feed_forward))
            .unwrap();
        p.set_parameter(ParameterId::from("mix"), ParameterValue::Float(mix))
            .unwrap();

        let frames = 256usize;
        let mut buf = vec![sample; frames];
        let ctx = ProcessContext::new(48000, frames);
        p.process_in_place(&mut buf, &ctx).unwrap();
        prop_assert!(
            buf.iter().all(|o| !o.is_nan()),
            "process_in_place produced NaN output"
        );
    }

    #[test]
    fn parameter_roundtrip(
        threshold in -20.0f32..0.0f32,
        release in 10.0f32..1000.0f32,
        lookahead in 0.0f32..20.0f32,
        mix in 0.0f32..1.0f32,
    ) {
        let mut p = LimiterPlugin::new(1, -6.0, 50.0, 5.0, false);
        p.initialize(48000).unwrap();

        p.set_parameter(ParameterId::from("threshold"), ParameterValue::Float(threshold))
            .unwrap();
        let got = p
            .get_parameter(&ParameterId::from("threshold"))
            .unwrap()
            .as_float()
            .unwrap();
        prop_assert!((got - threshold).abs() < 1e-4, "threshold roundtrip drift");

        p.set_parameter(ParameterId::from("release"), ParameterValue::Float(release))
            .unwrap();
        let got = p
            .get_parameter(&ParameterId::from("release"))
            .unwrap()
            .as_float()
            .unwrap();
        prop_assert!((got - release).abs() < 1e-4, "release roundtrip drift");

        p.set_parameter(ParameterId::from("lookahead"), ParameterValue::Float(lookahead))
            .unwrap();
        let got = p
            .get_parameter(&ParameterId::from("lookahead"))
            .unwrap()
            .as_float()
            .unwrap();
        prop_assert!((got - lookahead).abs() < 1e-4, "lookahead roundtrip drift");

        p.set_parameter(ParameterId::from("mix"), ParameterValue::Float(mix))
            .unwrap();
        let got = p.get_parameter(&ParameterId::from("mix")).unwrap().as_float().unwrap();
        prop_assert!((got - mix).abs() < 1e-4, "mix roundtrip drift");
    }

    #[test]
    fn unity_mix_passthrough(sample in -1.0f32..1.0f32) {
        let mut p = LimiterPlugin::new(1, 0.0, 50.0, 0.0, false);
        p.initialize(48000).unwrap();
        p.set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.0))
            .unwrap();

        // Warm up the mix smoother to converge to 0
        let mut warmup = vec![0.0f32; 4800];
        let ctx_warm = ProcessContext::new(48000, warmup.len());
        p.process_in_place(&mut warmup, &ctx_warm).unwrap();

        let frames = 128usize;
        let mut buf = vec![sample; frames];
        let ctx = ProcessContext::new(48000, frames);
        p.process_in_place(&mut buf, &ctx).unwrap();

        // The limiter has a minimum 1-sample lookahead even when lookahead_ms=0.
        let max_error = buf[1..]
            .iter()
            .map(|o| (o - sample).abs())
            .fold(0.0f32, f32::max);
        prop_assert!(
            max_error < 1e-4,
            "mix=0 should pass signal through unchanged: max_error={}",
            max_error
        );
    }

    #[test]
    fn monotonic_threshold(
        sample in 0.4f32..0.6f32,
        th_low in -18.0f32..-12.0f32,
        th_high in -6.0f32..-2.0f32,
    ) {
        // Lower threshold -> lower ceiling -> more attenuation for a loud signal.
        let mut p_low = LimiterPlugin::new(1, th_low, 50.0, 0.0, false);
        p_low.initialize(48000).unwrap();
        let mut p_high = LimiterPlugin::new(1, th_high, 50.0, 0.0, false);
        p_high.initialize(48000).unwrap();

        let frames = 512usize;
        let mut buf_low = vec![sample; frames];
        let mut buf_high = vec![sample; frames];
        let ctx = ProcessContext::new(48000, frames);
        p_low.process_in_place(&mut buf_low, &ctx).unwrap();
        p_high.process_in_place(&mut buf_high, &ctx).unwrap();

        let rms_low = buf_low.iter().map(|x| x * x).sum::<f32>().sqrt() / (frames as f32).sqrt();
        let rms_high = buf_high.iter().map(|x| x * x).sum::<f32>().sqrt() / (frames as f32).sqrt();
        prop_assert!(
            rms_low <= rms_high + 1e-4,
            "lower threshold should attenuate more: low_rms={} high_rms={}",
            rms_low,
            rms_high
        );
    }
}
