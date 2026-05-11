use sotf_host::{CountingAlloc, run_standard_tests};
use sotf_host::{Plugin, ProcessContext};
use sotf_plugin_downmix::{DownmixPlugin, DownmixPluginParams};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let params = DownmixPluginParams {
        input_channels: 6,
        center_gain_db: 0.0, // 1.0 linear
        surround_gain_db: -100.0,
        height_gain_db: -100.0,
        lfe_gain_db: -100.0,
        phase_coherence: false,
        phase_blend_low_hz: 200.0,
        phase_blend_high_hz: 5000.0,
        itu_mode: false,
        matrix_ltrt: false,
        phase_coherence_strength: 0.5,
    };

    let mut plugin = DownmixPlugin::from_params(params);
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: Downmix Plugin ===");

    // Test 1: Pure Center to Stereo
    println!("\n[Test 1] Center to L/R (Center=1.0, Gain=0dB)");
    let num_frames = 8192;
    let mut input = vec![0.0; num_frames * 6];
    for i in 0..num_frames {
        input[i * 6 + 2] = 1.0; // C
    }

    let mut output = vec![0.0; num_frames * 2];

    let mut pos = 0;
    let block_size = 1024;
    while pos < num_frames {
        let end = (pos + block_size).min(num_frames);
        let ctx = ProcessContext {
            sample_rate,
            num_frames: end - pos,
        };
        plugin
            .process(
                &input[pos * 6..end * 6],
                &mut output[pos * 2..end * 2],
                &ctx,
            )
            .unwrap();
        pos = end;
    }

    let last_sample_l = output[(num_frames - 1) * 2];
    println!("  L_out Expected: ~0.707, Measured: {:.3}", last_sample_l);
    assert!((last_sample_l - 0.707).abs() < 0.1);
    println!("  Center to L/R: PASS");

    // Run standard QA tests
    run_standard_tests(&mut plugin, "DownmixPlugin");

    println!("\n[ALL PASS] Downmix QA Complete.");
}
