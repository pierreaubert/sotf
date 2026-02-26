use sotf_host::Plugin;
use sotf_plugin_pnd::PndPlugin;
use sotf_host::{CountingAlloc, run_standard_tests, generate_dc, measure_peak_db};

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

    println!(
        "
[ALL PASS] PND QA Complete."
    );
}
