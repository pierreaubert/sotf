use sotf_plugins::plugin_ab_compare::ABComparePlugin;
use sotf_plugins::qa_util::{run_standard_tests, CountingAlloc};
use sotf_plugins::Plugin;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    let channels = 2;
    
    let mut plugin = ABComparePlugin::new(channels).unwrap();
    plugin.initialize(sample_rate).unwrap();

    println!("=== QA: ABCompare Plugin ===");

    // Run standard QA tests
    run_standard_tests(&mut plugin, "ABComparePlugin");

    println!("
[ALL PASS] ABCompare QA Complete.");
}
