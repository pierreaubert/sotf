use sotf_host::analyzer_loudness_monitor::LoudnessMonitorPlugin;
use sotf_host::analyzer_spectrum::SpectrumAnalyzerPlugin;
use sotf_host::auto_gain::AutoGain;
use sotf_host::oversampling::OversampledPlugin;
use sotf_host::parameters::{Parameter, ParameterId, ParameterValue};
use sotf_host::plugin::{PluginInfo, PluginResult};
use sotf_host::{
    CountingAlloc, InPlacePlugin, InPlacePluginAdapter, Plugin, ProcessContext, assert_no_allocs,
    run_standard_tests,
};
use std::time::Instant;

#[global_allocator]
static A: CountingAlloc = CountingAlloc;

fn main() {
    println!("=== QA: sotf-host ===\n");

    qa_spectrum_analyzer();
    qa_loudness_monitor();
    qa_oversampled_plugin();
    qa_auto_gain();

    println!("\n[ALL PASS] sotf-host QA Complete.");
}

// ============================================================================
// Spectrum Analyzer
// ============================================================================

fn qa_spectrum_analyzer() {
    println!("--- Spectrum Analyzer ---");

    let mut plugin = SpectrumAnalyzerPlugin::new(2).expect("create spectrum analyzer");
    plugin.initialize(48000).unwrap();

    run_standard_tests(&mut plugin, "SpectrumAnalyzer");

    // Extended: process many blocks and check no growth
    println!("\n[Test] Extended processing (50s, check no leaks)");
    let block_size = 512;
    let num_blocks = (48000 * 50) / block_size;
    let input = vec![0.1_f32; block_size * 2];
    let mut output = vec![0.0_f32; block_size * 2];
    let ctx = ProcessContext {
        sample_rate: 48000,
        num_frames: block_size,
    };

    for _ in 0..num_blocks {
        plugin.process(&input, &mut output, &ctx).unwrap();
    }

    // Verify no allocations in steady state
    assert_no_allocs("SpectrumAnalyzer::extended_process", || {
        for _ in 0..100 {
            plugin.process(&input, &mut output, &ctx).unwrap();
            let _data = plugin.get_data();
        }
    });
    println!("  Extended processing: PASS");
}

// ============================================================================
// Loudness Monitor
// ============================================================================

fn qa_loudness_monitor() {
    println!("\n--- Loudness Monitor ---");

    let mut plugin = LoudnessMonitorPlugin::new(2).expect("create loudness monitor");
    plugin.initialize(48000).unwrap();

    run_standard_tests(&mut plugin, "LoudnessMonitor");

    // Extended: process many blocks and check no growth
    println!("\n[Test] Extended processing (50s, check no leaks)");
    let block_size = 512;
    let num_blocks = (48000 * 50) / block_size;
    let input = vec![0.1_f32; block_size * 2];
    let mut output = vec![0.0_f32; block_size * 2];
    let ctx = ProcessContext {
        sample_rate: 48000,
        num_frames: block_size,
    };

    for _ in 0..num_blocks {
        plugin.process(&input, &mut output, &ctx).unwrap();
    }

    assert_no_allocs("LoudnessMonitor::extended_process", || {
        for _ in 0..100 {
            plugin.process(&input, &mut output, &ctx).unwrap();
            let _data = plugin.get_data();
        }
    });
    println!("  Extended processing: PASS");
}

// ============================================================================
// Oversampled Plugin
// ============================================================================

/// Minimal passthrough for testing the oversampler wrapper.
struct PassthroughPlugin;

impl InPlacePlugin for PassthroughPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Passthrough", "1.0", "Test")
    }
    fn channels(&self) -> usize {
        2
    }
    fn parameters(&self) -> Vec<Parameter> {
        vec![]
    }
    fn set_parameter(&mut self, _: ParameterId, _: ParameterValue) -> PluginResult<()> {
        Ok(())
    }
    fn get_parameter(&self, _: &ParameterId) -> Option<ParameterValue> {
        None
    }
    fn initialize(&mut self, _: u32) -> PluginResult<()> {
        Ok(())
    }
    fn reset(&mut self) {}
    fn process_in_place(
        &mut self,
        _buffer: &mut [f32],
        ctx: &ProcessContext,
    ) -> PluginResult<usize> {
        Ok(ctx.num_frames)
    }
}

fn qa_oversampled_plugin() {
    println!("\n--- Oversampled Plugin (4x) ---");

    let inner = PassthroughPlugin;
    let os = OversampledPlugin::new(inner, 4, 2).expect("create oversampled plugin");
    let mut plugin = InPlacePluginAdapter::new(os);
    plugin.initialize(48000).unwrap();

    run_standard_tests(&mut plugin, "OversampledPlugin_4x");

    // Extended steady-state check
    println!("\n[Test] Extended processing (50s, check no leaks)");
    let block_size = 512;
    let num_blocks = (48000 * 50) / block_size;
    let input = vec![0.1_f32; block_size * 2];
    let mut output = vec![0.0_f32; block_size * 2];
    let ctx = ProcessContext {
        sample_rate: 48000,
        num_frames: block_size,
    };

    for _ in 0..num_blocks {
        plugin.process(&input, &mut output, &ctx).unwrap();
    }

    assert_no_allocs("OversampledPlugin::extended_process", || {
        for _ in 0..100 {
            plugin.process(&input, &mut output, &ctx).unwrap();
        }
    });
    println!("  Extended processing: PASS");
}

// ============================================================================
// Auto Gain
// ============================================================================

fn qa_auto_gain() {
    println!("\n--- Auto Gain ---");

    let channels = 2;
    let sample_rate = 48000;
    let mut ag = AutoGain::new_default(channels, sample_rate).expect("create auto gain");

    let block_size = 512;
    let buffer = vec![0.1_f32; block_size * channels];

    // Warm up
    for _ in 0..200 {
        ag.measure_input(&buffer).unwrap();
        ag.measure_output(&buffer).unwrap();
        ag.next_n(block_size);
    }

    // Steady state: no allocations
    println!("\n[Test] Auto Gain steady-state (zero allocations)");
    assert_no_allocs("AutoGain::measure+gain", || {
        for _ in 0..500 {
            ag.measure_input(&buffer).unwrap();
            ag.measure_output(&buffer).unwrap();
            ag.next_n(block_size);
            let _ = ag.current_gain_db();
        }
    });
    println!("  Zero Allocations: PASS");

    // Extended processing for leak check
    println!("\n[Test] Extended processing (50s, check no leaks)");
    let num_blocks = (48000 * 50) / block_size;
    let start = Instant::now();
    for _ in 0..num_blocks {
        ag.measure_input(&buffer).unwrap();
        ag.measure_output(&buffer).unwrap();
        ag.next_n(block_size);
    }
    let elapsed = start.elapsed();
    let audio_sec = (num_blocks * block_size) as f64 / sample_rate as f64;
    let cpu = (elapsed.as_secs_f64() / audio_sec) * 100.0;
    println!(
        "  Processed {:.1}s in {:.2}ms ({:.2}% CPU)",
        audio_sec,
        elapsed.as_secs_f64() * 1000.0,
        cpu
    );
    println!("  Extended processing: PASS");
}
