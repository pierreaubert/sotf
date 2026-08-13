use sotf_host::parametric_in_place_plugin::ParametricInPlacePluginAdapter;
use sotf_host::plugin::{Plugin, ProcessContext};
use sotf_host::{AutoOversampledPlugin, CountingAlloc, assert_no_allocs};
use sotf_plugin_saturation::{SaturationPlugin, SaturationPluginParams};
use std::time::Instant;

#[global_allocator]
static ALLOCATOR: CountingAlloc = CountingAlloc;

fn main() {
    let inner = SaturationPlugin::from_params(
        2,
        SaturationPluginParams {
            mode: "Asymmetric".to_string(),
            drive: 10.0,
            oversampling: "4x".to_string(),
            dynamic_amount: 0.75,
            mix: 0.5,
            ..Default::default()
        },
    );
    let adapter = ParametricInPlacePluginAdapter::new(inner);
    let mut plugin = AutoOversampledPlugin::new(Box::new(adapter), 4).unwrap();
    plugin.initialize(48000).unwrap();
    let input: Vec<f32> = (0..1024)
        .flat_map(|frame| {
            let sample =
                (2.0 * std::f32::consts::PI * 8_000.0 * frame as f32 / 48_000.0).sin() * 0.5;
            [sample, -sample]
        })
        .collect();
    let mut output = vec![0.0; input.len()];
    let context = ProcessContext::new(48_000, 1024);
    plugin.process(&input, &mut output, &context).unwrap();
    assert_no_allocs("Saturation composed process", || {
        for _ in 0..1_000 {
            plugin.process(&input, &mut output, &context).unwrap();
        }
    });
    let start = Instant::now();
    for _ in 0..1_000 {
        plugin.process(&input, &mut output, &context).unwrap();
    }
    let cpu = start.elapsed().as_secs_f64() / (1_000.0 * 1024.0 / 48_000.0) * 100.0;
    assert!(output.iter().all(|sample| sample.is_finite()));
    let left_mean = output.iter().step_by(2).sum::<f32>() / 1024.0;
    let right_mean = output.iter().skip(1).step_by(2).sum::<f32>() / 1024.0;
    assert!(left_mean.is_finite() && right_mean.is_finite());
    println!(
        "Saturation Asymmetric stereo 4x path: {cpu:.2}% CPU, zero callback allocations, DC L/R={left_mean:.6}/{right_mean:.6}"
    );
}
