use criterion::{Criterion, criterion_group, criterion_main};
use sotf_host::{Plugin, ProcessContext};
use sotf_plugin_ambisonics::{AmbisonicsDecoderConfig, AmbisonicsDecoderPlugin};

const SAMPLE_RATE: u32 = 48000;
const FRAME_SIZE: usize = 512;

fn benchmark_plugin(c: &mut Criterion, name: &str, mut plugin: AmbisonicsDecoderPlugin) {
    plugin.initialize(SAMPLE_RATE).unwrap();
    let in_ch = plugin.input_channels();
    let out_ch = plugin.output_channels();

    // Generate omni signal (W channel = sine)
    let input: Vec<f32> = (0..FRAME_SIZE * in_ch)
        .map(|i| {
            if i % in_ch == 0 {
                let t = (i / in_ch) as f32 / SAMPLE_RATE as f32;
                (t * 440.0 * 2.0 * std::f32::consts::PI).sin() * 0.5
            } else {
                0.0
            }
        })
        .collect();
    let mut output = vec![0.0_f32; FRAME_SIZE * out_ch];
    let ctx = ProcessContext {
        sample_rate: SAMPLE_RATE,
        num_frames: FRAME_SIZE,
    };

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
    benchmark_plugin(c, "Ambisonics_FOA_5.1", foa_5_1);

    // FOA -> 7.1.4 (4 channels in, 12 out)
    let foa_7_1_4 = AmbisonicsDecoderPlugin::new(&AmbisonicsDecoderConfig {
        order: 1,
        target_layout: "7.1.4".to_owned(),
        max_re_weighting: true,
        dual_band: false,
    })
    .unwrap();
    benchmark_plugin(c, "Ambisonics_FOA_7.1.4", foa_7_1_4);

    // SOA -> 7.1.4 (9 channels in, 12 out)
    let soa_7_1_4 = AmbisonicsDecoderPlugin::new(&AmbisonicsDecoderConfig {
        order: 2,
        target_layout: "7.1.4".to_owned(),
        max_re_weighting: true,
        dual_band: false,
    })
    .unwrap();
    benchmark_plugin(c, "Ambisonics_SOA_7.1.4", soa_7_1_4);

    // FOA -> 5.1 without max-rE (to measure max-rE overhead)
    let foa_5_1_no_maxre = AmbisonicsDecoderPlugin::new(&AmbisonicsDecoderConfig {
        order: 1,
        target_layout: "5.1".to_owned(),
        max_re_weighting: false,
        dual_band: false,
    })
    .unwrap();
    benchmark_plugin(c, "Ambisonics_FOA_5.1_noMaxRE", foa_5_1_no_maxre);
}

criterion_group!(benches, benchmark_ambisonics);
criterion_main!(benches);
