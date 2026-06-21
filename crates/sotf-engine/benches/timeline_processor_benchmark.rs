use criterion::{BatchSize, BenchmarkId, Criterion, criterion_group, criterion_main};
use sotf_audio::engine::AudioFrame;
use sotf_audio::timeline::{Clip, Region, Timeline, TimelineProcessor, Track};

fn create_test_wav(dir: &std::path::Path, frames: usize, sr: u32, ch: u16) -> std::path::PathBuf {
    let _ = std::fs::create_dir_all(dir);
    let src = dir.join("src.wav");

    let spec = hound::WavSpec {
        channels: ch,
        sample_rate: sr,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(&src, spec).unwrap();
    for _ in 0..frames {
        for _ in 0..ch {
            w.write_sample(0.5f32).unwrap();
        }
    }
    w.finalize().unwrap();
    src
}

fn make_timeline(src: &std::path::Path, frames: usize) -> Timeline {
    let mut tl = Timeline::new(1, 48000, 1024);
    let mut track = Track::new("T1", 1, 48000);
    track.add_region(Region::new(Clip::from_file(src, frames as u64), 0));
    tl.add_track(track);
    tl.build().unwrap();
    tl.transport.play();
    tl
}

fn benchmark_timeline_processor(c: &mut Criterion) {
    let mut group = c.benchmark_group("timeline_processor");

    for &frames in &[4096, 16384, 65536] {
        let dir = std::env::temp_dir().join(format!("sotf_timeline_bench_{}", frames));
        let src = create_test_wav(&dir, frames, 48000, 1);

        group.bench_with_input(
            BenchmarkId::new("next_frame_allocating", frames),
            &frames,
            |b, _| {
                b.iter_batched(
                    || TimelineProcessor::new(make_timeline(&src, frames)),
                    |mut proc| {
                        let _ = proc.next_frame();
                    },
                    BatchSize::SmallInput,
                );
            },
        );

        group.bench_with_input(
            BenchmarkId::new("next_frame_into_reused", frames),
            &frames,
            |b, _| {
                b.iter_batched(
                    || {
                        let proc = TimelineProcessor::new(make_timeline(&src, frames));
                        let frame = AudioFrame {
                            data: Vec::with_capacity(1024),
                            num_frames: 0,
                            num_channels: 1,
                            sample_rate: 48000,
                        };
                        (proc, frame)
                    },
                    |(mut proc, mut frame)| {
                        let _ = proc.next_frame_into(&mut frame);
                    },
                    BatchSize::SmallInput,
                );
            },
        );

        let _ = std::fs::remove_dir_all(&dir);
    }

    group.finish();
}

criterion_group!(benches, benchmark_timeline_processor);
criterion_main!(benches);
