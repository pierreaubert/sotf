use sotf_host::ParametricInPlacePlugin;
use sotf_host::{CountingAlloc, ParameterId, ParameterValue, assert_no_allocs};
use sotf_plugin_gate::{GatePlugin, GatePluginParams};

#[global_allocator]
static ALLOCATOR: CountingAlloc = CountingAlloc;

#[test]
fn realtime_parameter_updates_and_reset_do_not_allocate() {
    let mut gate = GatePlugin::try_from_params(
        12,
        GatePluginParams {
            sidechain_hpf_hz: 120.0,
            sidechain_hpf_order: "4th".into(),
            ..GatePluginParams::default()
        },
    )
    .unwrap();
    gate.initialize(48_000).unwrap();
    let updates = [
        ("threshold", ParameterValue::Float(-35.0)),
        ("ratio", ParameterValue::Float(20.0)),
        ("attack", ParameterValue::Float(5.0)),
        ("hold", ParameterValue::Float(25.0)),
        ("release", ParameterValue::Float(200.0)),
        ("mix", ParameterValue::Float(0.5)),
        ("range_db", ParameterValue::Float(60.0)),
        ("hysteresis_db", ParameterValue::Float(3.0)),
        ("knee_db", ParameterValue::Float(4.0)),
    ];
    let updates: Vec<_> = updates
        .into_iter()
        .map(|(id, value)| (ParameterId::from(id), value))
        .collect();
    for (id, value) in updates {
        assert_no_allocs("Gate realtime parameter update", || {
            gate.parametric_set_parameter(id.clone(), value.clone())
                .unwrap();
        });
    }
    assert_no_allocs("Gate reset", || gate.reset());
}
