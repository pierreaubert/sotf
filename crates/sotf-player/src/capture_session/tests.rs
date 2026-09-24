use super::*;

fn plan() -> CaptureSessionPlan {
    serde_json::from_str(include_str!("../../tests/fixtures/capture-session.json")).unwrap()
}

#[test]
fn two_and_four_microphone_plans_round_trip() {
    for count in [2, 4] {
        let mut value = plan();
        for index in 2..count {
            let mut mic = value.microphones[0].clone();
            mic.id = format!("mic-{index}");
            mic.device = format!("device-{index}");
            mic.position_m[0] = index as f64;
            value.microphones.push(mic);
        }
        let session = value.validate().unwrap();
        let json = serde_json::to_string(session.plan()).unwrap();
        let restored: CaptureSessionPlan = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.validate().unwrap().plan().microphones.len(), count);
    }
}

#[test]
fn invalid_plans_fail_before_hardware_access() {
    let cases: &[fn(&mut CaptureSessionPlan)] = &[
        |p| p.version = 2,
        |p| p.sample_rate_hz = 16_000,
        |p| p.output_device.clear(),
        |p| p.microphones.truncate(1),
        |p| p.microphones[1].id = p.microphones[0].id.clone(),
        |p| p.microphones[1].device = p.microphones[0].device.clone(),
        |p| p.microphones[0].calibration_file.clear(),
        |p| p.microphones[0].gain_db = f64::NAN,
        |p| p.microphones[0].input_channel = 64,
        |p| p.microphones[0].position_m[0] = f64::INFINITY,
        |p| p.microphones[0].position_uncertainty_mm = -1.0,
        |p| p.sources.clear(),
        |p| p.sources[0].output_channel = 64,
        |p| p.sources.push(p.sources[0].clone()),
        |p| p.sweep.duration_secs = f64::INFINITY,
        |p| p.sweep.duration_secs = 1e100,
        |p| p.sweep.duration_secs = 1e-100,
        |p| p.sweep.start_hz = p.sweep.end_hz,
        |p| p.sweep.end_hz = 24_000.0,
        |p| p.sweep.amplitude = 1.01,
        |p| p.sweep.amplitude = 0.0,
        |p| p.sweep.amplitude = 1e-100,
    ];
    for (index, change) in cases.iter().enumerate() {
        let mut value = plan();
        change(&mut value);
        assert!(value.validate().is_err(), "invalid case {index} accepted");
    }
}

#[test]
fn compact_geometry_requires_precise_distinct_positions() {
    let mut value = plan();
    value.geometry = CaptureGeometry::Compact;
    value.microphones[1].position_m = [0.05, 0.0, 0.0];
    assert!(value.clone().validate().is_ok());
    value.microphones[1].position_uncertainty_mm = 1.01;
    assert!(value.clone().validate().is_err());
    value.microphones[1].position_uncertainty_mm = 1.0;
    value.microphones[1].position_m = value.microphones[0].position_m;
    assert!(value.validate().is_err());
}

#[test]
fn unknown_fields_are_not_silently_ignored() {
    let mut value = serde_json::to_value(plan()).unwrap();
    value["microphones"][0]["gain"] = serde_json::json!(12);
    assert!(serde_json::from_value::<CaptureSessionPlan>(value).is_err());
}
