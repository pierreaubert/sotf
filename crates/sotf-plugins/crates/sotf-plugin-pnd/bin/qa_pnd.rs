use sotf_host::{CountingAlloc, run_standard_tests};
use sotf_host::{Plugin, ProcessContext};
use sotf_plugin_pnd::PndPlugin;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 2;

    let mut plugin = PndPlugin::new(channels);
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: PND Plugin ===");

    // Run standard QA tests
    run_standard_tests(&mut plugin, "PndPlugin");

    println!("\n[PND] Fixed-frame duration-preserving contract");
    assert_eq!(plugin.latency_samples(), 2047);
    for frames in [1usize, 64, 511, 512, 1024, 1273] {
        let input: Vec<f32> = (0..frames * channels)
            .map(|sample| (sample as f32 * 0.013).sin() * 0.25)
            .collect();
        let mut output = vec![0.0; input.len()];
        let produced = plugin
            .process(
                &input,
                &mut output,
                &ProcessContext::new(sample_rate, frames),
            )
            .unwrap();
        assert_eq!(produced, frames);
        assert!(output.iter().all(|sample| sample.is_finite()));
    }
    println!("  fixed frame count, finite output, and 2047-frame latency: PASS");

    println!(
        "
[ALL PASS] PND QA Complete."
    );
}
