//! Integration tests for plugin-chain channel-count preservation.
//!
//! Verifies that the `PluginHost` / `DawHost` linear-chain builder correctly
//! propagates channel counts through a series of plugins, including chains that
//! intentionally change channel counts (upmix followed by downmix) and chains
//! that place a mismatched plugin at the end.

use sotf_plugins::factory::create_plugin;
use sotf_plugins::{
    CompressorPlugin, EqPlugin, GainPlugin, Host, LimiterPlugin, LoudnessData,
    LoudnessMonitorPlugin, ParametricInPlacePluginAdapter, ParametricPluginAdapter, PluginHost,
};

const SAMPLE_RATE: u32 = 48_000;
const FRAMES: usize = 512;

fn interleaved_sine(channels: usize, frames: usize) -> Vec<f32> {
    let mut buf = vec![0.0f32; frames * channels];
    for i in 0..frames {
        let t = i as f32 / SAMPLE_RATE as f32;
        let s = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.25;
        for ch in 0..channels {
            buf[i * channels + ch] = s;
        }
    }
    buf
}

#[test]
fn loudness_analyzer_tap_is_bit_transparent_inside_linear_chain() {
    let mut host = PluginHost::new(2, SAMPLE_RATE);
    host.add_plugin(Box::new(LoudnessMonitorPlugin::new(2).unwrap()))
        .unwrap();
    host.add_plugin(Box::new(ParametricPluginAdapter::new(GainPlugin::new(
        2, 0.0,
    ))))
    .unwrap();
    host.build().unwrap();

    let input = interleaved_sine(2, FRAMES);
    let mut output = vec![0.0; input.len()];
    assert_eq!(host.process(&input, &mut output).unwrap(), FRAMES);
    assert_eq!(output, input);

    let data = host.get_plugin_data(0).unwrap();
    let loudness = data.downcast_ref::<LoudnessData>().unwrap();
    assert!(loudness.sample_peak_valid);
}

fn assert_all_finite(buffer: &[f32], label: &str) {
    for (i, &s) in buffer.iter().enumerate() {
        assert!(
            s.is_finite(),
            "{label}: non-finite value at index {i} (value: {s})"
        );
    }
}

/// Build a chain of channel-preserving plugins and verify the final output
/// channel count equals the input channel count.
#[test]
fn channel_preserving_chain_maintains_count() {
    for &channels in &[1usize, 2, 6, 8] {
        let mut host = PluginHost::new(channels, SAMPLE_RATE);

        // EQ -> Compressor -> Gain -> Limiter
        let filters = vec![math_audio_iir_fir::Biquad::new(
            math_audio_iir_fir::BiquadFilterType::Peak,
            1000.0,
            SAMPLE_RATE as f64,
            1.0,
            3.0,
        )];
        let eq = ParametricPluginAdapter::new(EqPlugin::new(channels, filters));
        host.add_plugin(Box::new(eq)).unwrap();

        let compressor = ParametricInPlacePluginAdapter::new(CompressorPlugin::new(channels));
        host.add_plugin(Box::new(compressor)).unwrap();

        let gain = ParametricPluginAdapter::new(GainPlugin::new(channels, -3.0));
        host.add_plugin(Box::new(gain)).unwrap();

        let limiter = ParametricInPlacePluginAdapter::new(LimiterPlugin::new(
            channels, -1.0, 50.0, 5.0, false,
        ));
        host.add_plugin(Box::new(limiter)).unwrap();

        assert_eq!(
            host.input_channels(),
            channels,
            "chain input channels should match host creation count"
        );
        assert_eq!(
            host.output_channels(),
            channels,
            "channel-preserving chain should output the same number of channels"
        );

        let input = interleaved_sine(channels, FRAMES);
        let mut output = vec![0.0f32; FRAMES * channels];
        host.process(&input, &mut output)
            .expect("process should succeed");

        assert_all_finite(&output, &format!("preserving-chain@{channels}ch"));
        let energy: f32 = output.iter().map(|s| s * s).sum();
        assert!(
            energy > 0.0,
            "preserving-chain@{channels}ch output should not be silent"
        );
    }
}

/// Build a chain that upmixes from stereo to 5.1 and then downmixes back to
/// stereo, verifying the final output is 2 channels.
#[test]
fn upmix_then_downmix_returns_to_stereo() {
    let mut host = PluginHost::new(2, SAMPLE_RATE);

    // Upmixer: 2ch -> 6ch (default 5.1 config)
    let upmixer = create_plugin("upmixer", &serde_json::json!({}), 2, SAMPLE_RATE)
        .expect("upmixer should instantiate with stereo input");
    host.add_plugin(upmixer).unwrap();
    assert_eq!(
        host.output_channels(),
        6,
        "upmixer should output 6 channels"
    );

    // Downmix: 6ch -> 2ch
    let downmix = create_plugin(
        "downmix",
        &serde_json::json!({"input_channels": 6}),
        6,
        SAMPLE_RATE,
    )
    .expect("downmix should instantiate with 6 input channels");
    host.add_plugin(downmix).unwrap();
    assert_eq!(
        host.output_channels(),
        2,
        "final chain output should be stereo after downmix"
    );

    // STFT-based upmixers/downmixers have multi-frame latency. Use a long
    // block and process twice so the second block is past the lookahead.
    let frames = 16_384;
    let input = interleaved_sine(2, frames);
    let mut output = vec![0.0f32; frames * 2];
    host.process(&input, &mut output)
        .expect("upmix->downmix chain should process (warm-up)");

    let mut output2 = vec![0.0f32; frames * 2];
    let processed = host
        .process(&input, &mut output2)
        .expect("upmix->downmix chain should process");
    assert!(processed > 0, "upmix->downmix chain should produce frames");

    assert_all_finite(&output2, "upmix->downmix chain");
    let energy: f32 = output2.iter().map(|s| s * s).sum();
    assert!(
        energy > 0.0,
        "upmix->downmix chain output should not be silent"
    );
}

/// Decode first-order ACN/SN3D B-format to a 5.1 speaker bed, then render that
/// bed binaurally. This is the valid spatial counterpart to chaining an
/// upmixer into a binaural renderer: an ambisonics decoder consumes B-format,
/// not the speaker feeds produced by an upmixer.
#[test]
fn first_order_ambisonics_to_binaural_chain_is_finite_and_audible() {
    let mut host = PluginHost::new(4, SAMPLE_RATE);

    let ambisonics = create_plugin(
        "ambisonics_decoder",
        &serde_json::json!({
            "order": 1,
            "target_layout": "5.1",
            "max_re_weighting": true,
            "dual_band": false,
        }),
        4,
        SAMPLE_RATE,
    )
    .expect("first-order ambisonics decoder should instantiate");
    let ambisonics_latency = ambisonics.latency_samples();
    host.add_plugin(ambisonics).unwrap();
    assert_eq!(host.output_channels(), 6, "5.1 decode must produce 6ch");

    let binaural = create_plugin(
        "binaural_decoder",
        &serde_json::json!({"input_channels": 6}),
        6,
        SAMPLE_RATE,
    )
    .expect("5.1 binaural renderer should instantiate");
    let binaural_latency = binaural.latency_samples();
    host.add_plugin(binaural).unwrap();

    assert_eq!(host.input_channels(), 4);
    assert_eq!(host.output_channels(), 2);
    host.build()
        .expect("spatial chain should build before latency is queried");
    assert_eq!(
        host.total_latency_samples(),
        ambisonics_latency + binaural_latency,
        "linear spatial-chain latency must be the sum of both stages"
    );
    assert!(
        host.total_latency_samples() > 0,
        "the binaural convolution stage must report nonzero latency"
    );

    // ACN/SN3D first-order channel order is W, Y, Z, X. Use distinct
    // horizontal components so an accidental speaker-layout interpretation or
    // stale-channel reuse cannot look like four identical input channels.
    let frames = 16_384;
    let mut input = vec![0.0_f32; frames * 4];
    for frame in 0..frames {
        let t = frame as f32 / SAMPLE_RATE as f32;
        let fundamental = (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.2;
        input[frame * 4] = fundamental * std::f32::consts::FRAC_1_SQRT_2; // W
        input[frame * 4 + 1] = fundamental * 0.25; // Y
        input[frame * 4 + 2] = 0.0; // Z (horizontal target)
        input[frame * 4 + 3] = fundamental * 0.5; // X
    }

    let mut warmup = vec![0.0_f32; frames * 2];
    host.process(&input, &mut warmup)
        .expect("spatial chain warm-up should process");
    let mut output = vec![0.0_f32; frames * 2];
    let processed = host
        .process(&input, &mut output)
        .expect("spatial chain should process");

    assert_eq!(processed, frames);
    assert_all_finite(&output, "ambisonics->5.1->binaural chain");
    let energy: f32 = output.iter().map(|sample| sample * sample).sum();
    assert!(energy > 1e-6, "spatial chain output was silent: {energy}");
}

/// Place a channel-mismatched plugin at the end of a chain and verify the host
/// rejects it gracefully, leaving the original chain functional.
#[test]
fn mismatched_plugin_at_end_is_rejected_gracefully() {
    let mut host = PluginHost::new(2, SAMPLE_RATE);

    // Valid 2ch -> 2ch plugin.
    let gain = ParametricPluginAdapter::new(GainPlugin::new(2, -3.0));
    host.add_plugin(Box::new(gain)).unwrap();
    assert_eq!(host.plugin_count(), 1);
    assert_eq!(host.output_channels(), 2);

    // mono_to_stereo expects 1 input channel but the chain is currently 2ch.
    let mismatch = create_plugin("mono_to_stereo", &serde_json::json!({}), 1, SAMPLE_RATE)
        .expect("mono_to_stereo should instantiate on its own");

    let err = host
        .add_plugin(mismatch)
        .expect_err("mismatched plugin should be rejected");
    assert!(
        !err.is_empty(),
        "rejection error should contain a non-empty message"
    );

    // The original chain remains valid and processable.
    assert_eq!(host.plugin_count(), 1);
    assert_eq!(host.output_channels(), 2);

    let input = interleaved_sine(2, FRAMES);
    let mut output = vec![0.0f32; FRAMES * 2];
    host.process(&input, &mut output)
        .expect("original chain should still process");
    assert_all_finite(&output, "chain-after-mismatch-rejection");
}

/// Build a longer chain with an explicit channel-count change in the middle
/// (stereo -> mono -> stereo) and verify the final output count.
#[test]
fn mono_stereo_chain_changes_and_restores_count() {
    let mut host = PluginHost::new(2, SAMPLE_RATE);

    // First a channel-preserving plugin at 2ch.
    host.add_plugin(Box::new(ParametricPluginAdapter::new(GainPlugin::new(
        2, -3.0,
    ))))
    .unwrap();

    // Downmix to mono: treat stereo as 2ch input, request 1ch output via matrix.
    let to_mono = create_plugin(
        "matrix",
        &serde_json::json!({
            "input_channels": 2,
            "output_channels": 1,
            "matrix": [0.5, 0.5],
        }),
        2,
        SAMPLE_RATE,
    )
    .expect("matrix should create a 2->1 downmix");
    host.add_plugin(to_mono).unwrap();
    assert_eq!(host.output_channels(), 1, "chain should now be mono");

    // mono_to_stereo: 1ch -> 2ch.
    let to_stereo = create_plugin("mono_to_stereo", &serde_json::json!({}), 1, SAMPLE_RATE)
        .expect("mono_to_stereo should instantiate with mono input");
    host.add_plugin(to_stereo).unwrap();
    assert_eq!(
        host.output_channels(),
        2,
        "chain should be restored to stereo"
    );

    let input = interleaved_sine(2, FRAMES);
    let mut output = vec![0.0f32; FRAMES * 2];
    host.process(&input, &mut output)
        .expect("mono-stereo chain should process");
    assert_all_finite(&output, "mono-stereo chain");
}
