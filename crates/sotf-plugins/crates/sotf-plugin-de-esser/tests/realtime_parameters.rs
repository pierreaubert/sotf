use sotf_host::ParametricInPlacePlugin;
use sotf_host::{CountingAlloc, ParameterId, ParameterValue, assert_no_allocs};
use sotf_plugin_de_esser::DeEsserPlugin;

#[global_allocator]
static ALLOCATOR: CountingAlloc = CountingAlloc;

#[test]
fn realtime_parameter_updates_do_not_allocate() {
    let mut plugin = DeEsserPlugin::new(2);
    let updates = [
        ("threshold", ParameterValue::Float(-30.0)),
        ("ratio", ParameterValue::Float(8.0)),
        ("attack", ParameterValue::Float(2.0)),
        ("release", ParameterValue::Float(50.0)),
        ("mix", ParameterValue::Float(0.5)),
    ];
    let updates: Vec<_> = updates
        .into_iter()
        .map(|(name, value)| (ParameterId::from(name), value))
        .collect();
    for (id, value) in updates {
        assert_no_allocs("De-Esser realtime parameter update", || {
            plugin
                .parametric_set_parameter(id.clone(), value.clone())
                .unwrap();
        });
    }
}
