use sotf_host::plugin::{InPlacePlugin, InPlacePluginAdapter, ProcessContext};
use sotf_host::{CountingAlloc, assert_no_allocs, run_standard_tests};
use sotf_plugin_channel_mute_solo::{ChannelMuteSoloParams, ChannelMuteSoloPlugin, ChannelState};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 2;
    let params = ChannelMuteSoloParams {
        enabled: true,
        channel_states: vec![
            ChannelState {
                muted: true,
                soloed: false,
                dimmed: false,
            },
            ChannelState {
                muted: false,
                soloed: false,
                dimmed: false,
            },
        ],
        dim_gain_db: -20.0,
        fade_ms: 5.0,
    };

    let mut inner = ChannelMuteSoloPlugin::from_params(channels, params);
    inner.initialize(sample_rate).unwrap();

    println!("=== QA: ChannelMuteSolo Plugin ===");

    // Test 1: Ch0 muted, Ch1 passes through
    println!("\n[Test 1] Channel mute (Ch0 muted, Ch1 pass)");
    let num_frames = 24000; // 500ms for fade convergence
    let mut buffer = vec![1.0f32; num_frames * channels];
    let ctx = ProcessContext::new(sample_rate, num_frames);

    assert_no_allocs("ChannelMuteSoloPlugin::process_in_place", || {
        inner.process_in_place(&mut buffer, &ctx).unwrap();
    });

    let ch0_last = buffer[(num_frames - 1) * channels].abs();
    let ch1_last = buffer[(num_frames - 1) * channels + 1].abs();
    println!(
        "  Ch0 (muted): {:.4}, Ch1 (pass): {:.4}",
        ch0_last, ch1_last
    );
    assert!(ch0_last < 0.01, "Muted channel should be near zero");
    assert!(
        (ch1_last - 1.0).abs() < 0.01,
        "Unmuted channel should pass through"
    );

    // Run standard QA tests
    let mut plugin = InPlacePluginAdapter::new(inner);
    run_standard_tests(&mut plugin, "ChannelMuteSoloPlugin");

    println!("\n[ALL PASS] ChannelMuteSolo QA Complete.");
}
