use sotf_audio::{EmbeddedAudioEngine, EngineConfig, PluginConfig};
use sotf_plugins::ParameterValue;

#[test]
fn daw_external_clock_processes_sample_offset_automation_without_a_device() {
    let config = EngineConfig {
        frame_size: 8,
        plugins: vec![PluginConfig {
            plugin_type: "gain".to_string(),
            parameters: serde_json::json!({ "gain_db": 0.0 }),
        }],
        ..EngineConfig::default()
    };
    let (mut engine, diagnostics) = EmbeddedAudioEngine::new(&config).unwrap();
    assert!(diagnostics.is_empty());
    assert_eq!(engine.max_block_frames(), 8);

    engine
        .set_plugin_parameter_at(0, "gain_db", ParameterValue::Float(-12.0), 4)
        .unwrap();
    let input = [1.0; 16];
    let mut output = [0.0; 16];
    assert_eq!(engine.process_at(192_000, &input, &mut output).unwrap(), 8);
    assert!(
        output[..8]
            .iter()
            .all(|sample| (*sample - 1.0).abs() < 1e-5)
    );
    assert!(output[8..].iter().any(|sample| *sample < 0.9999));

    engine.reset_transport(384_000);
    let oversized = [0.0; 18];
    let error = engine
        .process_at(384_000, &oversized, &mut [0.0; 18])
        .unwrap_err();
    assert!(error.contains("exceeding configured maximum"));
}

#[test]
fn embedded_automation_rejects_structural_parameters_from_both_facades() {
    let config = EngineConfig {
        plugins: vec![PluginConfig::new(
            "upmixer",
            serde_json::json!({"speaker_config": "5.0"}),
        )],
        ..EngineConfig::default()
    };
    let (mut engine, diagnostics) = EmbeddedAudioEngine::new(&config).unwrap();
    assert!(diagnostics.is_empty());

    let error = engine
        .set_plugin_parameter_at(0, "speaker_config", ParameterValue::Int(3), 0)
        .unwrap_err();
    assert!(error.contains("requires rebuilding"), "{error}");

    let mut sender = engine.take_parameter_event_sender().unwrap();
    let error = sender
        .queue_plugin_parameter(0, "speaker_config".into(), ParameterValue::Int(3))
        .unwrap_err();
    assert!(error.contains("requires rebuilding"), "{error}");
}
