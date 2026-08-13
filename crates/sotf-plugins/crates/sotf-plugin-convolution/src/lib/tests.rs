use super::convolution_plugin::{ConvolutionPlugin, defer_or_forget};
use super::types::ConvolutionPluginParams;
use super::types::ConvolutionState;
use super::types::{ConvolutionLoadStatus, IrLoadCompletion};
use crate::misc::{FFT_SIZE, PARTITION_SIZE};
use plugins_spatial::nupc;
use rustfft::FftPlanner;
use rustfft::num_complex::Complex;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;
use std::sync::Arc;

#[test]
fn reclamation_queue_failure_never_drops_on_the_caller() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    struct DropCounter(Arc<AtomicUsize>);
    impl Drop for DropCounter {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::Relaxed);
        }
    }

    let drops = Arc::new(AtomicUsize::new(0));
    let mut pending = Some(DropCounter(Arc::clone(&drops)));
    defer_or_forget(
        &mut pending,
        std::sync::mpsc::TrySendError::Full(DropCounter(Arc::clone(&drops))),
    );
    defer_or_forget(
        &mut pending,
        std::sync::mpsc::TrySendError::Disconnected(DropCounter(Arc::clone(&drops))),
    );
    assert_eq!(drops.load(Ordering::Relaxed), 0);

    drop(pending);
    assert_eq!(drops.load(Ordering::Relaxed), 1);
}

#[path = "tests/misc.rs"]
mod misc;

#[test]
fn from_params_rebuilds_host_visible_values() {
    let params = ConvolutionPluginParams {
        ir_file: String::new(),
        mix: 0.25,
        gain_db: 6.0,
        use_nupc: true,
        zero_latency_head: true,
        head_taps: 256,
    };
    let plugin = ConvolutionPlugin::from_params(1, 48_000, params).unwrap();
    let values = plugin.current_values();

    assert_eq!(
        values.get(&ParameterId::from("mix")),
        Some(&ParameterValue::Float(0.25))
    );
    assert_eq!(
        values.get(&ParameterId::from("gain_db")),
        Some(&ParameterValue::Float(6.0))
    );
    assert_eq!(
        values.get(&ParameterId::from("use_nupc")),
        Some(&ParameterValue::Bool(true))
    );
    assert_eq!(
        values.get(&ParameterId::from("zero_latency_head")),
        Some(&ParameterValue::Bool(true))
    );
    assert_eq!(
        values.get(&ParameterId::from("head_taps")),
        Some(&ParameterValue::Int(256))
    );
}

#[test]
fn new_uses_the_declared_nupc_default() {
    let plugin = ConvolutionPlugin::new(1, 48_000);
    assert_eq!(plugin.use_nupc, crate::params::PARAMS[3].default_bool());
}

#[test]
fn construction_rejects_every_out_of_contract_value() {
    for params in [
        ConvolutionPluginParams {
            mix: f32::NAN,
            ..ConvolutionPluginParams::default()
        },
        ConvolutionPluginParams {
            mix: -0.01,
            ..ConvolutionPluginParams::default()
        },
        ConvolutionPluginParams {
            gain_db: 20.01,
            ..ConvolutionPluginParams::default()
        },
        ConvolutionPluginParams {
            head_taps: 31,
            ..ConvolutionPluginParams::default()
        },
        ConvolutionPluginParams {
            head_taps: 513,
            ..ConvolutionPluginParams::default()
        },
    ] {
        assert!(ConvolutionPlugin::from_params(1, 48_000, params).is_err());
    }
    assert!(ConvolutionPlugin::from_params(0, 48_000, ConvolutionPluginParams::default()).is_err());
    assert!(ConvolutionPlugin::from_params(1, 0, ConvolutionPluginParams::default()).is_err());
}

#[test]
fn ir_metadata_and_memory_limits_are_enforced_before_planning() {
    assert!(ConvolutionPlugin::validate_ir_limits(&[vec![1.0]], 0, 48_000, 2, true).is_err());
    assert!(ConvolutionPlugin::validate_ir_limits(&[], 48_000, 48_000, 2, true).is_err());
    let too_many_channels = vec![vec![1.0]; 33];
    assert!(
        ConvolutionPlugin::validate_ir_limits(&too_many_channels, 48_000, 48_000, 2, true).is_err()
    );
    let too_long = vec![vec![0.0; 48_000 * 30 + 1]];
    assert!(ConvolutionPlugin::validate_ir_limits(&too_long, 48_000, 48_000, 2, false).is_err());
    let large_but_duration_valid = vec![vec![0.0; 48_000 * 30]];
    assert!(
        ConvolutionPlugin::validate_ir_limits(
            &large_but_duration_valid,
            48_000,
            48_000,
            128,
            true,
        )
        .is_err(),
        "channel-expanded NUPC state must respect the memory budget"
    );
}

#[test]
fn upc_automation_envelope_is_captured_at_input_time() {
    let mut plugin = make_delta_ir_plugin(false, false);
    plugin.mix.reset(1.0);
    let mut first = vec![0.25; 100];
    plugin
        .process_in_place(&mut first, &ProcessContext::new(48_000, 100))
        .unwrap();
    plugin
        .parametric_set_parameter(ParameterId::from("mix"), ParameterValue::Float(0.0))
        .unwrap();
    let mut rest = vec![0.25; PARTITION_SIZE - 100];
    plugin
        .process_in_place(
            &mut rest,
            &ProcessContext::new(48_000, PARTITION_SIZE - 100),
        )
        .unwrap();
    assert_eq!(plugin.mix_envelope[99], 1.0);
    assert!(plugin.mix_envelope[100] < 1.0);
    assert!(plugin.mix_envelope[100] > plugin.mix_envelope[PARTITION_SIZE - 1]);
}

#[test]
fn stale_and_failed_async_generations_never_replace_last_known_good() {
    let mut plugin = make_delta_ir_plugin(false, false);
    let original_state = plugin.state.load_full();
    plugin.desired_generation = 7;
    plugin.load_status.store(
        ConvolutionLoadStatus::Loading as u8,
        std::sync::atomic::Ordering::Release,
    );
    let (tx, rx) = std::sync::mpsc::channel();
    plugin.ir_load_result_rx = Some(rx);
    tx.send(IrLoadCompletion {
        generation: 6,
        result: Err("stale failure".into()),
    })
    .unwrap();
    let mut block = vec![0.0; 16];
    plugin
        .process_in_place(&mut block, &ProcessContext::new(48_000, 16))
        .unwrap();
    assert!(Arc::ptr_eq(&original_state, &plugin.state.load_full()));

    let (tx, rx) = std::sync::mpsc::channel();
    plugin.ir_load_result_rx = Some(rx);
    tx.send(IrLoadCompletion {
        generation: 7,
        result: Err("decode failed".into()),
    })
    .unwrap();
    plugin
        .process_in_place(&mut block, &ProcessContext::new(48_000, 16))
        .unwrap();
    assert_eq!(plugin.load_status(), ConvolutionLoadStatus::Failed);
    assert!(Arc::ptr_eq(&original_state, &plugin.state.load_full()));
}

#[test]
fn clear_transition_has_a_bounded_sample_discontinuity() {
    let mut plugin = make_delta_ir_plugin(false, false);
    plugin.last_output[0] = 1.0;
    plugin
        .parametric_set_parameter(
            ParameterId::from("ir_file"),
            ParameterValue::String(String::new()),
        )
        .unwrap();
    let mut block = vec![0.0; 128];
    plugin
        .process_in_place(&mut block, &ProcessContext::new(48_000, 128))
        .unwrap();
    let mut previous = 1.0;
    for sample in block {
        assert!(
            (sample - previous).abs() <= 1.0 / 128.0 + 1e-6,
            "transition jumped from {previous} to {sample}"
        );
        previous = sample;
    }
    assert!(previous.abs() < 1e-6);
}

#[test]
fn clearing_ir_cancels_pending_async_result() {
    let mut plugin = ConvolutionPlugin::new(1, 48_000);
    let (_tx, rx) = std::sync::mpsc::channel();
    plugin.ir_load_result_rx = Some(rx);

    plugin
        .parametric_set_parameter(
            ParameterId::from("ir_file"),
            ParameterValue::String(String::new()),
        )
        .unwrap();

    assert!(plugin.ir_load_result_rx.is_none());
    assert!(plugin.state.load().is_none());
}

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

fn resampled_delta(
    source_rate: u32,
    target_rate: u32,
    source_len: usize,
    source_index: usize,
) -> Vec<f32> {
    let mut ir = vec![0.0_f32; source_len];
    ir[source_index] = 1.0;
    ConvolutionPlugin::resample_ir(&[ir], source_rate, target_rate)
        .expect("delta IR should resample")
        .into_iter()
        .next()
        .expect("one-channel resampling should return one channel")
}

#[test]
fn resampled_ir_delta_at_start_has_no_rubato_startup_delay() {
    for (source_rate, target_rate) in [(44_100, 48_000), (48_000, 44_100), (96_000, 48_000)] {
        let output = resampled_delta(source_rate, target_rate, 4097, 0);
        assert_eq!(
            output.len(),
            (4097.0 * target_rate as f64 / source_rate as f64).ceil() as usize,
            "resampled clip length must be rounded from the source clip length"
        );
        let start_peak = output[..output.len().min(32)]
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
            .map(|(index, sample)| (index, sample.abs()))
            .expect("resampled output must not be empty");
        assert!(
            start_peak.1 > 0.1 && start_peak.0 < 16,
            "start impulse must remain near the start after delay compensation: rate {source_rate}->{target_rate}, peak={start_peak:?}"
        );
    }
}

#[test]
fn resampled_ir_delta_at_tail_preserves_the_last_response() {
    for (source_rate, target_rate) in [(44_100, 48_000), (48_000, 44_100), (96_000, 48_000)] {
        let source_len = 4097;
        let output = resampled_delta(source_rate, target_rate, source_len, source_len - 1);
        let expected_index =
            ((source_len - 1) as f64 * target_rate as f64 / source_rate as f64).round() as usize;
        let search_start = expected_index.saturating_sub(8);
        let search_end = (expected_index + 9).min(output.len());
        let local_peak = output[search_start..search_end]
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| a.abs().total_cmp(&b.abs()))
            .map(|(offset, sample)| (search_start + offset, sample.abs()))
            .expect("tail response must be retained in the output clip");

        assert!(
            local_peak.1 > 0.05,
            "tail impulse response was truncated during resampling: rate {source_rate}->{target_rate}, expected={expected_index}, peak={local_peak:?}, output_len={} ",
            output.len()
        );
        assert!(
            output[expected_index.min(output.len() - 1)].abs() > 0.01 || local_peak.1 > 0.2,
            "tail response should remain centered near its nominal target position: rate {source_rate}->{target_rate}, expected={expected_index}, peak={local_peak:?}"
        );
    }
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
        fft_forward: Some(fft_forward),
        fft_inverse: Some(fft_inverse),
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

    let replacement = ConvolutionPlugin::build_ir_state(
        ir_path.to_str().unwrap(),
        1,
        48_000,
        plugin.use_nupc,
        plugin.zero_latency_head,
        plugin.head_taps,
    )
    .unwrap();
    plugin.desired_generation += 1;
    let generation = plugin.desired_generation;
    let (tx, rx) = std::sync::mpsc::channel();
    plugin.ir_load_result_rx = Some(rx);
    tx.send(IrLoadCompletion {
        generation,
        result: Ok(replacement),
    })
    .unwrap();
    assert_no_allocs("ConvolutionPlugin async install/retire", || {
        plugin.process_in_place(&mut buffer, &ctx).unwrap();
    });

    assert_no_allocs("ConvolutionPlugin::process_in_place long IR", || {
        for _ in 0..100 {
            plugin.process_in_place(&mut buffer, &ctx).unwrap();
        }
    });

    std::fs::remove_file(&ir_path).ok();
}
