//! Cross-process worker for the Swift/Rust HAL transport stress test.

use driver_hal::SharedAudioBuffer;
use std::path::PathBuf;
use std::time::Duration;

fn parse_arguments() -> Result<(PathBuf, usize), String> {
    let mut arguments = std::env::args_os().skip(1);
    let path = arguments.next().map(PathBuf::from).ok_or_else(|| {
        "usage: hal_transport_worker <shared-memory-path> [iterations]".to_string()
    })?;
    let iterations = arguments
        .next()
        .map(|value| {
            value
                .to_string_lossy()
                .parse::<usize>()
                .map_err(|error| format!("invalid iteration count: {error}"))
        })
        .transpose()?
        .unwrap_or(200);
    if arguments.next().is_some() || iterations == 0 {
        return Err("usage: hal_transport_worker <shared-memory-path> [iterations]".to_string());
    }
    Ok((path, iterations))
}

fn run() -> Result<(), String> {
    let (path, iterations) = parse_arguments()?;
    let mut transport = SharedAudioBuffer::open(&path)
        .map_err(|error| format!("failed to open {}: {error}", path.display()))?;

    for iteration in 0..iterations {
        let channels = if iteration % 2 == 0 { 2 } else { 8 };
        let sample_rate = if iteration % 2 == 0 { 48_000 } else { 96_000 };
        transport.reconfigure_quiesced(Some(sample_rate), Some(64), Some(channels));
        if transport.channel_count() != channels || transport.sample_rate() != sample_rate {
            return Err(format!(
                "reconfiguration did not commit at iteration {iteration}: expected {sample_rate} Hz/{channels}ch, observed {} Hz/{}ch",
                transport.sample_rate(),
                transport.channel_count()
            ));
        }

        let frame_count = 32usize;
        let mut samples = vec![0.0f32; frame_count * channels as usize];
        for (sample_index, sample) in samples.iter_mut().enumerate() {
            *sample = ((iteration * 17 + sample_index) % 1024) as f32 / 1024.0;
        }
        let _ = transport.write_audio(&samples);
        std::thread::sleep(Duration::from_micros(200));
    }

    Ok(())
}

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}
