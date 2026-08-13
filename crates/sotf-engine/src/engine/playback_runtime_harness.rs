use super::AudioFrame;
use super::playback_thread::{
    FrameWriteOutcome, PlaybackState, apply_volume_clamp, read_ring_buffer,
    required_conversion_capacity, write_chunk_bulk, write_frame_to_ring,
};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::mpsc::{Receiver, SyncSender, TryRecvError, sync_channel};

pub struct FrameWriterHarness {
    producer: Producer<f32>,
    consumer: Consumer<f32>,
    recycle_tx: SyncSender<Vec<f32>>,
    recycle_rx: Receiver<Vec<f32>>,
    conversion_buffer: Vec<f32>,
    output_channels: usize,
}

/// Deterministic harness for the consumer/callback half of the playback ring.
pub struct PlaybackCallbackHarness {
    producer: Producer<f32>,
    consumer: Consumer<f32>,
    state: PlaybackState,
    scratch: Vec<f32>,
    channels: usize,
    sample_rate: u32,
    capacity: usize,
}

impl PlaybackCallbackHarness {
    pub fn new(
        capacity: usize,
        callback_samples: usize,
        channels: usize,
        sample_rate: u32,
    ) -> Self {
        let capacity = capacity.max(callback_samples).max(1);
        let (producer, consumer) = RingBuffer::new(capacity);
        Self {
            producer,
            consumer,
            state: PlaybackState::new(capacity),
            scratch: vec![0.0; callback_samples],
            channels: channels.max(1),
            sample_rate,
            capacity,
        }
    }

    /// Feed and execute one callback without allocating after construction.
    #[inline(always)]
    pub fn process(&mut self, input: &[f32]) -> &[f32] {
        assert_eq!(input.len(), self.scratch.len());
        let chunk = self.producer.write_chunk_uninit(input.len()).unwrap();
        write_chunk_bulk(chunk, input);
        let len = self.scratch.len();
        read_ring_buffer(
            &mut self.consumer,
            &mut self.scratch,
            len,
            &self.state,
            self.capacity,
        );
        apply_volume_clamp(
            &mut self.scratch,
            &self.state,
            self.channels,
            self.sample_rate,
        );
        &self.scratch
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HarnessWriteReport {
    pub outcome: HarnessFrameWriteOutcome,
    pub slots_before: usize,
    pub slots_after: usize,
    pub recycled_buffers: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HarnessFrameWriteOutcome {
    Written { samples: usize },
    Dropped,
    ConversionBufferTooSmall,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FuzzerStats {
    pub cases: usize,
    pub sequences: usize,
    pub written: usize,
    pub dropped: usize,
    pub recycled: usize,
    pub samples_written: usize,
    pub events: usize,
}

impl FrameWriterHarness {
    pub fn new(
        ring_capacity: usize,
        output_channels: usize,
        conversion_capacity: usize,
        prefill_samples: usize,
    ) -> Self {
        let (mut producer, consumer) = RingBuffer::<f32>::new(ring_capacity.max(1));
        prefill_ring(&mut producer, prefill_samples);
        let (recycle_tx, recycle_rx) = sync_channel(1024);
        Self {
            producer,
            consumer,
            recycle_tx,
            recycle_rx,
            conversion_buffer: Vec::with_capacity(conversion_capacity),
            output_channels: output_channels.max(1),
        }
    }

    pub fn for_frame(
        ring_capacity: usize,
        output_channels: usize,
        frame: &AudioFrame,
        prefill_samples: usize,
    ) -> Self {
        Self::new(
            ring_capacity,
            output_channels,
            ring_capacity.max(required_conversion_capacity(
                frame.num_frames,
                output_channels.max(1),
            )),
            prefill_samples,
        )
    }

    #[inline(always)]
    pub fn write(&mut self, frame: AudioFrame) -> HarnessWriteReport {
        drain_recycled(&self.recycle_rx);
        let slots_before = self.producer.slots();
        let outcome = write_frame_to_ring(
            &mut self.producer,
            &self.recycle_tx,
            &mut self.conversion_buffer,
            self.output_channels,
            frame,
        );
        let outcome = match outcome {
            FrameWriteOutcome::Written { samples } => HarnessFrameWriteOutcome::Written { samples },
            FrameWriteOutcome::Dropped => HarnessFrameWriteOutcome::Dropped,
            FrameWriteOutcome::ConversionBufferTooSmall => {
                HarnessFrameWriteOutcome::ConversionBufferTooSmall
            }
        };
        let slots_after = self.producer.slots();
        let recycled_buffers = drain_recycled(&self.recycle_rx);
        HarnessWriteReport {
            outcome,
            slots_before,
            slots_after,
            recycled_buffers,
        }
    }

    pub fn rebuild(
        &mut self,
        ring_capacity: usize,
        output_channels: usize,
        prefill_samples: usize,
    ) {
        let (mut producer, consumer) = RingBuffer::<f32>::new(ring_capacity.max(1));
        prefill_ring(&mut producer, prefill_samples);
        drain_recycled(&self.recycle_rx);
        self.producer = producer;
        self.consumer = consumer;
        self.conversion_buffer = Vec::with_capacity(ring_capacity.max(1));
        self.output_channels = output_channels.max(1);
    }

    pub fn drain_samples(&mut self, samples: usize) -> usize {
        let available = self.consumer.slots().min(samples);
        if available == 0 {
            return 0;
        }
        let Ok(chunk) = self.consumer.read_chunk(available) else {
            return 0;
        };
        chunk.commit_all();
        available
    }

    pub fn recycle_without_write(&mut self, data: Vec<f32>) -> usize {
        drain_recycled(&self.recycle_rx);
        let _ = self.recycle_tx.try_send(data);
        drain_recycled(&self.recycle_rx)
    }

    pub fn free_slots(&self) -> usize {
        self.producer.slots()
    }

    pub fn ring_capacity(&self) -> usize {
        self.producer.slots() + self.consumer.slots()
    }
}

pub fn run_fuzzer(seed: u64, cases: usize) -> Result<FuzzerStats, String> {
    let mut rng = XorShift64::new(seed);
    let mut stats = FuzzerStats::default();

    while stats.cases < cases {
        let ring_capacity = rng.range(512, 65_537);
        let output_channels = rng.range(1, 13);
        let prefill_samples = if rng.one_in(20) {
            ring_capacity
        } else {
            rng.range(0, ring_capacity / 4 + 1)
        };
        let mut runtime = FuzzRuntime::new(ring_capacity, output_channels, prefill_samples);
        stats.sequences += 1;

        let sequence_len = rng.range(1, 65);
        for _ in 0..sequence_len {
            if stats.cases >= cases || runtime.terminated {
                break;
            }
            let before_stats = stats;
            let op = FuzzOp::generate(&mut rng, runtime.ring_capacity());
            runtime.apply(op, &mut rng, &mut stats)?;
            stats.cases += 1;

            if stats.cases < before_stats.cases
                || stats.written < before_stats.written
                || stats.dropped < before_stats.dropped
                || stats.recycled < before_stats.recycled
                || stats.samples_written < before_stats.samples_written
                || stats.events < before_stats.events
            {
                return Err(format!("case {}: fuzzer counters regressed", stats.cases));
            }
        }
    }

    if stats.events > stats.cases + stats.sequences {
        return Err(format!(
            "event emission exceeded operation bound: events={}, cases={}, sequences={}",
            stats.events, stats.cases, stats.sequences
        ));
    }

    Ok(stats)
}

struct FuzzRuntime {
    writer: FrameWriterHarness,
    flush_mode: HarnessFlushMode,
    end_of_stream: bool,
    terminated: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HarnessFlushMode {
    Normal,
    DroppingUntilFlush,
    WaitingForDrain,
}

enum FuzzOp {
    Frame {
        frames: usize,
        input_channels: usize,
    },
    Drain {
        samples: usize,
    },
    Flush,
    Stop,
    EndOfStream,
    Disconnect,
    UpdateChannels {
        output_channels: usize,
        ring_capacity: usize,
        prefill_samples: usize,
    },
    SetVolume,
    Mute,
}

impl FuzzRuntime {
    fn new(ring_capacity: usize, output_channels: usize, prefill_samples: usize) -> Self {
        Self {
            writer: FrameWriterHarness::new(
                ring_capacity,
                output_channels,
                ring_capacity.max(1),
                prefill_samples,
            ),
            flush_mode: HarnessFlushMode::Normal,
            end_of_stream: false,
            terminated: false,
        }
    }

    fn ring_capacity(&self) -> usize {
        self.writer.ring_capacity()
    }

    fn apply(
        &mut self,
        op: FuzzOp,
        rng: &mut XorShift64,
        stats: &mut FuzzerStats,
    ) -> Result<(), String> {
        match op {
            FuzzOp::Frame {
                frames,
                input_channels,
            } => self.apply_frame(frames, input_channels, rng, stats),
            FuzzOp::Drain { samples } => {
                self.apply_drain(samples, stats);
                Ok(())
            }
            FuzzOp::Flush => {
                self.end_of_stream = false;
                self.flush_mode = if self.writer.free_slots() >= self.writer.ring_capacity() {
                    HarnessFlushMode::Normal
                } else {
                    HarnessFlushMode::WaitingForDrain
                };
                Ok(())
            }
            FuzzOp::Stop => {
                self.flush_mode = HarnessFlushMode::DroppingUntilFlush;
                self.end_of_stream = false;
                Ok(())
            }
            FuzzOp::EndOfStream => {
                if !matches!(self.flush_mode, HarnessFlushMode::DroppingUntilFlush) {
                    self.end_of_stream = true;
                    if self.writer.free_slots() >= self.writer.ring_capacity() {
                        self.terminated = true;
                        stats.events += 1;
                    }
                }
                Ok(())
            }
            FuzzOp::Disconnect => {
                if self.end_of_stream && self.writer.free_slots() >= self.writer.ring_capacity() {
                    stats.events += 1;
                }
                self.terminated = true;
                Ok(())
            }
            FuzzOp::UpdateChannels {
                output_channels,
                ring_capacity,
                prefill_samples,
            } => {
                self.writer
                    .rebuild(ring_capacity, output_channels, prefill_samples);
                self.flush_mode = HarnessFlushMode::Normal;
                self.end_of_stream = false;
                stats.events += 1;
                Ok(())
            }
            FuzzOp::SetVolume | FuzzOp::Mute => Ok(()),
        }
    }

    fn apply_frame(
        &mut self,
        frames: usize,
        input_channels: usize,
        rng: &mut XorShift64,
        stats: &mut FuzzerStats,
    ) -> Result<(), String> {
        let frame_samples = frames
            .checked_mul(input_channels)
            .ok_or_else(|| "frame sample count overflow".to_string())?;
        let frame = generated_frame(frames, input_channels, frame_samples, rng);

        if matches!(self.flush_mode, HarnessFlushMode::DroppingUntilFlush) {
            let recycled = self.writer.recycle_without_write(frame.data);
            if recycled != 1 {
                return Err(format!(
                    "flush drop recycled {recycled} buffers instead of exactly one"
                ));
            }
            stats.dropped += 1;
            stats.recycled += recycled;
            return Ok(());
        }

        let report = self.writer.write(frame);
        if report.recycled_buffers != 1 {
            return Err(format!(
                "expected exactly one recycled buffer, got {}",
                report.recycled_buffers
            ));
        }
        if report.slots_after > report.slots_before {
            return Err(format!(
                "ring slots increased after write/drop ({} -> {})",
                report.slots_before, report.slots_after
            ));
        }

        stats.recycled += report.recycled_buffers;
        match report.outcome {
            HarnessFrameWriteOutcome::Written { samples } => {
                stats.written += 1;
                stats.samples_written += samples;
                let consumed_slots = report.slots_before - report.slots_after;
                if consumed_slots != samples {
                    return Err(format!(
                        "committed {consumed_slots} slots but reported {samples} samples"
                    ));
                }
            }
            HarnessFrameWriteOutcome::Dropped => {
                stats.dropped += 1;
                if report.slots_after != report.slots_before {
                    return Err(format!(
                        "dropped frame changed ring slots ({} -> {})",
                        report.slots_before, report.slots_after
                    ));
                }
            }
            HarnessFrameWriteOutcome::ConversionBufferTooSmall => {
                return Err("conversion buffer invariant failed".to_string());
            }
        }

        Ok(())
    }

    fn apply_drain(&mut self, samples: usize, stats: &mut FuzzerStats) {
        self.writer.drain_samples(samples);
        if matches!(self.flush_mode, HarnessFlushMode::WaitingForDrain)
            && self.writer.free_slots() >= self.writer.ring_capacity()
        {
            self.flush_mode = HarnessFlushMode::Normal;
        }
        if self.end_of_stream && self.writer.free_slots() >= self.writer.ring_capacity() {
            self.terminated = true;
            stats.events += 1;
        }
    }
}

impl FuzzOp {
    fn generate(rng: &mut XorShift64, current_ring_capacity: usize) -> Self {
        match rng.range(0, 100) {
            0..=44 => Self::Frame {
                frames: rng.range(1, 1025),
                input_channels: rng.range(1, 17),
            },
            45..=69 => Self::Drain {
                samples: rng.range(1, current_ring_capacity.max(2)),
            },
            70..=76 => Self::UpdateChannels {
                output_channels: rng.range(1, 13),
                ring_capacity: rng.range(512, 65_537),
                prefill_samples: 0,
            },
            77..=82 => Self::Flush,
            83..=87 => Self::Stop,
            88..=92 => Self::EndOfStream,
            93..=96 => Self::Disconnect,
            97..=98 => Self::SetVolume,
            _ => Self::Mute,
        }
    }
}

pub fn generated_frame(
    frames: usize,
    channels: usize,
    samples: usize,
    rng: &mut impl HarnessRng,
) -> AudioFrame {
    let mut data = Vec::with_capacity(samples);
    for _ in 0..samples {
        let raw = rng.next_u32();
        let sample = ((raw as f32 / u32::MAX as f32) * 2.0) - 1.0;
        data.push(sample);
    }
    AudioFrame {
        data,
        num_frames: frames,
        num_channels: channels,
        sample_rate: 48_000,
    }
}

pub trait HarnessRng {
    fn next_u32(&mut self) -> u32;
}

pub struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    pub fn new(seed: u64) -> Self {
        Self { state: seed.max(1) }
    }

    fn one_in(&mut self, n: usize) -> bool {
        self.range(0, n) == 0
    }

    fn range(&mut self, start: usize, end: usize) -> usize {
        debug_assert!(start < end);
        let span = end - start;
        start + (self.next_u32() as usize % span)
    }
}

impl HarnessRng for XorShift64 {
    fn next_u32(&mut self) -> u32 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        (x >> 32) as u32
    }
}

fn prefill_ring(producer: &mut Producer<f32>, samples: usize) {
    let to_write = samples.min(producer.slots());
    if to_write == 0 {
        return;
    }
    let Ok(mut chunk) = producer.write_chunk_uninit(to_write) else {
        return;
    };
    let (first, second) = chunk.as_mut_slices();
    for slot in first {
        slot.write(0.0);
    }
    for slot in second {
        slot.write(0.0);
    }
    // Safety: exactly `to_write` samples were initialized above.
    unsafe { chunk.commit(to_write) };
}

fn drain_recycled(recycle_rx: &Receiver<Vec<f32>>) -> usize {
    let mut count = 0;
    loop {
        match recycle_rx.try_recv() {
            Ok(_) => count += 1,
            Err(TryRecvError::Empty) => return count,
            Err(TryRecvError::Disconnected) => return count,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::engine::{
        DecoderCommand, DecoderThread, DsdOutputMode, GcThread, ProcessingMessage,
        ProcessingThread, ThreadEvent,
    };
    use arc_swap::ArcSwap;
    use sotf_testkit::audio;
    use std::sync::{Arc, mpsc::sync_channel};
    use std::time::Duration;

    /// Mock output sink: receives processed frames and signals end-of-stream.
    fn run_mock_sink(
        rx: std::sync::mpsc::Receiver<ProcessingMessage>,
        done_tx: std::sync::mpsc::Sender<MockSinkReport>,
    ) {
        let mut frames = 0usize;
        let mut got_eos = false;

        while let Ok(msg) = rx.recv() {
            match msg {
                ProcessingMessage::Frame(_) => {
                    frames += 1;
                }
                ProcessingMessage::EndOfStream => {
                    got_eos = true;
                    break;
                }
                ProcessingMessage::Flush => {}
            }
        }

        let _ = done_tx.send(MockSinkReport { frames, got_eos });
    }

    #[derive(Debug)]
    struct MockSinkReport {
        frames: usize,
        got_eos: bool,
    }

    #[cfg(feature = "streaming")]
    #[allow(clippy::too_many_arguments)]
    fn spawn_processing_thread(
        decoder_rx: std::sync::mpsc::Receiver<crate::engine::DecoderMessage>,
        message_tx: std::sync::mpsc::SyncSender<ProcessingMessage>,
        event_tx: crossbeam::channel::Sender<ThreadEvent>,
        sample_rate: u32,
        channels: usize,
        plugin_data_cache: crate::engine::PluginDataCache,
        gc_tx: crate::engine::GcSender,
        recycle_rx: std::sync::mpsc::Receiver<Vec<f32>>,
        decoder_recycle_tx: std::sync::mpsc::SyncSender<Vec<f32>>,
    ) -> ProcessingThread {
        ProcessingThread::new(
            decoder_rx,
            message_tx,
            event_tx,
            sample_rate,
            channels,
            plugin_data_cache,
            gc_tx,
            recycle_rx,
            decoder_recycle_tx,
            None,
        )
        .expect("processing thread should spawn")
    }

    #[cfg(not(feature = "streaming"))]
    fn spawn_processing_thread(
        decoder_rx: std::sync::mpsc::Receiver<crate::engine::DecoderMessage>,
        message_tx: std::sync::mpsc::SyncSender<ProcessingMessage>,
        event_tx: crossbeam::channel::Sender<ThreadEvent>,
        sample_rate: u32,
        channels: usize,
        plugin_data_cache: crate::engine::PluginDataCache,
        gc_tx: crate::engine::GcSender,
        recycle_rx: std::sync::mpsc::Receiver<Vec<f32>>,
        decoder_recycle_tx: std::sync::mpsc::SyncSender<Vec<f32>>,
    ) -> ProcessingThread {
        ProcessingThread::new(
            decoder_rx,
            message_tx,
            event_tx,
            sample_rate,
            channels,
            plugin_data_cache,
            gc_tx,
            recycle_rx,
            decoder_recycle_tx,
        )
        .expect("processing thread should spawn")
    }

    /// End-to-end pipeline test: decode → process → mock sink, with no real
    /// audio hardware.
    #[test]
    fn decode_process_mock_sink_no_hardware() {
        let sample_rate = 48_000;
        let channels = 2;
        let frame_size = 512;

        // Short synthetic stereo WAV file.
        let (temp_wav, _mono) = audio::temp_sine_wav(0.2, sample_rate, channels as u16, 440.0)
            .expect("should create temp sine WAV");

        // Decoder → processing channel.
        let (decoder_tx, decoder_rx) = sync_channel::<crate::engine::DecoderMessage>(64);
        // Processing → mock sink channel.
        let (sink_tx, sink_rx) = sync_channel::<ProcessingMessage>(64);
        // Shared event bus.
        let (event_tx, _event_rx) = std::sync::mpsc::channel::<ThreadEvent>();
        // Recycle channel: processing thread sends buffers back to decoder.
        let (decoder_recycle_tx, decoder_recycle_rx) = sync_channel::<Vec<f32>>(64);
        // Recycle channel: mock sink → processing thread (left empty; allocations
        // fall back to the recycle-miss path).
        let (_playback_recycle_tx, playback_recycle_rx) = sync_channel::<Vec<f32>>(64);

        let decoder = DecoderThread::new(
            decoder_tx,
            event_tx.clone(),
            sample_rate,
            frame_size,
            decoder_recycle_rx,
            DsdOutputMode::Disabled,
        )
        .expect("decoder thread should spawn");

        let gc = GcThread::new().expect("gc thread should spawn");

        let plugin_data_cache: crate::engine::PluginDataCache =
            Arc::new(ArcSwap::from_pointee(Vec::new()));

        let processing = spawn_processing_thread(
            decoder_rx,
            sink_tx,
            event_tx,
            sample_rate,
            channels,
            plugin_data_cache,
            gc.sender(),
            playback_recycle_rx,
            decoder_recycle_tx,
        );

        let (done_tx, done_rx) = std::sync::mpsc::channel();
        let sink_handle = std::thread::spawn(move || run_mock_sink(sink_rx, done_tx));

        decoder
            .send_command(DecoderCommand::Play(temp_wav.path().to_path_buf().into()))
            .expect("Play command should send");

        let report = done_rx
            .recv_timeout(Duration::from_secs(10))
            .expect("mock sink should report within timeout");

        // Dropping the threads sends shutdown commands and joins.
        drop(processing);
        drop(decoder);
        drop(gc);
        let _ = sink_handle.join();

        assert!(
            report.frames > 0,
            "mock sink should receive at least one frame, got {report:?}"
        );
        assert!(
            report.got_eos,
            "mock sink should receive end-of-stream, got {report:?}"
        );
    }
}
