use super::Plugin;
use super::in_place_plugin::InPlacePlugin;
use super::in_place_plugin_adapter::InPlacePluginAdapter;
use super::loop_range::LoopRange;
use super::midi_event::MidiEvent;
use super::midi_message::MidiMessage;
use super::note_expression_event::NoteExpressionEvent;
use super::plugin_info::PluginInfo;
use super::process_context::ProcessContext;
use super::time_signature::TimeSignature;
use super::transport_info::TransportInfo;
use super::types::PluginResult;
use super::types::{NoteExpressionKind, PluginCompiledOp};
use crate::parameters::{Parameter, ParameterId, ParameterValue};

/// Minimal in-place plugin that uses all defaults.
struct DummyInPlacePlugin;

impl InPlacePlugin for DummyInPlacePlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Dummy", "0.0.1", "Test")
    }
    fn channels(&self) -> usize {
        2
    }
    fn parameters(&self) -> Vec<Parameter> {
        vec![]
    }
    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> PluginResult<()> {
        Ok(())
    }
    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
        None
    }
    fn process_in_place(
        &mut self,
        _buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        Ok(context.num_frames)
    }
}

/// In-place plugin that declares oversampling and f64 support.
struct OversampledPlugin;

impl InPlacePlugin for OversampledPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Oversampled", "0.0.1", "Test")
    }
    fn channels(&self) -> usize {
        2
    }
    fn parameters(&self) -> Vec<Parameter> {
        vec![]
    }
    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> PluginResult<()> {
        Ok(())
    }
    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
        None
    }
    fn process_in_place(
        &mut self,
        _buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        Ok(context.num_frames)
    }
    fn preferred_oversampling(&self) -> Option<u32> {
        Some(4)
    }
    fn supports_f64(&self) -> bool {
        true
    }
}

#[test]
fn test_default_oversampling_is_none() {
    let plugin = DummyInPlacePlugin;
    assert_eq!(plugin.preferred_oversampling(), None);
}

#[test]
fn test_default_f64_is_false() {
    let plugin = DummyInPlacePlugin;
    assert!(!plugin.supports_f64());
}

#[test]
fn test_adapter_forwards_oversampling() {
    let adapted = InPlacePluginAdapter::new(OversampledPlugin);
    assert_eq!(adapted.preferred_oversampling(), Some(4));
    assert!(adapted.supports_f64());
}

#[test]
fn test_adapter_forwards_defaults() {
    let adapted = InPlacePluginAdapter::new(DummyInPlacePlugin);
    assert_eq!(adapted.preferred_oversampling(), None);
    assert!(!adapted.supports_f64());
}

#[test]
fn process_context_defaults_to_musical_transport_without_midi() {
    let ctx = ProcessContext::new(48_000, 512);

    assert_eq!(ctx.sample_rate, 48_000);
    assert_eq!(ctx.num_frames, 512);
    assert!(ctx.transport.playing);
    assert_eq!(ctx.transport.bpm, 120.0);
    assert_eq!(ctx.transport.time_signature, TimeSignature::default());
    assert_eq!(ctx.transport.sample_position, 0);
    assert_eq!(ctx.transport.ppq_position, 0.0);
    assert!(ctx.transport.loop_range.is_none());
    assert!(ctx.midi_events.is_empty());
    assert!(ctx.note_expression_events.is_empty());
}

#[test]
fn process_context_tracks_sample_position_and_ppq() {
    let ctx = ProcessContext::new(48_000, 128).with_sample_position(48_000);

    assert_eq!(ctx.transport.sample_position, 48_000);
    assert!(
        (ctx.transport.ppq_position - 2.0).abs() < 1e-9,
        "120 bpm at 48 kHz should advance 2 quarter notes per second"
    );
}

#[test]
fn process_context_borrows_midi_events_without_copying() {
    let events = [MidiEvent::new(12, MidiMessage::note_on(0, 60, 100))];
    let ctx = ProcessContext::new(48_000, 128).with_midi_events(&events);

    assert_eq!(ctx.midi_events.len(), 1);
    assert_eq!(ctx.midi_events[0].sample_offset, 12);
    assert_eq!(ctx.midi_events[0].message.as_bytes(), &[0x90, 60, 100]);
}

#[test]
fn process_context_borrows_note_expression_events_without_copying() {
    let midi_events = [MidiEvent::new(12, MidiMessage::note_on(0, 60, 100))];
    let note_events = [NoteExpressionEvent::new(
        24,
        7,
        0,
        60,
        NoteExpressionKind::PitchBend,
        0.5,
    )];
    let ctx = ProcessContext::new(48_000, 128).with_events(&midi_events, &note_events);

    assert_eq!(ctx.midi_events.len(), 1);
    assert_eq!(ctx.note_expression_events.len(), 1);
    assert_eq!(ctx.note_expression_events[0].sample_offset, 24);
    assert_eq!(ctx.note_expression_events[0].note_id, 7);
    assert_eq!(
        ctx.note_expression_events[0].expression,
        NoteExpressionKind::PitchBend
    );
}

#[test]
fn transport_loop_range_validates_order() {
    assert_eq!(LoopRange::new(100, 100), None);
    let range = LoopRange::new(100, 200).unwrap();
    let transport = TransportInfo::default().with_loop_range(Some(range));

    assert!(transport.looping);
    assert_eq!(transport.loop_range, Some(range));
}

/// Plugin exposing float, int, and bool parameters for `validate_parameter` tests.
struct ValidatingPlugin;

impl InPlacePlugin for ValidatingPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Validating", "0.0.1", "Test")
    }

    fn channels(&self) -> usize {
        1
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![
            Parameter::new_float("gain_db", "Gain dB", 0.0, -60.0, 12.0),
            Parameter::new_int("num_bands", "Bands", 3, 1, 8),
            Parameter::new_bool("bypass", "Bypass", false),
            Parameter::new_string("preset", "Preset", String::from("default")),
        ]
    }

    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> PluginResult<()> {
        Ok(())
    }

    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
        None
    }

    fn process_in_place(
        &mut self,
        _buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        Ok(context.num_frames)
    }
}

#[test]
fn validate_parameter_accepts_valid_float() {
    let plugin = InPlacePluginAdapter::new(ValidatingPlugin);
    assert!(
        plugin
            .validate_parameter(&ParameterId::from("gain_db"), &ParameterValue::Float(0.0))
            .is_ok()
    );
    assert!(
        plugin
            .validate_parameter(&ParameterId::from("gain_db"), &ParameterValue::Float(-60.0))
            .is_ok()
    );
    assert!(
        plugin
            .validate_parameter(&ParameterId::from("gain_db"), &ParameterValue::Float(12.0))
            .is_ok()
    );
}

#[test]
fn validate_parameter_rejects_nan() {
    let plugin = InPlacePluginAdapter::new(ValidatingPlugin);
    let err = plugin
        .validate_parameter(
            &ParameterId::from("gain_db"),
            &ParameterValue::Float(f32::NAN),
        )
        .unwrap_err();
    assert!(err.contains("NaN"), "expected NaN error, got: {}", err);
}

#[test]
fn validate_parameter_rejects_infinite() {
    let plugin = InPlacePluginAdapter::new(ValidatingPlugin);
    let err = plugin
        .validate_parameter(
            &ParameterId::from("gain_db"),
            &ParameterValue::Float(f32::INFINITY),
        )
        .unwrap_err();
    assert!(
        err.contains("infinite"),
        "expected infinite error, got: {}",
        err
    );
}

#[test]
fn validate_parameter_rejects_float_below_minimum() {
    let plugin = InPlacePluginAdapter::new(ValidatingPlugin);
    let err = plugin
        .validate_parameter(
            &ParameterId::from("gain_db"),
            &ParameterValue::Float(-100.0),
        )
        .unwrap_err();
    assert!(err.contains("below minimum"), "got: {}", err);
}

#[test]
fn validate_parameter_rejects_float_above_maximum() {
    let plugin = InPlacePluginAdapter::new(ValidatingPlugin);
    let err = plugin
        .validate_parameter(&ParameterId::from("gain_db"), &ParameterValue::Float(100.0))
        .unwrap_err();
    assert!(err.contains("above maximum"), "got: {}", err);
}

#[test]
fn validate_parameter_accepts_valid_int() {
    let plugin = InPlacePluginAdapter::new(ValidatingPlugin);
    assert!(
        plugin
            .validate_parameter(&ParameterId::from("num_bands"), &ParameterValue::Int(1))
            .is_ok()
    );
    assert!(
        plugin
            .validate_parameter(&ParameterId::from("num_bands"), &ParameterValue::Int(8))
            .is_ok()
    );
}

#[test]
fn validate_parameter_rejects_int_below_minimum() {
    let plugin = InPlacePluginAdapter::new(ValidatingPlugin);
    let err = plugin
        .validate_parameter(&ParameterId::from("num_bands"), &ParameterValue::Int(0))
        .unwrap_err();
    assert!(err.contains("below minimum"), "got: {}", err);
}

#[test]
fn validate_parameter_rejects_int_above_maximum() {
    let plugin = InPlacePluginAdapter::new(ValidatingPlugin);
    let err = plugin
        .validate_parameter(&ParameterId::from("num_bands"), &ParameterValue::Int(100))
        .unwrap_err();
    assert!(err.contains("above maximum"), "got: {}", err);
}

#[test]
fn validate_parameter_accepts_bool() {
    let plugin = InPlacePluginAdapter::new(ValidatingPlugin);
    assert!(
        plugin
            .validate_parameter(&ParameterId::from("bypass"), &ParameterValue::Bool(true))
            .is_ok()
    );
    assert!(
        plugin
            .validate_parameter(&ParameterId::from("bypass"), &ParameterValue::Bool(false))
            .is_ok()
    );
}

#[test]
fn validate_parameter_accepts_string() {
    let plugin = InPlacePluginAdapter::new(ValidatingPlugin);
    assert!(
        plugin
            .validate_parameter(
                &ParameterId::from("preset"),
                &ParameterValue::String("custom".to_string()),
            )
            .is_ok()
    );
}

#[test]
fn validate_parameter_rejects_unknown_id() {
    let plugin = InPlacePluginAdapter::new(ValidatingPlugin);
    let err = plugin
        .validate_parameter(
            &ParameterId::from("not_a_param"),
            &ParameterValue::Float(0.0),
        )
        .unwrap_err();
    assert!(err.contains("Unknown parameter"), "got: {}", err);
}

#[test]
fn validate_parameter_rejects_type_mismatch() {
    let plugin = InPlacePluginAdapter::new(ValidatingPlugin);
    let err = plugin
        .validate_parameter(&ParameterId::from("gain_db"), &ParameterValue::Int(5))
        .unwrap_err();
    assert!(
        err.contains("type mismatch"),
        "expected type mismatch error, got: {}",
        err
    );
}

#[test]
fn parameter_id_from_str_and_display() {
    let id = ParameterId::from("gain_db");
    assert_eq!(id.as_str(), "gain_db");
    assert_eq!(format!("{}", id), "gain_db");
}

#[test]
fn parameter_id_equality_and_clone() {
    let a = ParameterId::from("gain_db");
    let b = ParameterId::from("gain_db");
    let c = ParameterId::from("mix");
    assert_eq!(a, b);
    assert_ne!(a, c);
    assert_eq!(a.clone(), b);
}

#[test]
fn validate_parameter_error_includes_parameter_id() {
    let plugin = InPlacePluginAdapter::new(ValidatingPlugin);
    let err = plugin
        .validate_parameter(
            &ParameterId::from("gain_db"),
            &ParameterValue::Float(f32::NAN),
        )
        .unwrap_err();
    assert!(
        err.contains("gain_db"),
        "error should name parameter: {}",
        err
    );
}

/// In-place plugin that scales samples by 2.0 to exercise the f64 adapter.
struct Scale2Plugin;

impl InPlacePlugin for Scale2Plugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("Scale2", "0.0.1", "Test")
    }
    fn channels(&self) -> usize {
        2
    }
    fn parameters(&self) -> Vec<Parameter> {
        vec![]
    }
    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> PluginResult<()> {
        Ok(())
    }
    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
        None
    }
    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        for s in buffer.iter_mut() {
            *s *= 2.0;
        }
        Ok(context.num_frames)
    }
    fn supports_f64(&self) -> bool {
        true
    }
}

#[test]
fn in_place_adapter_f64_reuses_scratch_and_produces_correct_output() {
    let mut adapted = InPlacePluginAdapter::new(Scale2Plugin);
    let context = ProcessContext::new(48_000, 128);
    let input: Vec<f64> = (0..256).map(|i| i as f64 * 0.01).collect();
    let mut output = vec![0.0f64; 256];

    // Warm up to allocate scratch buffers.
    adapted.process_f64(&input, &mut output, &context).unwrap();

    // Second call should reuse scratch buffers (no per-block allocation).
    let result = adapted.process_f64(&input, &mut output, &context).unwrap();
    assert_eq!(result, 128);
    for (i, &s) in output.iter().enumerate() {
        let expected = (i as f64 * 0.01) * 2.0;
        assert!(
            (s - expected).abs() < 1e-6,
            "sample {}: got {}, expected {}",
            i,
            s,
            expected
        );
    }
}

#[derive(Default)]
struct StatefulValidationPlugin {
    process_calls: usize,
    compiled_calls: usize,
}

impl InPlacePlugin for StatefulValidationPlugin {
    fn info(&self) -> PluginInfo {
        PluginInfo::new("StatefulValidation", "0.0.1", "Test")
    }

    fn channels(&self) -> usize {
        2
    }

    fn parameters(&self) -> Vec<Parameter> {
        vec![]
    }

    fn set_parameter(&mut self, _id: ParameterId, _value: ParameterValue) -> PluginResult<()> {
        Ok(())
    }

    fn get_parameter(&self, _id: &ParameterId) -> Option<ParameterValue> {
        None
    }

    fn process_in_place(
        &mut self,
        buffer: &mut [f32],
        context: &ProcessContext,
    ) -> PluginResult<usize> {
        self.process_calls += 1;
        for sample in buffer {
            *sample += 1.0;
        }
        Ok(context.num_frames)
    }

    fn process_compiled_f32(
        &mut self,
        _op: PluginCompiledOp,
        input: &[f32],
        output: &mut [f32],
        context: &ProcessContext,
    ) -> Option<PluginResult<usize>> {
        self.compiled_calls += 1;
        output.copy_from_slice(input);
        Some(Ok(context.num_frames))
    }
}

#[test]
fn in_place_adapter_rejects_malformed_or_nonfinite_blocks_transactionally() {
    let mut adapted = InPlacePluginAdapter::new(StatefulValidationPlugin::default());
    let context = ProcessContext::new(48_000, 4);
    let valid = vec![0.25; 8];
    let mut output = vec![7.0; 8];

    for input in [
        vec![0.25; 7],
        vec![0.25; 9],
        vec![f32::NAN; 8],
        vec![f32::INFINITY; 8],
        vec![f32::NEG_INFINITY; 8],
    ] {
        let before = output.clone();
        let error = adapted.process(&input, &mut output, &context).unwrap_err();
        assert!(
            error.contains("expected") || error.contains("non-finite"),
            "unexpected validation error: {error}"
        );
        assert_eq!(output, before, "rejected input must not alter output");
        assert_eq!(adapted.plugin.process_calls, 0, "DSP state advanced");
    }

    let mut short_output = vec![7.0; 7];
    assert!(
        adapted
            .process(&valid, &mut short_output, &context)
            .is_err()
    );
    assert_eq!(short_output, vec![7.0; 7]);
    assert_eq!(adapted.plugin.process_calls, 0);

    assert_eq!(adapted.process(&valid, &mut output, &context).unwrap(), 4);
    assert_eq!(adapted.plugin.process_calls, 1);
    assert_eq!(output, vec![1.25; 8]);
}

#[test]
fn compiled_adapter_uses_the_same_transactional_block_validation() {
    let mut adapted = InPlacePluginAdapter::new(StatefulValidationPlugin::default());
    let context = ProcessContext::new(48_000, 4);
    let mut output = vec![3.0; 8];

    let result = adapted
        .process_compiled_f32(
            PluginCompiledOp::ApplyGain,
            &[f32::NAN; 8],
            &mut output,
            &context,
        )
        .expect("invalid compiled blocks must return an explicit error");
    assert!(result.unwrap_err().contains("non-finite"));
    assert_eq!(output, vec![3.0; 8]);
    assert_eq!(adapted.plugin.compiled_calls, 0);
}
