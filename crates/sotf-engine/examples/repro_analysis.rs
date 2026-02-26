use sotf_audio::signal_analysis::analyze_recording;
use sotf_audio::signals;

fn main() {
    // Generate a sweep
    let sample_rate = 48000;
    let duration = 2.0;
    let sweep = signals::gen_log_sweep(20.0, 20000.0, 0.5, sample_rate, duration);

    // Simulate a recording (just the sweep + some silence/delay)
    let mut recorded = vec![0.0; 4800]; // 0.1s delay
    recorded.extend_from_slice(&sweep);
    recorded.extend(vec![0.0; 48000]); // 1s tail

    // Write to temporary file as analyze_recording reads from file
    let path = std::path::Path::new("temp_repro.wav");
    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for &s in &recorded {
        writer.write_sample(s).unwrap();
    }
    writer.finalize().unwrap();

    // Analyze
    let result = analyze_recording(path, &sweep, sample_rate, Some((20.0, 20000.0))).unwrap();

    println!("Frequencies: {}", result.frequencies.len());
    println!("Distortion DB len: {}", result.harmonic_distortion_db.len());
    if !result.harmonic_distortion_db.is_empty() {
        println!(
            "Distortion[0] len: {}",
            result.harmonic_distortion_db[0].len()
        );
    }

    println!("RT60 len: {}", result.rt60_ms.len());
    println!("RT60 val[0]: {}", result.rt60_ms.first().unwrap_or(&-1.0));

    println!("Clarity C50 len: {}", result.clarity_c50_db.len());
    println!(
        "Clarity C50 val[0]: {}",
        result.clarity_c50_db.first().unwrap_or(&-1.0)
    );

    println!("Spectrogram len: {}", result.spectrogram_db.len());

    std::fs::remove_file(path).unwrap();
}
