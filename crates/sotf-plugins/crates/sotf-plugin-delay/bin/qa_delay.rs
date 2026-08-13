use sotf_host::{
    CountingAlloc, ParametricInPlacePlugin, ParametricInPlacePluginAdapter, ProcessContext,
    run_standard_tests,
};
use sotf_plugin_delay::{DelayPlugin, DelayPluginParams};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 1;
    let params = DelayPluginParams {
        delay_ms: 10.0, // 480 samples
        feedback: 0.0,
        mix: 1.0, // Wet only
        lfo_rate_hz: 0.0,
        lfo_depth_ms: 0.0,
        pitch_preserving: false,
        allpass_feedback: false,
        allpass_coeff: 0.5,
        channel_delays_ms: Vec::new(),
    };

    let mut inner = DelayPlugin::from_params(channels, params).expect("valid params");
    inner.initialize(sample_rate).unwrap();

    println!("=== QA: Delay Plugin ===");

    // Test 1: Impulse Delay
    println!("\n[Test 1] Impulse Delay (10ms / 480 samples)");
    let num_frames = 1000;
    let mut buffer = vec![0.0; num_frames];
    buffer[0] = 1.0; // Impulse

    let ctx = ProcessContext::new(sample_rate, num_frames);
    inner.process_in_place(&mut buffer, &ctx).unwrap();

    // Find impulse in output
    let mut impulse_pos = 0;
    for (i, &s) in buffer.iter().enumerate() {
        if s > 0.5 {
            impulse_pos = i;
            break;
        }
    }

    println!("  Impulse Position: {} samples", impulse_pos);
    assert_eq!(impulse_pos, 480);
    println!("  Impulse Position: PASS");

    // Run standard QA tests
    let mut plugin = ParametricInPlacePluginAdapter::new(inner);
    run_standard_tests(&mut plugin, "DelayPlugin");

    let clean_params = DelayPluginParams {
        delay_ms: 100.0,
        feedback: 0.3,
        mix: 1.0,
        lfo_rate_hz: 0.0,
        lfo_depth_ms: 0.0,
        pitch_preserving: true,
        allpass_feedback: false,
        allpass_coeff: 0.5,
        channel_delays_ms: Vec::new(),
    };
    let mut clean = DelayPlugin::from_params(2, clean_params).expect("valid clean params");
    clean.initialize(sample_rate).unwrap();
    let mut clean = ParametricInPlacePluginAdapter::new(clean);
    run_standard_tests(&mut clean, "DelayPlugin pitch-preserving stereo");

    println!("\n[ALL PASS] Delay QA Complete.");
}
