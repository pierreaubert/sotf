use criterion::{Criterion, criterion_group, criterion_main};
use sotf_host::{Plugin, ProcessContext};
use sotf_plugin_ambisonics::{AmbisonicsDecoderConfig, AmbisonicsDecoderPlugin};

const SAMPLE_RATE: u32 = 48000;
fn benchmark_plugin(
    c: &mut Criterion,
    name: &str,
    mut plugin: AmbisonicsDecoderPlugin,
    frame_size: usize,
) {
    plugin.initialize(SAMPLE_RATE).unwrap();
    let in_ch = plugin.input_channels();
    let out_ch = plugin.output_channels();

    // Generate omni signal (W channel = sine)
    let input: Vec<f32> = (0..frame_size * in_ch)
        .map(|i| {
            if i % in_ch == 0 {
                let t = (i / in_ch) as f32 / SAMPLE_RATE as f32;
                (t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5
            } else {
                0.0
            }
        })
        .collect();
    let mut output = vec![0.0_f32; frame_size * out_ch];
    let ctx = ProcessContext::new(SAMPLE_RATE, frame_size);

    c.bench_function(name, |b| {
        b.iter(|| {
            plugin
                .process(
                    std::hint::black_box(&input),
                    std::hint::black_box(&mut output),
                    std::hint::black_box(&ctx),
                )
                .unwrap();
        });
    });
}

fn benchmark_ambisonics(c: &mut Criterion) {
    // FOA -> 5.1 (4 channels in, 6 out)
    let foa_5_1 = AmbisonicsDecoderPlugin::new(&AmbisonicsDecoderConfig {
        order: 1,
        target_layout: "5.1".to_owned(),
        max_re_weighting: true,
        dual_band: false,
    })
    .unwrap();
    benchmark_plugin(c, "Ambisonics_FOA_5.1_512", foa_5_1, 512);

    // FOA -> 7.1.4 (4 channels in, 12 out)
    let foa_7_1_4 = AmbisonicsDecoderPlugin::new(&AmbisonicsDecoderConfig {
        order: 1,
        target_layout: "7.1.4".to_owned(),
        max_re_weighting: true,
        dual_band: false,
    })
    .unwrap();
    benchmark_plugin(c, "Ambisonics_FOA_7.1.4_512", foa_7_1_4, 512);

    // SOA -> 7.1.4 (9 channels in, 12 out)
    let soa_7_1_4 = AmbisonicsDecoderPlugin::new(&AmbisonicsDecoderConfig {
        order: 2,
        target_layout: "7.1.4".to_owned(),
        max_re_weighting: true,
        dual_band: false,
    })
    .unwrap();
    benchmark_plugin(c, "Ambisonics_SOA_7.1.4_512", soa_7_1_4, 512);

    for layout in sotf_plugin_ambisonics::params::TARGET_LAYOUTS {
        for dual_band in [false, true] {
            let plugin = AmbisonicsDecoderPlugin::new(&AmbisonicsDecoderConfig {
                order: 1,
                target_layout: (*layout).to_owned(),
                max_re_weighting: true,
                dual_band,
            })
            .unwrap();
            benchmark_plugin(
                c,
                &format!(
                    "Ambisonics_FOA_{}_{}_512",
                    layout,
                    if dual_band { "dual" } else { "single" }
                ),
                plugin,
                512,
            );
        }
    }

    for frame_size in [64, 512, 2048] {
        for dual_band in [false, true] {
            let toa = AmbisonicsDecoderPlugin::new(&AmbisonicsDecoderConfig {
                order: 3,
                target_layout: "9.1.6".to_owned(),
                max_re_weighting: true,
                dual_band,
            })
            .unwrap();
            benchmark_plugin(
                c,
                &format!(
                    "Ambisonics_TOA_9.1.6_{}_{}",
                    if dual_band { "dual" } else { "single" },
                    frame_size
                ),
                toa,
                frame_size,
            );
        }
    }
}

criterion_group!(benches, benchmark_ambisonics);
criterion_main!(benches);
