use sotf_host::{CountingAlloc, run_standard_tests};
use sotf_host::{Plugin, ProcessContext};
use sotf_plugin_beamformer::BeamformerPlugin;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let num_mics = 2;

    let mut plugin = BeamformerPlugin::new(num_mics, sample_rate);
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: Beamformer Plugin ===");

    // Test 1: GSC mode — sample-by-sample, zero latency
    println!("\n[Test 1] GSC mode processing (512 frames)");
    let num_frames = 512;
    let input = vec![0.1f32; num_frames * num_mics];
    let mut output = vec![0.0f32; num_frames];
    let ctx = ProcessContext {
        sample_rate,
        num_frames,
    };

    use sotf_host::parameters::{ParameterId, ParameterValue};
    plugin
        .set_parameter(ParameterId::from("beamformer_type"), ParameterValue::Int(2))
        .unwrap(); // GSC mode
    plugin.process(&input, &mut output, &ctx).unwrap();
    println!("  GSC process completed: PASS");

    // Test 2: MVDR mode
    println!("\n[Test 2] MVDR mode processing");
    plugin
        .set_parameter(ParameterId::from("beamformer_type"), ParameterValue::Int(0))
        .unwrap();
    let mut output2 = vec![0.0f32; num_frames];
    plugin.process(&input, &mut output2, &ctx).unwrap();
    println!("  MVDR process completed: PASS");

    // Run standard QA tests
    run_standard_tests(&mut plugin, "BeamformerPlugin");

    println!("\n[ALL PASS] Beamformer QA Complete.");
}
