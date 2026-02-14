use criterion::{Criterion, criterion_group, criterion_main};
use math_audio_iir_fir::{Biquad, BiquadFilterType, Fir, WindowType};
use std::hint::black_box;

fn bench_biquad_process(c: &mut Criterion) {
    let mut biquad = Biquad::new(BiquadFilterType::Lowpass, 1000.0, 48000.0, 0.707, 0.0);
    let input = vec![0.5; 48000]; // 1 second of audio

    c.bench_function("biquad_process_loop_write", |b| {
        let mut buffer = input.clone();
        b.iter(|| {
            buffer.copy_from_slice(&input);
            for sample in buffer.iter_mut() {
                *sample = biquad.process(*sample);
            }
            black_box(&buffer);
        })
    });

    c.bench_function("biquad_process_block", |b| {
        let mut buffer = input.clone();
        b.iter(|| {
            buffer.copy_from_slice(&input);
            biquad.process_block(black_box(&mut buffer));
        })
    });
}

fn bench_fir_process(c: &mut Criterion) {
    // create a 101 tap FIR
    let mut fir = Fir::lowpass(101, 1000.0, 48000.0, WindowType::Hamming, 0.0);
    let input = vec![0.5; 4800]; // 0.1 second of audio (FIR is slow)

    c.bench_function("fir_process_loop", |b| {
        b.iter(|| {
            for &sample in &input {
                black_box(fir.process(sample));
            }
        })
    });

    c.bench_function("fir_process_block", |b| {
        let mut buffer = input.clone();
        b.iter(|| {
            buffer.copy_from_slice(&input);
            fir.process_block(black_box(&mut buffer));
        })
    });
}

criterion_group!(benches, bench_biquad_process, bench_fir_process);
criterion_main!(benches);
