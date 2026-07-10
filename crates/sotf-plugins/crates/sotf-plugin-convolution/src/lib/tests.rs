use super::convolution_plugin::ConvolutionPlugin;
use super::types::ConvolutionPluginParams;
use super::types::ConvolutionState;
use crate::misc::{FFT_SIZE, PARTITION_SIZE};
use plugins_spatial::nupc;
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;
use std::sync::Arc;

mod misc;

#[test]
fn test_from_params_propagates_ir_load_errors() {
    let params = ConvolutionPluginParams {
        ir_file: "/definitely/missing/sotf-test-ir.wav".to_string(),
        mix: 1.0,
        gain_db: 0.0,
        use_nupc: true,
        zero_latency_head: false,
        head_taps: 128,
    };
    assert!(ConvolutionPlugin::from_params(1, 48000, params).is_err());
}

#[test]
fn test_ir_file_parameter_reports_load_errors() {
    let mut plugin = ConvolutionPlugin::new(1, 48000);
    let err = plugin
        .parametric_set_parameter(
            ParameterId::from("ir_file"),
            ParameterValue::String("/definitely/missing/sotf-test-ir.wav".to_string()),
        )
        .unwrap_err();
    assert!(err.contains("IO:"), "unexpected error: {err}");
    assert!(plugin.state.load().is_none());
}

#[test]
fn uniform_partitioned_convolution_reports_partition_latency() {
    let plugin = ConvolutionPlugin::new(1, 48_000);
    assert_eq!(plugin.latency_samples(), PARTITION_SIZE);
}

#[test]
fn configured_zero_latency_head_is_stable_before_runtime_ir_load() {
    let params = ConvolutionPluginParams {
        ir_file: String::new(),
        mix: 1.0,
        gain_db: 0.0,
        use_nupc: true,
        zero_latency_head: true,
        head_taps: 128,
    };
    let mut plugin = ConvolutionPlugin::from_params(1, 48_000, params).unwrap();
    assert!(plugin.nupc_engines.is_empty());
    let latency_before_load = plugin.latency_samples();

    plugin.nupc_engines = vec![nupc::NupcEngine::new_with_head(&[1.0], PARTITION_SIZE, 128)];
    assert_eq!(latency_before_load, 0);
    assert_eq!(plugin.latency_samples(), latency_before_load);
}

fn make_delta_ir_plugin(use_nupc: bool, zero_latency_head: bool) -> ConvolutionPlugin {
    let mut plugin = ConvolutionPlugin::new(1, 48_000);
    let mut planner = FftPlanner::<f32>::new();
    let fft_forward = planner.plan_fft_forward(FFT_SIZE);
    let fft_inverse = planner.plan_fft_inverse(FFT_SIZE);
    let mut partition = vec![Complex::new(0.0, 0.0); FFT_SIZE];
    partition[0] = Complex::new(1.0, 0.0);
    fft_forward.process(&mut partition);
    let scratch_len = fft_forward
        .get_inplace_scratch_len()
        .max(fft_inverse.get_inplace_scratch_len());
    plugin.state.store(Arc::new(Some(ConvolutionState {
        partitions: vec![vec![partition]],
        num_partitions: 1,
        ir_channels: 1,
        fft_forward,
        fft_inverse,
    })));
    plugin.fdl_flat = vec![Complex::new(0.0, 0.0); FFT_SIZE];
    plugin.fft_scratch = vec![Complex::new(0.0, 0.0); scratch_len];
    plugin.use_nupc = use_nupc;
    plugin.zero_latency_head = zero_latency_head;
    if use_nupc {
        plugin.nupc_engines = vec![if zero_latency_head {
            nupc::NupcEngine::new_with_head(&[1.0], PARTITION_SIZE, 128)
        } else {
            nupc::NupcEngine::new(&[1.0], PARTITION_SIZE)
        }];
    }
    plugin
}

fn processed_delta_peak(mut plugin: ConvolutionPlugin, mix: f32) -> (usize, f32) {
    plugin.mix_value = mix;
    plugin.mix.reset(mix);
    plugin.gain_linear.reset(1.0);
    let mut signal = vec![0.0f32; PARTITION_SIZE * 3];
    signal[0] = 1.0;
    for block in signal.chunks_mut(127) {
        plugin
            .process_in_place(block, &ProcessContext::new(48_000, block.len()))
            .unwrap();
    }
    signal
        .iter()
        .copied()
        .enumerate()
        .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
        .unwrap()
}

#[test]
fn convolution_delta_positions_match_reported_latency() {
    for (use_nupc, zero_latency_head) in [(false, false), (true, false), (true, true)] {
        let plugin = make_delta_ir_plugin(use_nupc, zero_latency_head);
        let latency = plugin.latency_samples();
        assert_eq!(
            processed_delta_peak(plugin, 1.0).0,
            latency,
            "use_nupc={use_nupc}, zero_latency_head={zero_latency_head}"
        );
    }
}

#[test]
fn nupc_partial_mix_delays_dry_to_the_wet_path() {
    let plugin = make_delta_ir_plugin(true, false);
    let latency = plugin.latency_samples();
    let (peak_index, peak) = processed_delta_peak(plugin, 0.5);
    assert_eq!(peak_index, latency);
    assert!(
        (peak - 1.0).abs() < 1e-4,
        "aligned dry and wet impulses should sum to unity, got {peak}"
    );
}

#[test]
fn runtime_structural_change_cannot_desynchronize_latency_from_loaded_engine() {
    let mut plugin = make_delta_ir_plugin(true, false);
    let latency = plugin.latency_samples();
    let error = plugin
        .parametric_set_parameter(
            ParameterId::from("zero_latency_head"),
            ParameterValue::Bool(true),
        )
        .unwrap_err();
    assert!(error.contains("structural"), "unexpected error: {error}");
    assert_eq!(plugin.latency_samples(), latency);
    assert_eq!(processed_delta_peak(plugin, 1.0).0, latency);
}

#[test]
fn ir_loader_does_not_hold_receiver_lock_while_building_ir_state() {
    let source = include_str!("convolution_plugin.rs");
    assert!(
        source.contains("let req = match rx.lock().unwrap().recv()"),
        "IR loader workers should hold the receiver mutex only while taking a request"
    );
    assert!(
        !source.contains("while let Ok(req) = rx.lock().unwrap().recv()"),
        "IR loader must not hold the receiver mutex while build_ir_state runs"
    );
}

#[test]
fn test_set_parameter_long_ir_loads_without_process_allocations() {
    use sotf_host::assert_no_allocs;
    use std::io::Write;

    fn write_silence_wav(
        path: &std::path::Path,
        frames: usize,
        channels: u16,
        sample_rate: u32,
    ) -> std::io::Result<()> {
        let bits_per_sample: u16 = 16;
        let byte_rate = sample_rate * (channels as u32) * (bits_per_sample as u32) / 8;
        let block_align = channels * bits_per_sample / 8;
        let data_len = (frames * channels as usize * (bits_per_sample as usize / 8)) as u32;
        let riff_len = 36 + data_len;

        let mut file = std::fs::File::create(path)?;
        file.write_all(b"RIFF")?;
        file.write_all(&riff_len.to_le_bytes())?;
        file.write_all(b"WAVE")?;
        file.write_all(b"fmt ")?;
        file.write_all(&16u32.to_le_bytes())?;
        file.write_all(&1u16.to_le_bytes())?;
        file.write_all(&channels.to_le_bytes())?;
        file.write_all(&sample_rate.to_le_bytes())?;
        file.write_all(&byte_rate.to_le_bytes())?;
        file.write_all(&block_align.to_le_bytes())?;
        file.write_all(&bits_per_sample.to_le_bytes())?;
        file.write_all(b"data")?;
        file.write_all(&data_len.to_le_bytes())?;
        for _ in 0..frames * channels as usize {
            file.write_all(&0i16.to_le_bytes())?;
        }
        Ok(())
    }

    let tmp_dir = std::env::temp_dir().join("sotf_convolution_long_ir_test");
    std::fs::create_dir_all(&tmp_dir).unwrap();
    let ir_path = tmp_dir.join("long_ir.wav");

    // 8192 samples = 8 partitions, triggering the parallel convolution path.
    write_silence_wav(&ir_path, 8192, 1, 48000).unwrap();

    let mut plugin = ConvolutionPlugin::new(1, 48000);
    plugin
        .parametric_set_parameter(
            ParameterId::from("ir_file"),
            ParameterValue::String(ir_path.to_string_lossy().into_owned()),
        )
        .unwrap();

    let mut buffer = vec![0.0f32; 1024];
    let ctx = ProcessContext::new(48000, 1024);

    // Warm up and wait for the background IR load to complete.
    for _ in 0..200 {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
        if plugin.ir_load_result_rx.is_none() {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert!(
        plugin.ir_load_result_rx.is_none(),
        "IR load should complete"
    );

    assert_no_allocs("ConvolutionPlugin::process_in_place long IR", || {
        for _ in 0..100 {
            plugin.process_in_place(&mut buffer, &ctx).unwrap();
        }
    });

    std::fs::remove_file(&ir_path).ok();
}
