use sotf_host::{CountingAlloc, Plugin, ProcessContext, run_standard_tests};
use sotf_plugin_ambisonics::{AmbisonicsDecoderConfig, AmbisonicsDecoderPlugin};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;

    println!("=== QA: Ambisonics Decoder Plugin ===");

    // -- Test 1: FOA -> 5.1 decode correctness --
    println!("\n[Test 1] FOA -> 5.1 Omni Decode");
    let config = AmbisonicsDecoderConfig {
        order: 1,
        target_layout: "5.1".to_owned(),
        max_re_weighting: true,
        dual_band: false,
    };
    let mut plugin = AmbisonicsDecoderPlugin::new(&config).unwrap();
    plugin.initialize(sample_rate).unwrap();

    assert_eq!(plugin.input_channels(), 4);
    assert_eq!(plugin.output_channels(), 6);

    let num_frames = 512;
    // Pure W (omnidirectional) signal
    let mut input = vec![0.0_f32; num_frames * 4];
    for f in 0..num_frames {
        input[f * 4] = 1.0; // W channel only
    }
    let mut output = vec![0.0_f32; num_frames * 6];
    let ctx = ProcessContext::new(sample_rate, num_frames);
    plugin.process(&input, &mut output, &ctx).unwrap();

    // Non-LFE channels should all be non-zero for omni signal
    let speaker_config = sotf_host::speaker_config::get_speaker_config("5.1").unwrap();
    let last_frame_start = (num_frames - 1) * 6;
    for spk in speaker_config.speakers {
        if !spk.is_lfe {
            let level = output[last_frame_start + spk.channel].abs();
            println!("  Speaker {} (ch {}): {:.4}", spk.name, spk.channel, level);
            assert!(
                level > 0.01,
                "Speaker {} should be non-zero for omni signal, got {}",
                spk.name,
                level
            );
        }
    }
    println!("  Omni Decode: PASS");

    // -- Test 2: Silence in = Silence out --
    println!("\n[Test 2] Silence Passthrough");
    let input_silent = vec![0.0_f32; num_frames * 4];
    let mut output_silent = vec![0.0_f32; num_frames * 6];
    plugin
        .process(&input_silent, &mut output_silent, &ctx)
        .unwrap();
    let peak = output_silent
        .iter()
        .map(|s| s.abs())
        .fold(0.0_f32, f32::max);
    println!("  Peak output for silence: {:.10}", peak);
    assert!(peak < 1e-10, "Silence in must produce silence out");
    println!("  Silence: PASS");

    // -- Test 3: SOA -> 7.1.4 --
    println!("\n[Test 3] SOA -> 7.1.4");
    let config_soa = AmbisonicsDecoderConfig {
        order: 2,
        target_layout: "7.1.4".to_owned(),
        max_re_weighting: true,
        dual_band: false,
    };
    let mut plugin_soa = AmbisonicsDecoderPlugin::new(&config_soa).unwrap();
    plugin_soa.initialize(sample_rate).unwrap();
    assert_eq!(plugin_soa.input_channels(), 9);
    assert_eq!(plugin_soa.output_channels(), 12);

    let mut input_soa = vec![0.0_f32; num_frames * 9];
    for f in 0..num_frames {
        input_soa[f * 9] = 1.0; // W channel
    }
    let mut output_soa = vec![0.0_f32; num_frames * 12];
    let ctx_soa = ProcessContext::new(sample_rate, num_frames);
    plugin_soa
        .process(&input_soa, &mut output_soa, &ctx_soa)
        .unwrap();

    let energy: f32 = output_soa.iter().map(|s| s * s).sum();
    println!("  SOA total energy: {:.4}", energy);
    assert!(energy > 1.0, "SOA omni should produce significant energy");
    println!("  SOA Decode: PASS");

    // -- Standard tests (latency, zero alloc, performance) --
    run_standard_tests(&mut plugin, "AmbisonicsDecoder");

    println!("\n[ALL PASS] Ambisonics QA Complete.");
}
