// ============================================================================
// Playback Thread - iOS CoreAudio RemoteIO
// ============================================================================
//
// Uses CoreAudio's RemoteIO AudioUnit for audio output on iOS.
// The render callback reads from an rtrb ring buffer (lock-free, real-time safe).
// A feeder thread reads ProcessingMessage frames and writes to the ring buffer.
//
// Architecture:
//   ProcessingThread → mpsc → FeederThread → rtrb → CoreAudio callback → hardware

use super::{PlaybackCommand, ProcessingMessage, ThreadEvent};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender, SyncSender};

const SPIN_MS_RINGBUFFER: u64 = 5;

// ============================================================================
// CoreAudio FFI bindings (minimal subset for RemoteIO output)
// ============================================================================

#[allow(non_camel_case_types, non_upper_case_globals, dead_code)]
mod core_audio_ffi {
    use std::os::raw::c_void;

    pub type OSStatus = i32;
    pub type AudioComponentInstance = *mut c_void;
    pub type AudioComponent = *mut c_void;
    pub type Float64 = f64;
    pub type UInt32 = u32;
    pub type UInt64 = u64;
    pub type SInt32 = i32;

    pub const kAudioUnitType_Output: u32 = u32::from_be_bytes(*b"auou");
    pub const kAudioUnitSubType_RemoteIO: u32 = u32::from_be_bytes(*b"rioc");
    pub const kAudioUnitManufacturer_Apple: u32 = u32::from_be_bytes(*b"appl");

    pub const kAudioUnitScope_Input: u32 = 1;
    pub const kAudioUnitScope_Output: u32 = 2;
    pub const kAudioUnitScope_Global: u32 = 0;

    pub const kAudioUnitProperty_StreamFormat: u32 = 8;
    pub const kAudioUnitProperty_SetRenderCallback: u32 = 23;
    pub const kAudioUnitProperty_MaximumFramesPerSlice: u32 = 14;
    pub const kAudioOutputUnitProperty_EnableIO: u32 = 2003;

    pub const kAudioFormatLinearPCM: u32 = u32::from_be_bytes(*b"lpcm");
    pub const kAudioFormatFlagIsFloat: u32 = 1 << 0;
    pub const kAudioFormatFlagIsPacked: u32 = 1 << 3;
    pub const kAudioFormatFlagIsNonInterleaved: u32 = 1 << 5;

    pub const noErr: OSStatus = 0;

    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct AudioComponentDescription {
        pub component_type: u32,
        pub component_sub_type: u32,
        pub component_manufacturer: u32,
        pub component_flags: u32,
        pub component_flags_mask: u32,
    }

    #[repr(C)]
    #[derive(Clone, Copy, Debug)]
    pub struct AudioStreamBasicDescription {
        pub sample_rate: Float64,
        pub format_id: u32,
        pub format_flags: u32,
        pub bytes_per_packet: u32,
        pub frames_per_packet: u32,
        pub bytes_per_frame: u32,
        pub channels_per_frame: u32,
        pub bits_per_channel: u32,
        pub reserved: u32,
    }

    #[repr(C)]
    pub struct AudioBuffer {
        pub number_channels: u32,
        pub data_byte_size: u32,
        pub data: *mut c_void,
    }

    #[repr(C)]
    pub struct AudioBufferList {
        pub number_buffers: u32,
        // Followed by AudioBuffer[number_buffers] — we use a single buffer for interleaved
        pub buffers: [AudioBuffer; 1],
    }

    pub type AURenderCallback = Option<
        unsafe extern "C" fn(
            in_ref_con: *mut c_void,
            io_action_flags: *mut u32,
            in_time_stamp: *const AudioTimeStamp,
            in_bus_number: u32,
            in_number_frames: u32,
            io_data: *mut AudioBufferList,
        ) -> OSStatus,
    >;

    #[repr(C)]
    pub struct AURenderCallbackStruct {
        pub input_proc: AURenderCallback,
        pub input_proc_ref_con: *mut c_void,
    }

    #[repr(C)]
    pub struct AudioTimeStamp {
        pub sample_time: Float64,
        pub host_time: UInt64,
        pub rate_scalar: Float64,
        pub word_clock_time: UInt64,
        pub smpte_time: SMPTETime,
        pub flags: u32,
        pub reserved: u32,
    }

    #[repr(C)]
    pub struct SMPTETime {
        pub subframes: SInt32,
        pub subframe_divisor: SInt32,
        pub counter: u32,
        pub smpte_type: u32,
        pub flags: u32,
        pub hours: SInt32,
        pub minutes: SInt32,
        pub seconds: SInt32,
        pub frames: SInt32,
    }

    unsafe extern "C" {
        pub fn AudioComponentFindNext(
            component: AudioComponent,
            desc: *const AudioComponentDescription,
        ) -> AudioComponent;

        pub fn AudioComponentInstanceNew(
            component: AudioComponent,
            out_instance: *mut AudioComponentInstance,
        ) -> OSStatus;

        pub fn AudioComponentInstanceDispose(instance: AudioComponentInstance) -> OSStatus;

        pub fn AudioUnitSetProperty(
            unit: AudioComponentInstance,
            property_id: u32,
            scope: u32,
            element: u32,
            data: *const c_void,
            data_size: u32,
        ) -> OSStatus;

        pub fn AudioUnitInitialize(unit: AudioComponentInstance) -> OSStatus;

        pub fn AudioUnitUninitialize(unit: AudioComponentInstance) -> OSStatus;

        pub fn AudioOutputUnitStart(unit: AudioComponentInstance) -> OSStatus;

        pub fn AudioOutputUnitStop(unit: AudioComponentInstance) -> OSStatus;
    }
}

use core_audio_ffi as ca;

// ============================================================================
// Shared playback state (lock-free atomics, same pattern as desktop)
// ============================================================================

struct PlaybackState {
    capacity: usize,
    volume: Arc<AtomicU32>,
    muted: Arc<AtomicBool>,
    flush_requested: Arc<AtomicBool>,
}

impl PlaybackState {
    fn new(capacity: usize) -> Self {
        Self {
            capacity,
            volume: Arc::new(AtomicU32::new(1.0f32.to_bits())),
            muted: Arc::new(AtomicBool::new(false)),
            flush_requested: Arc::new(AtomicBool::new(false)),
        }
    }
}

// ============================================================================
// CoreAudio render callback context
// ============================================================================

struct RenderContext {
    consumer: Consumer<f32>,
    state: Arc<PlaybackState>,
    scratch: Vec<f32>,
}

/// CoreAudio render callback — called on the real-time audio thread.
/// Reads from ring buffer, applies volume/clamp, writes to AudioBufferList.
unsafe extern "C" fn render_callback(
    in_ref_con: *mut std::os::raw::c_void,
    _io_action_flags: *mut u32,
    _in_time_stamp: *const ca::AudioTimeStamp,
    _in_bus_number: u32,
    in_number_frames: u32,
    io_data: *mut ca::AudioBufferList,
) -> ca::OSStatus {
    let ctx = &mut *(in_ref_con as *mut RenderContext);
    let buf_list = &mut *io_data;
    let buf = &mut buf_list.buffers[0];

    let output_samples = buf.data_byte_size as usize / std::mem::size_of::<f32>();
    let out = std::slice::from_raw_parts_mut(buf.data as *mut f32, output_samples);

    // Scratch buffer is pre-allocated for max_frames * channels.
    // Process up to scratch capacity; zero-fill any excess the hardware requests.
    let processable = output_samples.min(ctx.scratch.len());

    // Handle flush: discard ring buffer contents, output silence
    if ctx.state.flush_requested.load(Ordering::Relaxed) {
        let available = ctx.consumer.slots().min(processable);
        if available > 0 {
            if let Ok(chunk) = ctx.consumer.read_chunk(available) {
                chunk.commit_all();
            }
        }
        out.fill(0.0);
        if ctx.consumer.slots() == 0 {
            ctx.state.flush_requested.store(false, Ordering::Relaxed);
        }
        return ca::noErr;
    }

    // Read from ring buffer
    let available = ctx.consumer.slots();
    let to_read = processable.min(available);

    if to_read > 0 {
        if let Ok(chunk) = ctx.consumer.read_chunk(to_read) {
            let (first, second) = chunk.as_slices();
            ctx.scratch[..first.len()].copy_from_slice(first);
            if !second.is_empty() {
                ctx.scratch[first.len()..first.len() + second.len()].copy_from_slice(second);
            }
            chunk.commit_all();
        }
    }

    // Zero-pad if underrun
    if to_read < processable {
        ctx.scratch[to_read..processable].fill(0.0);
    }

    // Apply volume and clamp
    let volume = f32::from_bits(ctx.state.volume.load(Ordering::Relaxed));
    let muted = ctx.state.muted.load(Ordering::Relaxed);

    if muted {
        ctx.scratch[..processable].fill(0.0);
    } else if (volume - 1.0).abs() > 0.001 {
        for s in ctx.scratch[..processable].iter_mut() {
            *s = (*s * volume).clamp(-1.0, 1.0);
        }
    } else {
        for s in ctx.scratch[..processable].iter_mut() {
            *s = s.clamp(-1.0, 1.0);
        }
    }

    // Copy processed samples to output buffer
    out[..processable].copy_from_slice(&ctx.scratch[..processable]);
    // Zero-fill any excess (if output buffer > scratch capacity)
    if processable < output_samples {
        out[processable..].fill(0.0);
    }

    ca::noErr
}

// ============================================================================
// AudioUnit wrapper
// ============================================================================

struct AudioUnitHandle {
    instance: ca::AudioComponentInstance,
    // RenderContext is heap-allocated and lives as long as the AudioUnit.
    // The raw pointer is passed to the render callback.
    _render_ctx: Box<RenderContext>,
}

impl AudioUnitHandle {
    fn new(
        sample_rate: u32,
        channels: usize,
        consumer: Consumer<f32>,
        state: Arc<PlaybackState>,
    ) -> Result<Self, String> {
        let desc = ca::AudioComponentDescription {
            component_type: ca::kAudioUnitType_Output,
            component_sub_type: ca::kAudioUnitSubType_RemoteIO,
            component_manufacturer: ca::kAudioUnitManufacturer_Apple,
            component_flags: 0,
            component_flags_mask: 0,
        };

        let component = unsafe { ca::AudioComponentFindNext(std::ptr::null_mut(), &desc) };
        if component.is_null() {
            return Err("RemoteIO AudioComponent not found".to_string());
        }

        let mut instance: ca::AudioComponentInstance = std::ptr::null_mut();
        let status = unsafe { ca::AudioComponentInstanceNew(component, &mut instance) };
        if status != ca::noErr {
            return Err(format!("AudioComponentInstanceNew failed: {}", status));
        }

        // Enable output on bus 0
        let enable_output: u32 = 1;
        let status = unsafe {
            ca::AudioUnitSetProperty(
                instance,
                ca::kAudioOutputUnitProperty_EnableIO,
                ca::kAudioUnitScope_Output,
                0,
                &enable_output as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            )
        };
        if status != ca::noErr {
            log::warn!("[iOS AudioUnit] EnableIO failed: {} (continuing anyway)", status);
        }

        // Set stream format: interleaved f32
        let asbd = ca::AudioStreamBasicDescription {
            sample_rate: sample_rate as f64,
            format_id: ca::kAudioFormatLinearPCM,
            format_flags: ca::kAudioFormatFlagIsFloat | ca::kAudioFormatFlagIsPacked,
            bytes_per_packet: (channels * std::mem::size_of::<f32>()) as u32,
            frames_per_packet: 1,
            bytes_per_frame: (channels * std::mem::size_of::<f32>()) as u32,
            channels_per_frame: channels as u32,
            bits_per_channel: 32,
            reserved: 0,
        };

        let status = unsafe {
            ca::AudioUnitSetProperty(
                instance,
                ca::kAudioUnitProperty_StreamFormat,
                ca::kAudioUnitScope_Input,
                0, // bus 0 = output
                &asbd as *const ca::AudioStreamBasicDescription as *const _,
                std::mem::size_of::<ca::AudioStreamBasicDescription>() as u32,
            )
        };
        if status != ca::noErr {
            unsafe { ca::AudioComponentInstanceDispose(instance) };
            return Err(format!("Set stream format failed: {}", status));
        }

        // Set max frames per slice
        let max_frames: u32 = 4096;
        let status = unsafe {
            ca::AudioUnitSetProperty(
                instance,
                ca::kAudioUnitProperty_MaximumFramesPerSlice,
                ca::kAudioUnitScope_Global,
                0,
                &max_frames as *const u32 as *const _,
                std::mem::size_of::<u32>() as u32,
            )
        };
        if status != ca::noErr {
            log::warn!("[iOS AudioUnit] MaxFramesPerSlice failed: {} (continuing)", status);
        }

        // Create render context (heap-allocated, stable address)
        let scratch_size = 4096 * channels;
        let render_ctx = Box::new(RenderContext {
            consumer,
            state,
            scratch: vec![0.0; scratch_size],
        });

        // Set render callback
        let callback_struct = ca::AURenderCallbackStruct {
            input_proc: Some(render_callback),
            input_proc_ref_con: &*render_ctx as *const RenderContext as *mut _,
        };

        let status = unsafe {
            ca::AudioUnitSetProperty(
                instance,
                ca::kAudioUnitProperty_SetRenderCallback,
                ca::kAudioUnitScope_Input,
                0,
                &callback_struct as *const ca::AURenderCallbackStruct as *const _,
                std::mem::size_of::<ca::AURenderCallbackStruct>() as u32,
            )
        };
        if status != ca::noErr {
            unsafe { ca::AudioComponentInstanceDispose(instance) };
            return Err(format!("Set render callback failed: {}", status));
        }

        // Initialize
        let status = unsafe { ca::AudioUnitInitialize(instance) };
        if status != ca::noErr {
            unsafe { ca::AudioComponentInstanceDispose(instance) };
            return Err(format!("AudioUnitInitialize failed: {}", status));
        }

        // Start
        let status = unsafe { ca::AudioOutputUnitStart(instance) };
        if status != ca::noErr {
            unsafe {
                ca::AudioUnitUninitialize(instance);
                ca::AudioComponentInstanceDispose(instance);
            }
            return Err(format!("AudioOutputUnitStart failed: {}", status));
        }

        log::info!(
            "[iOS AudioUnit] Started: {}Hz, {}ch, interleaved f32",
            sample_rate,
            channels
        );

        Ok(Self {
            instance,
            _render_ctx: render_ctx,
        })
    }
}

impl Drop for AudioUnitHandle {
    fn drop(&mut self) {
        unsafe {
            ca::AudioOutputUnitStop(self.instance);
            ca::AudioUnitUninitialize(self.instance);
            ca::AudioComponentInstanceDispose(self.instance);
        }
        log::info!("[iOS AudioUnit] Stopped and disposed");
    }
}

// ============================================================================
// PlaybackThread — public API (same as desktop)
// ============================================================================

pub struct PlaybackThread {
    command_tx: Sender<PlaybackCommand>,
    thread_handle: Option<std::thread::JoinHandle<()>>,
}

impl PlaybackThread {
    pub fn new(
        message_rx: Receiver<ProcessingMessage>,
        event_tx: Sender<ThreadEvent>,
        sample_rate: u32,
        buffer_ms: u32,
        channels: usize,
        _output_device: Option<String>,
        recycle_tx: SyncSender<Vec<f32>>,
        _allow_virtual_output: bool,
    ) -> Result<Self, String> {
        let (command_tx, command_rx) = std::sync::mpsc::channel();

        let thread_handle = std::thread::Builder::new()
            .name("playback-ios".to_string())
            .spawn(move || {
                if let Err(e) = run_playback_ios(
                    message_rx,
                    command_rx,
                    event_tx.clone(),
                    sample_rate,
                    buffer_ms,
                    channels,
                    recycle_tx,
                ) {
                    log::error!("[Playback Thread iOS] Error: {}", e);
                    event_tx
                        .send(ThreadEvent::ProcessingError(format!(
                            "iOS playback error: {}",
                            e
                        )))
                        .ok();
                }
            })
            .map_err(|e| format!("Failed to spawn playback thread: {}", e))?;

        Ok(Self {
            command_tx,
            thread_handle: Some(thread_handle),
        })
    }

    pub fn send_command(&self, command: PlaybackCommand) -> Result<(), String> {
        self.command_tx
            .send(command)
            .map_err(|e| format!("Failed to send command: {}", e))
    }

    pub fn shutdown(&mut self) {
        self.send_command(PlaybackCommand::Shutdown).ok();
        if let Some(handle) = self.thread_handle.take() {
            handle.join().ok();
        }
    }
}

impl Drop for PlaybackThread {
    fn drop(&mut self) {
        self.shutdown();
    }
}

// ============================================================================
// Main playback thread function
// ============================================================================

fn playback_buffer_capacity(sample_rate: u32, channels: usize, buffer_ms: u32) -> usize {
    (((sample_rate as u64 * buffer_ms as u64) / 1000) as usize) * channels
}

fn run_playback_ios(
    message_rx: Receiver<ProcessingMessage>,
    command_rx: Receiver<PlaybackCommand>,
    event_tx: Sender<ThreadEvent>,
    sample_rate: u32,
    buffer_ms: u32,
    channels: usize,
    recycle_tx: SyncSender<Vec<f32>>,
) -> Result<(), String> {
    // Create ring buffer
    let buffer_capacity = playback_buffer_capacity(sample_rate, channels, buffer_ms);
    let (mut producer, consumer) = RingBuffer::<f32>::new(buffer_capacity);

    // Create shared state
    let state = Arc::new(PlaybackState::new(buffer_capacity));

    // Create CoreAudio AudioUnit
    let _audio_unit = AudioUnitHandle::new(sample_rate, channels, consumer, Arc::clone(&state))?;

    event_tx
        .send(ThreadEvent::PlaybackChannelsChanged(channels))
        .ok();

    log::info!(
        "[Playback Thread iOS] Started - {}Hz, {}ch, buffer={}ms ({}samples)",
        sample_rate,
        channels,
        buffer_ms,
        buffer_capacity,
    );

    // End-of-stream drain tracking
    let mut end_of_stream = false;
    let mut drain_start: Option<std::time::Instant> = None;
    let drain_timeout = std::time::Duration::from_secs(2);
    let mut flush_dropping = false;

    // Main loop: read from processing queue and write to ring buffer
    loop {
        // Check for commands
        if let Ok(command) = command_rx.try_recv() {
            match command {
                PlaybackCommand::SetVolume(vol) => {
                    state.volume.store(vol.to_bits(), Ordering::Relaxed);
                }
                PlaybackCommand::Mute(muted) => {
                    state.muted.store(muted, Ordering::Relaxed);
                }
                PlaybackCommand::UpdateSampleRate(new_rate) => {
                    if new_rate != sample_rate {
                        log::warn!(
                            "[Playback Thread iOS] Sample rate change {}→{} not supported at runtime on iOS",
                            sample_rate, new_rate
                        );
                    }
                }
                PlaybackCommand::UpdateChannels(new_ch) => {
                    if new_ch != channels {
                        log::warn!(
                            "[Playback Thread iOS] Channel count change {}→{} not supported at runtime on iOS",
                            channels, new_ch
                        );
                    }
                }
                PlaybackCommand::Stop => {
                    state.flush_requested.store(true, Ordering::Relaxed);
                    flush_dropping = true;
                    end_of_stream = false;
                    drain_start = None;
                }
                PlaybackCommand::Shutdown => {
                    log::debug!("[Playback Thread iOS] Shutting down");
                    break;
                }
            }
        }

        // Check for ring buffer space
        let available_space = producer.slots();
        let min_space = 1024 * channels * 2;
        if available_space < min_space {
            std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_RINGBUFFER));
            continue;
        }

        // Read from message queue
        match message_rx.try_recv() {
            Ok(ProcessingMessage::Frame(frame)) => {
                if flush_dropping {
                    recycle_tx.try_send(frame.data).ok();
                    continue;
                }

                // Write to ring buffer
                let frame_samples = frame.data.len();
                match producer.write_chunk_uninit(frame_samples) {
                    Ok(chunk) => {
                        chunk.fill_from_iter(frame.data.iter().copied());
                    }
                    Err(_) => {
                        // Ring buffer full — drop frame
                    }
                }
                recycle_tx.try_send(frame.data).ok();
            }
            Ok(ProcessingMessage::EndOfStream) => {
                if flush_dropping {
                    continue;
                }
                log::debug!("[Playback Thread iOS] End of stream - starting drain");
                end_of_stream = true;
                drain_start = Some(std::time::Instant::now());
            }
            Ok(ProcessingMessage::Flush) => {
                state.flush_requested.store(true, Ordering::Relaxed);
                end_of_stream = false;
                drain_start = None;
                flush_dropping = false;
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => {
                if end_of_stream {
                    // Check if ring buffer has drained
                    if producer.slots() >= buffer_capacity {
                        log::info!("[Playback Thread iOS] Ring buffer drained");
                        event_tx.send(ThreadEvent::PlaybackDrained).ok();
                        break;
                    }
                    if let Some(start) = drain_start {
                        if start.elapsed() > drain_timeout {
                            log::warn!("[Playback Thread iOS] Drain timeout, signaling completion");
                            event_tx.send(ThreadEvent::PlaybackDrained).ok();
                            break;
                        }
                    }
                    std::thread::sleep(std::time::Duration::from_millis(5));
                } else {
                    std::thread::sleep(std::time::Duration::from_millis(1));
                }
            }
            Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                if end_of_stream {
                    // Wait for drain
                    let drain_start = std::time::Instant::now();
                    while drain_start.elapsed() < drain_timeout {
                        if producer.slots() >= buffer_capacity {
                            event_tx.send(ThreadEvent::PlaybackDrained).ok();
                            break;
                        }
                        std::thread::sleep(std::time::Duration::from_millis(5));
                    }
                }
                log::debug!("[Playback Thread iOS] Queue disconnected");
                break;
            }
        }
    }

    // AudioUnit is dropped here, which stops and disposes it
    log::debug!("[Playback Thread iOS] Stopped");
    Ok(())
}
