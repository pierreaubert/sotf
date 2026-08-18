use super::super::hal_input_reader::HalInputReader;
use super::super::shared_audio_buffer::SharedAudioBuffer;
use crate::encryption::AudioCipher;
use proptest::prelude::*;
use std::env;
use std::fs;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::Ordering;
use std::thread;
use std::time::{Duration, Instant};

use tempfile::NamedTempFile;

fn create_mock_shared_memory(
    sample_rate: u32,
    buffer_frames: u32,
    channel_count: u32,
) -> NamedTempFile {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let buffer = SharedAudioBuffer::create_or_open(
        temp_file.path(),
        sample_rate,
        buffer_frames,
        channel_count,
    )
    .expect("Failed to create mock shared memory");
    buffer.header().driver_ready.store(1, Ordering::Release);
    buffer.header().active.store(1, Ordering::Release);
    drop(buffer);
    temp_file
}

fn create_mock_shared_memory_with_max_geometry(
    sample_rate: u32,
    buffer_frames: u32,
    channel_count: u32,
    max_buffer_frames: u32,
    max_channel_count: u32,
) -> NamedTempFile {
    let temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let buffer = SharedAudioBuffer::create_or_open_with_max_geometry(
        temp_file.path(),
        sample_rate,
        buffer_frames,
        channel_count,
        max_buffer_frames,
        max_channel_count,
    )
    .expect("Failed to create mock shared memory with max geometry");
    buffer.header().driver_ready.store(1, Ordering::Release);
    buffer.header().active.store(1, Ordering::Release);
    drop(buffer);
    temp_file
}

#[test]
fn test_shared_memory_roundtrip_bit_exact() {
    let sample_rate = 48000;
    let buffer_frames = 1024;
    let channel_count = 2;
    let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);

    let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

    assert_eq!(buffer.sample_rate(), sample_rate);
    assert_eq!(buffer.buffer_frames(), buffer_frames);
    assert_eq!(buffer.channel_count(), channel_count);
    assert!(buffer.driver_ready());

    let num_samples = buffer_frames as usize * channel_count as usize;
    let input_audio: Vec<f32> = (0..num_samples)
        .map(|i| {
            let t = i as f32 / sample_rate as f32;
            (2.0 * std::f32::consts::PI * 440.0 * t).sin() * 0.5
        })
        .collect();

    let frames_written = buffer.write_audio(&input_audio);
    assert_eq!(frames_written, buffer_frames as usize);

    let mut output_audio = vec![0.0f32; num_samples];
    let frames_read = buffer.read_audio(&mut output_audio);
    assert_eq!(frames_read, buffer_frames as usize);

    for (i, (input, output)) in input_audio.iter().zip(output_audio.iter()).enumerate() {
        assert_eq!(input.to_bits(), output.to_bits(), "Sample {} mismatch", i);
    }
}

#[test]
fn plain_writer_never_commits_a_partial_interleaved_frame() {
    let temp_file = create_mock_shared_memory(48_000, 4, 2);
    let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");
    let capacity = buffer.current_audio_capacity() as u64;

    buffer
        .header()
        .write_position
        .store(capacity - 1, Ordering::Release);
    buffer.header().read_position.store(0, Ordering::Release);
    assert_eq!(buffer.write_audio(&[1.0, 2.0]), 0);
    assert_eq!(
        buffer.header().write_position.load(Ordering::Acquire),
        capacity - 1
    );

    // Three free slots can accept exactly one stereo frame, including across
    // the physical wrap boundary, while leaving the odd remainder unused.
    buffer
        .header()
        .write_position
        .store(capacity - 3, Ordering::Release);
    assert_eq!(buffer.write_audio(&[3.0, 4.0, 5.0, 6.0]), 1);
    assert_eq!(
        buffer.header().write_position.load(Ordering::Acquire),
        capacity - 1
    );
}

#[test]
fn plain_io_does_not_publish_a_cursor_while_reconfigure_owns_commit_state() {
    let temp_file = create_mock_shared_memory(48_000, 16, 2);
    let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

    buffer.header().configuring.store(
        super::super::shared_audio_buffer::CONFIGURING_WRITE_COMMIT,
        Ordering::Release,
    );
    assert_eq!(buffer.write_audio(&[1.0, 2.0]), 0);
    assert_eq!(buffer.header().write_position.load(Ordering::Acquire), 0);

    buffer.header().configuring.store(
        super::super::shared_audio_buffer::CONFIGURING_READ_COMMIT,
        Ordering::Release,
    );
    let mut output = [9.0_f32, 9.0_f32];
    assert_eq!(buffer.read_audio(&mut output), 0);
    assert_eq!(output, [0.0, 0.0]);
    assert_eq!(buffer.header().read_position.load(Ordering::Acquire), 0);

    buffer.header().configuring.store(0, Ordering::Release);
    assert_eq!(buffer.write_audio(&[1.0, 2.0]), 1);
    assert_eq!(buffer.read_audio(&mut output), 1);
    assert_eq!(output, [1.0, 2.0]);
}

#[test]
fn cursor_commit_is_rejected_after_reconfiguration_claims_the_word() {
    let temp_file = create_mock_shared_memory(48_000, 16, 2);
    let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

    // Simulate an IO cycle that has finished copying but has not yet published
    // its cursor. Reconfiguration owns the word before the publication step.
    buffer.copy_audio_slots_from(0, &[1.0, 2.0]);
    buffer.header().configuring.store(
        super::super::shared_audio_buffer::CONFIGURING_RECONFIGURE,
        Ordering::Release,
    );
    assert!(!buffer.commit_write_position(2));
    assert_eq!(buffer.header().write_position.load(Ordering::Acquire), 0);

    assert!(!buffer.commit_read_position(2));
    assert_eq!(buffer.header().read_position.load(Ordering::Acquire), 0);
}

#[test]
fn encrypted_io_does_not_publish_a_cursor_while_reconfigure_owns_commit_state() {
    let temp_file = create_mock_shared_memory(48_000, 512, 2);
    let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");
    let cipher = test_audio_cipher();
    buffer.set_key_fingerprint(*cipher.fingerprint());
    buffer.set_encrypted(true);
    let (mut ciphertext_buf, mut encrypted_buf) = encrypted_staging_buffers();
    let samples = sequential_audio(32, 2, 0);

    buffer.header().configuring.store(
        super::super::shared_audio_buffer::CONFIGURING_WRITE_COMMIT,
        Ordering::Release,
    );
    assert_eq!(
        buffer.write_audio_encrypted_into(
            &samples,
            &cipher,
            &mut ciphertext_buf,
            &mut encrypted_buf,
        ),
        0
    );
    assert_eq!(buffer.header().write_position.load(Ordering::Acquire), 0);

    buffer.header().configuring.store(0, Ordering::Release);
    assert_eq!(
        buffer.write_audio_encrypted_into(
            &samples,
            &cipher,
            &mut ciphertext_buf,
            &mut encrypted_buf,
        ),
        32
    );

    buffer.header().configuring.store(
        super::super::shared_audio_buffer::CONFIGURING_READ_COMMIT,
        Ordering::Release,
    );
    let mut output = vec![9.0_f32; samples.len()];
    assert_eq!(
        buffer.read_audio_encrypted_into(
            &mut output,
            &cipher,
            &mut encrypted_buf,
            &mut ciphertext_buf,
        ),
        0
    );
    assert!(output.iter().all(|sample| *sample == 0.0));
    assert_eq!(buffer.header().read_position.load(Ordering::Acquire), 0);
}

#[test]
fn deterministic_ring_sequence_preserves_interleaved_frame_order_across_wraps() {
    let temp_file = create_mock_shared_memory(48_000, 8, 2);
    let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("open shared memory");
    let mut expected = std::collections::VecDeque::new();
    let mut next_sample = 0.0_f32;

    for step in 0..256 {
        let requested_frames = (step % 3) + 1;
        let capacity = buffer.current_audio_capacity() / 2;
        let free_frames = capacity.saturating_sub(expected.len());
        let frames_to_write = requested_frames.min(free_frames);
        if frames_to_write > 0 {
            let input: Vec<f32> = (0..frames_to_write * 2)
                .map(|_| {
                    let sample = next_sample;
                    next_sample += 1.0;
                    sample
                })
                .collect();
            assert_eq!(buffer.write_audio(&input), frames_to_write);
            expected.extend(input);
        }

        if step % 2 == 0 {
            let mut output = vec![0.0_f32; 6];
            let frames_read = buffer.read_audio(&mut output);
            let sample_count = frames_read * 2;
            for sample in output.iter().take(sample_count) {
                assert_eq!(
                    Some(sample.to_bits()),
                    expected.pop_front().map(|v| v.to_bits())
                );
            }
        }
    }

    while !expected.is_empty() {
        let mut output = [0.0_f32; 6];
        let frames_read = buffer.read_audio(&mut output);
        assert!(frames_read > 0, "pending frames must remain readable");
        for sample in output.iter().take(frames_read * 2) {
            assert_eq!(
                Some(sample.to_bits()),
                expected.pop_front().map(|v| v.to_bits())
            );
        }
    }
}

proptest! {
    #[test]
    fn property_ring_preserves_order_for_random_io_sequences(
        buffer_frames in 2_u32..16,
        channel_count in 1_u32..5,
        encrypted in any::<bool>(),
        operations in prop::collection::vec((1_usize..9, 1_usize..9), 1..128),
    ) {
        let temp_file = create_mock_shared_memory(48_000, buffer_frames, channel_count);
        let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("open shared memory");
        let cipher = test_audio_cipher();
        if encrypted {
            buffer.set_key_fingerprint(*cipher.fingerprint());
            buffer.set_encrypted(true);
        }

        let mut expected = std::collections::VecDeque::new();
        let mut next_sample = 0.0_f32;

        for (write_frames, read_frames) in operations {
            let input: Vec<f32> = (0..write_frames * channel_count as usize)
                .map(|_| {
                    let sample = next_sample;
                    next_sample += 1.0;
                    sample
                })
                .collect();
            let frames_written = if encrypted {
                buffer.write_audio_encrypted(&input, &cipher)
            } else {
                buffer.write_audio(&input)
            };
            expected.extend(input.into_iter().take(
                frames_written * channel_count as usize
            ));

            // Exercise the read-side frame rounding with an occasional extra
            // sample. The extra slot must never advance the channel cursor.
            let extra_sample = if channel_count > 1 { 1 } else { 0 };
            let mut output = vec![0.0_f32; read_frames * channel_count as usize + extra_sample];
            let frames_read = if encrypted {
                buffer.read_audio_encrypted(&mut output, &cipher)
            } else {
                buffer.read_audio(&mut output)
            };
            for sample in output
                .iter()
                .take(frames_read * channel_count as usize)
            {
                prop_assert_eq!(
                    Some(sample.to_bits()),
                    expected.pop_front().map(|value| value.to_bits())
                );
            }
        }

        while !expected.is_empty() {
            let mut output = vec![0.0_f32; 8 * channel_count as usize];
            let frames_read = if encrypted {
                buffer.read_audio_encrypted(&mut output, &cipher)
            } else {
                buffer.read_audio(&mut output)
            };
            prop_assert!(frames_read > 0, "pending frames must remain readable");
            for sample in output
                .iter()
                .take(frames_read * channel_count as usize)
            {
                prop_assert_eq!(
                    Some(sample.to_bits()),
                    expected.pop_front().map(|value| value.to_bits())
                );
            }
        }
    }

    #[test]
    fn property_reconfiguration_preserves_frame_aligned_positions(
        operations in prop::collection::vec((1_usize..8, 0_u32..2), 1..96),
    ) {
        let temp_file = create_mock_shared_memory_with_max_geometry(
            48_000,
            8,
            2,
            32,
            8,
        );
        let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("open shared memory");
        let cipher = test_audio_cipher();
        buffer.set_key_fingerprint(*cipher.fingerprint());

        for (requested_frames, channel_variant) in operations {
            let channels = if channel_variant == 0 { 2 } else { 4 };
            buffer.reconfigure_quiesced(
                None,
                Some(requested_frames as u32 + 1),
                Some(channels),
            );

            let active_frames = buffer.buffer_frames() as usize;
            let active_channels = buffer.channel_count() as usize;
            prop_assert_eq!(active_channels, channels as usize);
            prop_assert!(active_frames >= 2);

            let input = sequential_audio(
                requested_frames.min(active_frames),
                active_channels,
                0,
            );
            let written = buffer.write_audio(&input);
            let mut output = vec![0.0; input.len() + active_channels];
            let read = buffer.read_audio(&mut output);
            prop_assert!(written <= input.len() / active_channels);
            prop_assert!(read <= output.len() / active_channels);
            prop_assert_eq!(
                buffer.header().write_position.load(Ordering::Acquire)
                    % active_channels as u64,
                0
            );
            prop_assert_eq!(
                buffer.header().read_position.load(Ordering::Acquire)
                    % active_channels as u64,
                0
            );
        }
    }
}

#[test]
fn cross_process_plain_ring_stress_reconfigures_only_between_spsc_commits() {
    let temp_file = create_mock_shared_memory_with_max_geometry(48_000, 8, 2, 64, 8);
    let mut writer = SharedAudioBuffer::open(temp_file.path()).expect("open writer");
    let reader = SharedAudioBuffer::open(temp_file.path()).expect("open reader");
    let mut controller = SharedAudioBuffer::open(temp_file.path()).expect("open controller");

    let writer_errors = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let reader_errors = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let writer_error_count = std::sync::Arc::clone(&writer_errors);
    let writer_thread = std::thread::spawn(move || {
        for step in 0..2_000 {
            let channels = writer.channel_count() as usize;
            if channels == 0 {
                writer_error_count.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            let requested_frames = (step % 4) + 1;
            let input = sequential_audio(requested_frames, channels, step * 16);
            let written = writer.write_audio(&input);
            if written > requested_frames || written * channels > input.len() {
                writer_error_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    });

    let reader_error_count = std::sync::Arc::clone(&reader_errors);
    let reader_thread = std::thread::spawn(move || {
        let reader = reader;
        let mut output = vec![0.0_f32; 64];
        for _ in 0..2_000 {
            let channels = reader.channel_count() as usize;
            if channels == 0 {
                reader_error_count.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            let read = reader.read_audio(&mut output);
            if read > output.len() / channels || output.iter().any(|sample| !sample.is_finite()) {
                reader_error_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    });

    for step in 0..128 {
        let channels = if step % 2 == 0 { 4 } else { 2 };
        controller.reconfigure_quiesced(None, Some(8), Some(channels));
        assert_eq!(
            controller.header().configuring.load(Ordering::Acquire)
                & super::super::shared_audio_buffer::CONFIGURING_RECONFIGURE,
            0,
            "reconfiguration must release its ownership bit"
        );
    }

    writer_thread
        .join()
        .expect("writer thread must not panic");
    reader_thread
        .join()
        .expect("reader thread must not panic");
    assert_eq!(writer_errors.load(Ordering::Relaxed), 0);
    assert_eq!(reader_errors.load(Ordering::Relaxed), 0);

    // After the concurrent run, the next frame must still use the active
    // channel geometry rather than a stale pre-reconfiguration cursor.
    controller.reconfigure_quiesced(None, Some(8), Some(2));
    let mut post_writer = SharedAudioBuffer::open(temp_file.path()).expect("open post writer");
    let post_reader = SharedAudioBuffer::open(temp_file.path()).expect("open post reader");
    let expected = [0.25_f32, -0.5_f32];
    assert_eq!(post_writer.write_audio(&expected), 1);
    let mut actual = [0.0_f32; 2];
    assert_eq!(post_reader.read_audio(&mut actual), 1);
    assert_eq!(actual, expected);
}

#[test]
fn cross_process_encrypted_ring_stress_reconfigures_without_stale_records() {
    let temp_file = create_mock_shared_memory_with_max_geometry(48_000, 16, 2, 64, 8);
    let mut writer = SharedAudioBuffer::open(temp_file.path()).expect("open encrypted writer");
    let reader = SharedAudioBuffer::open(temp_file.path()).expect("open encrypted reader");
    let mut controller = SharedAudioBuffer::open(temp_file.path()).expect("open controller");
    let writer_key = [0x21_u8; 32];
    let reader_key = writer_key;
    writer.set_key_fingerprint(*AudioCipher::new(&writer_key).fingerprint());
    writer.set_encrypted(true);

    let writer_errors = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let reader_errors = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));

    let writer_error_count = std::sync::Arc::clone(&writer_errors);
    let writer_thread = std::thread::spawn(move || {
        let cipher = AudioCipher::new(&writer_key);
        let (mut ciphertext, mut encrypted) = encrypted_staging_buffers();
        let mut writer = writer;
        for step in 0..512 {
            let channels = writer.channel_count() as usize;
            if channels == 0 {
                writer_error_count.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            let frames = (step % 4) + 1;
            let samples = sequential_audio(frames, channels, step * 32);
            let written = writer.write_audio_encrypted_into(
                &samples,
                &cipher,
                &mut ciphertext,
                &mut encrypted,
            );
            if written > frames {
                writer_error_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    });

    let reader_error_count = std::sync::Arc::clone(&reader_errors);
    let reader_thread = std::thread::spawn(move || {
        let cipher = AudioCipher::new(&reader_key);
        let (mut encrypted, mut ciphertext) = encrypted_staging_buffers();
        let reader = reader;
        let mut output = vec![0.0_f32; 64];
        for _ in 0..512 {
            let channels = reader.channel_count() as usize;
            if channels == 0 {
                reader_error_count.fetch_add(1, Ordering::Relaxed);
                continue;
            }
            let read = reader.read_audio_encrypted_into(
                &mut output,
                &cipher,
                &mut ciphertext,
                &mut encrypted,
            );
            if read > output.len() / channels || output.iter().any(|sample| !sample.is_finite()) {
                reader_error_count.fetch_add(1, Ordering::Relaxed);
            }
        }
    });

    for step in 0..64 {
        controller.reconfigure_quiesced(None, Some(16), Some(if step % 2 == 0 { 4 } else { 2 }));
    }

    let _writer = writer_thread.join().expect("encrypted writer must not panic");
    let _reader = reader_thread.join().expect("encrypted reader must not panic");
    assert_eq!(writer_errors.load(Ordering::Relaxed), 0);
    assert_eq!(reader_errors.load(Ordering::Relaxed), 0);

    // Verify that the post-reconfiguration record stream is still readable
    // with the same key and starts on a complete channel frame.
    controller.reconfigure_quiesced(None, Some(16), Some(2));
    let mut post_writer = SharedAudioBuffer::open(temp_file.path()).expect("open post writer");
    let post_reader = SharedAudioBuffer::open(temp_file.path()).expect("open post reader");
    post_writer.set_key_fingerprint(*AudioCipher::new(&writer_key).fingerprint());
    post_writer.set_encrypted(true);
    let cipher = AudioCipher::new(&writer_key);
    let (mut ciphertext, mut encrypted) = encrypted_staging_buffers();
    let expected = [0.25_f32, -0.5_f32];
    assert_eq!(
        post_writer.write_audio_encrypted_into(
            &expected,
            &cipher,
            &mut ciphertext,
            &mut encrypted,
        ),
        1
    );
    let mut actual = [0.0_f32; 2];
    assert_eq!(
        post_reader.read_audio_encrypted_into(
            &mut actual,
            &cipher,
            &mut encrypted,
            &mut ciphertext,
        ),
        1
    );
    assert_eq!(actual, expected);
}

const PROCESS_STRESS_PATH: &str = "SOTF_DRIVER_HAL_PROCESS_STRESS_PATH";
const PROCESS_STRESS_READY: &str = "SOTF_DRIVER_HAL_PROCESS_STRESS_READY";
const PROCESS_STRESS_MODE: &str = "SOTF_DRIVER_HAL_PROCESS_STRESS_MODE";
const PROCESS_STRESS_ROLE: &str = "SOTF_DRIVER_HAL_PROCESS_STRESS_ROLE";

fn spawn_process_ring_worker(
    path: &std::path::Path,
    ready_path: &std::path::Path,
    mode: &str,
    role: &str,
) -> Child {
    Command::new(env::current_exe().expect("current test executable"))
        .arg("cross_process_ring_worker")
        .arg("--nocapture")
        .env(PROCESS_STRESS_PATH, path)
        .env(PROCESS_STRESS_READY, ready_path)
        .env(PROCESS_STRESS_MODE, mode)
        .env(PROCESS_STRESS_ROLE, role)
        .env("RUST_BACKTRACE", "1")
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn shared-memory worker process")
}

fn wait_for_worker_ready(paths: &[&std::path::Path]) {
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if paths.iter().all(|path| path.exists()) {
            return;
        }
        thread::sleep(Duration::from_millis(2));
    }
    panic!("shared-memory worker processes did not become ready");
}

fn wait_for_worker(child: Child, role: &str) {
    let output = child
        .wait_with_output()
        .unwrap_or_else(|error| panic!("wait for {role} worker: {error}"));
    assert!(
        output.status.success(),
        "{role} worker failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_true_cross_process_ring_stress(mode: &str) {
    let temp_file = create_mock_shared_memory_with_max_geometry(48_000, 8, 2, 64, 8);
    let mut controller = SharedAudioBuffer::open(temp_file.path()).expect("open controller");

    if mode == "encrypted" {
        let cipher = AudioCipher::new(&[0x21_u8; 32]);
        controller.set_key_fingerprint(*cipher.fingerprint());
        controller.set_encrypted(true);
    }

    let writer_ready = NamedTempFile::new().expect("writer ready file");
    let reader_ready = NamedTempFile::new().expect("reader ready file");
    fs::remove_file(writer_ready.path()).expect("remove writer ready placeholder");
    fs::remove_file(reader_ready.path()).expect("remove reader ready placeholder");

    let writer = spawn_process_ring_worker(
        temp_file.path(),
        writer_ready.path(),
        mode,
        "writer",
    );
    let reader = spawn_process_ring_worker(
        temp_file.path(),
        reader_ready.path(),
        mode,
        "reader",
    );
    wait_for_worker_ready(&[writer_ready.path(), reader_ready.path()]);

    for step in 0..256 {
        let channels = if step % 2 == 0 { 4 } else { 2 };
        controller.reconfigure_quiesced(None, Some(8), Some(channels));
        assert_eq!(
            controller.header().configuring.load(Ordering::Acquire)
                & super::super::shared_audio_buffer::CONFIGURING_RECONFIGURE,
            0,
            "cross-process reconfiguration must release its ownership bit"
        );
    }

    wait_for_worker(writer, "writer");
    wait_for_worker(reader, "reader");

    // Prove the mappings still agree after the other processes have exited and
    // that a complete frame can be exchanged at the final geometry.
    controller.reconfigure_quiesced(None, Some(8), Some(2));
    let mut post_writer = SharedAudioBuffer::open(temp_file.path()).expect("open post writer");
    let post_reader = SharedAudioBuffer::open(temp_file.path()).expect("open post reader");
    let expected = [0.25_f32, -0.5_f32];
    if mode == "encrypted" {
        let cipher = AudioCipher::new(&[0x21_u8; 32]);
        post_writer.set_key_fingerprint(*cipher.fingerprint());
        post_writer.set_encrypted(true);
        let (mut ciphertext, mut encrypted) = encrypted_staging_buffers();
        assert_eq!(
            post_writer.write_audio_encrypted_into(
                &expected,
                &cipher,
                &mut ciphertext,
                &mut encrypted,
            ),
            1
        );
        let mut actual = [0.0_f32; 2];
        assert_eq!(
            post_reader.read_audio_encrypted_into(
                &mut actual,
                &cipher,
                &mut encrypted,
                &mut ciphertext,
            ),
            1
        );
        assert_eq!(actual, expected);
    } else {
        assert_eq!(post_writer.write_audio(&expected), 1);
        let mut actual = [0.0_f32; 2];
        assert_eq!(post_reader.read_audio(&mut actual), 1);
        assert_eq!(actual, expected);
    }
}

#[test]
fn true_cross_process_plain_ring_stress_reconfigures_without_stale_records() {
    run_true_cross_process_ring_stress("plain");
}

#[test]
fn true_cross_process_encrypted_ring_stress_reconfigures_without_stale_records() {
    run_true_cross_process_ring_stress("encrypted");
}

#[test]
fn cross_process_ring_worker() {
    let (Some(path), Some(ready), Some(mode), Some(role)) = (
        env::var_os(PROCESS_STRESS_PATH),
        env::var_os(PROCESS_STRESS_READY),
        env::var(PROCESS_STRESS_MODE).ok(),
        env::var(PROCESS_STRESS_ROLE).ok(),
    ) else {
        return;
    };

    let mut buffer = SharedAudioBuffer::open(path).expect("open worker mapping");
    let cipher = AudioCipher::new(&[0x21_u8; 32]);
    if mode == "encrypted" {
        buffer.set_key_fingerprint(*cipher.fingerprint());
        buffer.set_encrypted(true);
    }
    fs::write(ready, b"ready").expect("publish worker readiness");

    for step in 0..10_000 {
        let channels = buffer.channel_count() as usize;
        assert!((1..=8).contains(&channels), "invalid worker channel count");
        let frames = (step % 4) + 1;
        if role == "writer" {
            let samples = sequential_audio(frames, channels, step * 16);
            let written = if mode == "encrypted" {
                let (mut ciphertext, mut encrypted) = encrypted_staging_buffers();
                buffer.write_audio_encrypted_into_with_channel_count(
                    &samples,
                    &cipher,
                    &mut ciphertext,
                    &mut encrypted,
                    channels,
                )
            } else {
                buffer.write_audio_with_channel_count(&samples, channels)
            };
            assert!(written <= frames, "writer returned too many frames");
        } else if role == "reader" {
            let mut output = vec![0.0_f32; channels * 8];
            let read = if mode == "encrypted" {
                let (mut encrypted, mut ciphertext) = encrypted_staging_buffers();
                buffer.read_audio_encrypted_into(
                    &mut output,
                    &cipher,
                    &mut ciphertext,
                    &mut encrypted,
                )
            } else {
                buffer.read_audio(&mut output)
            };
            assert!(read <= output.len() / channels, "reader returned too many frames");
            assert!(output.iter().all(|sample| sample.is_finite()));
        } else {
            panic!("unknown worker role {role}");
        }
        thread::yield_now();
    }
}

#[test]
fn plain_reader_never_consumes_a_partial_interleaved_frame() {
    let temp_file = create_mock_shared_memory(48_000, 4, 2);
    let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("open shared memory");
    let input = [1.0, 2.0, 3.0, 4.0];
    assert_eq!(buffer.write_audio(&input), 2);

    let mut odd = [0.0; 3];
    assert_eq!(buffer.read_audio(&mut odd), 1);
    assert_eq!(odd, [1.0, 2.0, 0.0]);

    let mut next = [0.0; 2];
    assert_eq!(buffer.read_audio(&mut next), 1);
    assert_eq!(next, [3.0, 4.0]);
}

#[test]
fn test_config_negotiation_round_trip() {
    let sample_rate = 48000;
    let buffer_frames = 1024;
    let channel_count = 2;
    let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);

    let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

    let new_sample_rate = 96000;
    let new_buffer_frames = 512;
    buffer.request_config_change(new_sample_rate, new_buffer_frames, channel_count, 1);

    assert!(buffer.config_changed());
    assert_eq!(buffer.config_source(), 1);
    assert_eq!(buffer.requested_sample_rate(), new_sample_rate);
    assert_eq!(buffer.requested_buffer_frames(), new_buffer_frames);

    buffer.acknowledge_config_change(new_sample_rate, new_buffer_frames, 1, 0);

    assert!(!buffer.config_changed());
    assert_eq!(buffer.config_status(), 1);
    assert_eq!(buffer.actual_sample_rate(), new_sample_rate);
    assert_eq!(buffer.actual_buffer_frames(), new_buffer_frames);
}

#[test]
fn test_config_negotiation_error() {
    let sample_rate = 48000;
    let buffer_frames = 1024;
    let channel_count = 2;
    let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);

    let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

    buffer.request_config_change(999_999, 512, channel_count, 1);
    buffer.acknowledge_config_change(0, 0, 3, 42);

    assert_eq!(buffer.config_status(), 3);
    assert_eq!(buffer.config_error_code(), 42);
}

#[test]
fn test_frame_counter_increment() {
    let sample_rate = 48000;
    let buffer_frames = 1024;
    let channel_count = 2;
    let temp_file = create_mock_shared_memory(sample_rate, buffer_frames, channel_count);

    let buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

    assert_eq!(buffer.frame_counter(), 0);
    let new_counter = buffer.increment_frame_counter();
    assert_eq!(new_counter, 1);
    assert_eq!(buffer.frame_counter(), 1);

    for expected in 2..=100 {
        let counter = buffer.increment_frame_counter();
        assert_eq!(counter, expected);
    }
}

fn test_audio_cipher() -> crate::encryption::AudioCipher {
    let key = [0x42u8; 32];
    crate::encryption::AudioCipher::new(&key)
}

fn sequential_audio(frame_count: usize, channel_count: usize, offset: usize) -> Vec<f32> {
    (0..frame_count * channel_count)
        .map(|sample| ((offset + sample) as f32 * 0.0001) - 0.5)
        .collect()
}

fn encrypted_staging_buffers() -> (Vec<u8>, Vec<f32>) {
    let max_samples = super::super::consts::pre_alloc_capacity_samples();
    let ciphertext_capacity = super::super::encrypted::encrypted_record_total_bytes(max_samples)
        .expect("maximum encrypted record size should fit");
    let encrypted_capacity = super::super::encrypted::encrypted_record_slots(max_samples)
        .expect("maximum encrypted record slots should fit");
    (
        Vec::with_capacity(ciphertext_capacity),
        Vec::with_capacity(encrypted_capacity),
    )
}

#[test]
fn test_encrypted_available_read_frames_reports_plaintext_frames() {
    let temp_file = create_mock_shared_memory(48_000, 512, 2);
    let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");
    let cipher = test_audio_cipher();
    buffer.set_key_fingerprint(*cipher.fingerprint());
    buffer.set_encrypted(true);

    let (mut ciphertext_buf, mut encrypted_buf) = encrypted_staging_buffers();
    let samples = sequential_audio(192, 2, 0);

    assert_eq!(
        buffer.write_audio_encrypted_into(
            &samples,
            &cipher,
            &mut ciphertext_buf,
            &mut encrypted_buf,
        ),
        192
    );

    assert_eq!(buffer.available_read_frames(), 192);
}

#[test]
fn test_flush_audio_drops_pending_ring_data() {
    let temp_file = create_mock_shared_memory(48_000, 512, 2);
    let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");
    let samples = sequential_audio(128, 2, 0);

    assert_eq!(buffer.write_audio(&samples), 128);
    assert_eq!(buffer.available_read_frames(), 128);

    buffer.flush_audio();

    assert_eq!(buffer.available_read_frames(), 0);
    assert_eq!(
        buffer.header().write_position.load(Ordering::Acquire),
        buffer.header().read_position.load(Ordering::Acquire)
    );
}

#[test]
fn test_read_audio_handles_inverted_ring_positions() {
    // After the repair refactor the reader does NOT rewrite
    // `read_position` from a shared reference on plain reads — the
    // repaired position is consumed locally and only the post-read
    // position is committed. This test verifies inverted positions
    // still return 0 frames and don't leak stale data.
    let temp_file = create_mock_shared_memory(48_000, 512, 2);
    let buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

    buffer.header().write_position.store(64, Ordering::Release);
    buffer.header().read_position.store(96, Ordering::Release);

    let mut output = vec![1.0; 32];
    assert_eq!(buffer.read_audio(&mut output), 0);
    assert!(output.iter().all(|sample| *sample == 0.0));
}

#[test]
fn test_available_read_frames_clamps_overfull_ring() {
    let temp_file = create_mock_shared_memory(48_000, 512, 2);
    let buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");
    let capacity = buffer.current_audio_capacity() as u64;

    buffer
        .header()
        .write_position
        .store(capacity + 128, Ordering::Release);
    buffer.header().read_position.store(0, Ordering::Release);

    assert_eq!(
        buffer.available_read_frames(),
        buffer.current_audio_capacity() / 2
    );
}

#[test]
fn test_encrypted_hal_sized_records_read_back_in_plaintext_order() {
    let temp_file = create_mock_shared_memory(48_000, 512, 2);
    let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");
    let cipher = test_audio_cipher();
    buffer.set_key_fingerprint(*cipher.fingerprint());
    buffer.set_encrypted(true);

    let (mut ciphertext_buf, mut encrypted_buf) = encrypted_staging_buffers();
    let mut expected = Vec::new();

    for chunk in 0..5 {
        let samples = sequential_audio(192, 2, chunk * 192 * 2);
        expected.extend_from_slice(&samples);
        assert_eq!(
            buffer.write_audio_encrypted_into(
                &samples,
                &cipher,
                &mut ciphertext_buf,
                &mut encrypted_buf,
            ),
            192
        );
    }

    let mut output = vec![0.0; expected.len()];
    let frames_read = buffer.read_audio_encrypted_into(
        &mut output,
        &cipher,
        &mut encrypted_buf,
        &mut ciphertext_buf,
    );

    assert_eq!(frames_read, 960);
    for (index, (actual, expected)) in output.iter().zip(expected.iter()).enumerate() {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "sample {index} mismatch"
        );
    }
}

#[test]
fn test_hal_input_reader_stages_partial_encrypted_record() {
    let temp_file = create_mock_shared_memory(48_000, 512, 2);
    let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");
    let writer_cipher = test_audio_cipher();
    let reader_cipher = test_audio_cipher();
    buffer.set_key_fingerprint(*writer_cipher.fingerprint());
    buffer.set_encrypted(true);

    let (mut ciphertext_buf, mut encrypted_buf) = encrypted_staging_buffers();
    let mut expected = Vec::new();

    for chunk in 0..6 {
        let samples = sequential_audio(192, 2, chunk * 192 * 2);
        expected.extend_from_slice(&samples);
        assert_eq!(
            buffer.write_audio_encrypted_into(
                &samples,
                &writer_cipher,
                &mut ciphertext_buf,
                &mut encrypted_buf,
            ),
            192
        );
    }

    let (ciphertext_buf, encrypted_samples_buf) = encrypted_staging_buffers();
    let sample_capacity = super::super::consts::pre_alloc_capacity_samples();
    let mut reader = HalInputReader {
        buffer: Some(buffer),
        cipher: Some(reader_cipher),
        encrypted_samples_buf,
        ciphertext_buf,
        decrypted_record_buf: Vec::with_capacity(sample_capacity),
        pending_decrypted_samples: Vec::with_capacity(sample_capacity),
        pending_sample_offset: 0,
        key_mismatch_count: std::sync::atomic::AtomicU64::new(0),
    };

    let mut output = vec![0.0; 1024 * 2];
    assert_eq!(reader.available_read_frames(), 1152);
    assert_eq!(reader.read(&mut output), 1024);
    for (index, (actual, expected)) in output.iter().zip(expected.iter()).enumerate() {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "sample {index} mismatch"
        );
    }

    let mut tail = vec![0.0; 128 * 2];
    assert_eq!(reader.available_read_frames(), 128);
    assert_eq!(reader.read(&mut tail), 128);
    for (index, (actual, expected)) in tail.iter().zip(expected[output.len()..].iter()).enumerate()
    {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "tail sample {index} mismatch"
        );
    }
}

#[test]
fn encrypted_reader_never_consumes_a_partial_interleaved_frame() {
    let temp_file = create_mock_shared_memory(48_000, 512, 2);
    let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("open shared memory");
    let cipher = test_audio_cipher();
    buffer.set_key_fingerprint(*cipher.fingerprint());
    buffer.set_encrypted(true);

    assert_eq!(buffer.write_audio_encrypted(&[1.0, 2.0], &cipher), 1);
    assert_eq!(buffer.write_audio_encrypted(&[3.0, 4.0], &cipher), 1);

    let mut odd = [0.0; 3];
    assert_eq!(buffer.read_audio_encrypted(&mut odd, &cipher), 1);
    assert_eq!(odd, [1.0, 2.0, 0.0]);

    let mut next = [0.0; 2];
    assert_eq!(buffer.read_audio_encrypted(&mut next, &cipher), 1);
    assert_eq!(next, [3.0, 4.0]);
}

#[test]
fn encrypted_writer_rejects_zero_channel_geometry_without_panicking() {
    let temp_file = create_mock_shared_memory(48_000, 512, 2);
    let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("open shared memory");
    let cipher = test_audio_cipher();
    buffer.set_key_fingerprint(*cipher.fingerprint());
    buffer.set_encrypted(true);

    // Simulate a corrupted cross-process geometry word after the mapping was
    // opened. The writer must reject it before doing sample/channel math.
    buffer
        .header()
        .channel_count
        .store(0, std::sync::atomic::Ordering::Release);

    let mut ciphertext_buf = Vec::new();
    let mut encrypted_buf = Vec::new();
    assert_eq!(
        buffer.write_audio_encrypted_into(
            &[1.0, 2.0],
            &cipher,
            &mut ciphertext_buf,
            &mut encrypted_buf,
        ),
        0
    );
}

#[test]
fn test_hal_input_reader_reports_cipher_reload_need() {
    let temp_file = create_mock_shared_memory(48_000, 512, 2);
    let buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");
    let writer_cipher = test_audio_cipher();
    let stale_cipher = crate::encryption::AudioCipher::new(&[0x43u8; 32]);

    buffer.set_key_fingerprint(*writer_cipher.fingerprint());
    buffer.set_encrypted(false);

    let mut reader = HalInputReader {
        buffer: Some(buffer),
        cipher: Some(stale_cipher),
        encrypted_samples_buf: Vec::new(),
        ciphertext_buf: Vec::new(),
        decrypted_record_buf: Vec::new(),
        pending_decrypted_samples: Vec::new(),
        pending_sample_offset: 0,
        key_mismatch_count: std::sync::atomic::AtomicU64::new(0),
    };

    assert!(
        !reader.needs_cipher_reload(),
        "unencrypted shared memory should not require a cipher reload"
    );

    reader.buffer.as_ref().unwrap().set_encrypted(true);
    assert!(
        reader.needs_cipher_reload(),
        "encrypted shared memory should report stale cached cipher"
    );

    reader.cipher = Some(writer_cipher);
    assert!(
        !reader.needs_cipher_reload(),
        "matching cached cipher should be considered current"
    );

    reader.cipher = None;
    assert!(
        reader.needs_cipher_reload(),
        "encrypted shared memory without a cached cipher should reload"
    );
}

#[test]
fn test_corrupt_encrypted_record_is_dropped() {
    let temp_file = create_mock_shared_memory(48_000, 512, 2);
    let mut buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");
    let cipher = test_audio_cipher();
    buffer.set_key_fingerprint(*cipher.fingerprint());
    buffer.set_encrypted(true);

    let (mut ciphertext_buf, mut encrypted_buf) = encrypted_staging_buffers();
    let corrupt = sequential_audio(192, 2, 0);
    let good = sequential_audio(192, 2, corrupt.len());

    assert_eq!(
        buffer.write_audio_encrypted_into(
            &corrupt,
            &cipher,
            &mut ciphertext_buf,
            &mut encrypted_buf,
        ),
        192
    );
    assert_eq!(
        buffer.write_audio_encrypted_into(&good, &cipher, &mut ciphertext_buf, &mut encrypted_buf,),
        192
    );

    // SAFETY (test-only): flip one bit inside the first encrypted
    // record's ciphertext slot to simulate tampering.
    unsafe {
        let tampered_slot = buffer.audio_data_mut().add(6);
        *tampered_slot = f32::from_bits((*tampered_slot).to_bits() ^ 0x0000_0001);
    }

    let mut output = vec![0.0; corrupt.len()];
    assert_eq!(
        buffer.read_audio_encrypted_into(
            &mut output,
            &cipher,
            &mut encrypted_buf,
            &mut ciphertext_buf,
        ),
        0
    );

    let frames_read = buffer.read_audio_encrypted_into(
        &mut output,
        &cipher,
        &mut encrypted_buf,
        &mut ciphertext_buf,
    );

    assert_eq!(frames_read, 192);
    for (index, (actual, expected)) in output.iter().zip(good.iter()).enumerate() {
        assert_eq!(
            actual.to_bits(),
            expected.to_bits(),
            "sample {index} mismatch"
        );
    }
}

#[test]
fn test_engine_ready_flag_clears_heartbeat_after_ready() {
    let temp_file = create_mock_shared_memory(48_000, 1024, 2);
    let buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

    let engine_ready = buffer.header().engine_ready.load(Ordering::Acquire);
    assert_eq!(engine_ready, 0);

    buffer.set_engine_ready(true);
    assert_eq!(buffer.header().engine_ready.load(Ordering::Acquire), 1);
    let first_heartbeat = buffer.header().daemon_heartbeat_ms.load(Ordering::Acquire);
    assert!(first_heartbeat > 0);

    buffer.refresh_daemon_heartbeat();
    let refreshed = buffer.header().daemon_heartbeat_ms.load(Ordering::Acquire);
    assert!(refreshed >= first_heartbeat);

    buffer.set_engine_ready(false);
    assert_eq!(buffer.header().engine_ready.load(Ordering::Acquire), 0);
    // After engine_ready=0, refresh_daemon_heartbeat must not revive
    // the heartbeat.
    buffer.refresh_daemon_heartbeat();
    assert_eq!(
        buffer.header().daemon_heartbeat_ms.load(Ordering::Acquire),
        0
    );
}

#[test]
fn test_encryption_flag() {
    let temp_file = create_mock_shared_memory(48000, 1024, 2);
    let buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");

    assert!(!buffer.is_encrypted());

    let fingerprint = [1, 2, 3, 4, 5, 6, 7, 8];
    buffer.set_encrypted(true);
    buffer.set_key_fingerprint(fingerprint);

    assert!(buffer.is_encrypted());
    assert_eq!(buffer.key_fingerprint(), fingerprint);

    buffer.set_encrypted(false);
    assert!(!buffer.is_encrypted());
}

#[test]
fn test_active_flag() {
    let temp_file = create_mock_shared_memory(48000, 1024, 2);
    let buffer = SharedAudioBuffer::open(temp_file.path()).expect("Failed to open buffer");
    assert!(buffer.is_active());
}

#[test]
fn test_multichannel_configurations() {
    let configurations = vec![
        (2, "Stereo"),
        (6, "5.1 Surround"),
        (8, "7.1 Surround"),
        (16, "9.1.6"),
        (32, "Maximum HAL supported"),
    ];

    for (channel_count, name) in configurations {
        let temp_file = create_mock_shared_memory(48000, 256, channel_count);
        let buffer = SharedAudioBuffer::open(temp_file.path())
            .unwrap_or_else(|_| panic!("Failed to open {} buffer", name));

        assert_eq!(buffer.channel_count(), channel_count, "{}", name);

        let samples = vec![0.5f32; 256 * channel_count as usize];
        let mut output = vec![0.0f32; 256 * channel_count as usize];

        let mut buffer = buffer;
        buffer.write_audio(&samples);
        buffer.read_audio(&mut output);

        for (i, (input, output)) in samples.iter().zip(output.iter()).enumerate() {
            assert_eq!(
                input.to_bits(),
                output.to_bits(),
                "{}: Sample {} mismatch",
                name,
                i
            );
        }
    }
}
