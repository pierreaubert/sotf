use super::convolution_plugin::ConvolutionPlugin;
use super::types::ConvolutionPluginParams;
use sotf_host::parameters::{ParameterId, ParameterValue};
use sotf_host::parametric_in_place_plugin::ParametricInPlacePlugin;
use sotf_host::plugin::ProcessContext;

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
