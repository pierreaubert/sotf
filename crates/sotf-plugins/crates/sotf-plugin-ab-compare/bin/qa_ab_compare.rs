use sotf_host::Plugin;
use sotf_host::{CountingAlloc, run_standard_tests};
use sotf_plugin_ab_compare::{ABComparePlugin, ABComparePluginParams, PathConfig};

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    let sample_rate = 48000;
    println!("=== QA: ABCompare Plugin ===");

    for channels in [1, 2, 6, 12] {
        let params = ABComparePluginParams {
            path_a: PathConfig::Plugin {
                plugin_type: "gain".into(),
                parameters: serde_json::json!({"gain_db": -1.0}),
            },
            path_b: PathConfig::Rack {
                plugins: vec![
                    sotf_plugin_ab_compare::PluginInRack {
                        plugin_type: "delay".into(),
                        parameters: serde_json::json!({"delay_ms": 1.0}),
                    },
                    sotf_plugin_ab_compare::PluginInRack {
                        plugin_type: "gain".into(),
                        parameters: serde_json::json!({"gain_db": 1.0}),
                    },
                ],
            },
            band_mask_low_hz: 80.0,
            band_mask_high_hz: 12_000.0,
            auto_gain_enabled: false,
            ..Default::default()
        };
        let mut plugin = ABComparePlugin::from_params(channels, params).unwrap();
        plugin.initialize(sample_rate).unwrap();
        run_standard_tests(
            &mut plugin,
            &format!("ABComparePlugin-{channels}ch-full-path"),
        );
    }

    println!(
        "
[ALL PASS] ABCompare QA Complete."
    );
}
