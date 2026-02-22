use sotf_plugins::plugin_delay::{DelayPlugin, DelayPluginParams};
use sotf_plugins::{InPlacePlugin, ProcessContext};

fn main() {
    let sample_rate = 48000;
    let channels = 1;
    let params = DelayPluginParams {
        delay_ms: 10.0, // 480 samples
        feedback: 0.0,
        mix: 1.0, // Wet only
    };

    let mut plugin = DelayPlugin::from_params(channels, params);
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: Delay Plugin ===");

    // Test 1: Impulse Delay
    println!("
[Test 1] Impulse Delay (10ms / 480 samples)");
    let num_frames = 1000;
    let mut buffer = vec![0.0; num_frames];
    buffer[0] = 1.0; // Impulse
    
    let ctx = ProcessContext { sample_rate, num_frames };
    plugin.process_in_place(&mut buffer, &ctx).unwrap();
    
    // Find impulse in output
    let mut impulse_pos = 0;
    for (i, &s) in buffer.iter().enumerate() {
        if s > 0.5 { impulse_pos = i; break; }
    }
    
    println!("  Impulse Position: {} samples", impulse_pos);
    assert_eq!(impulse_pos, 480);
    println!("  Impulse Position: PASS");

    println!("
[PASS] Delay QA Complete.");
}
