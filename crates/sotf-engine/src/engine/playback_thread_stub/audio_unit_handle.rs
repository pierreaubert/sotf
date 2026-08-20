use super::super::{PlaybackCommand, ProcessingMessage, ThreadEvent};
use super::misc::SPIN_MS_RINGBUFFER;
use super::misc::core_audio_ffi as ca;
use super::misc::playback_buffer_capacity;
use super::misc::write_chunk_bulk;
use super::playback_state::PlaybackState;
use super::types::RenderContext;
use super::types::render_callback;
use rtrb::{Consumer, RingBuffer};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{Receiver, SyncSender};

pub(super) struct AudioUnitHandle {
    pub(super) instance: ca::AudioComponentInstance,
    // RenderContext is heap-allocated and lives as long as the AudioUnit.
    // The raw pointer is passed to the render callback.
    pub(super) _render_ctx: Box<RenderContext>,
}

impl AudioUnitHandle {
    pub(super) fn new(
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
            log::warn!(
                "[iOS AudioUnit] EnableIO failed: {} (continuing anyway)",
                status
            );
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
            log::warn!(
                "[iOS AudioUnit] MaxFramesPerSlice failed: {status}; render callback supports the device-provided slice directly"
            );
        }

        // Create render context (heap-allocated, stable address)
        let render_ctx = Box::new(RenderContext {
            consumer,
            state,
            sample_rate,
            channels,
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

pub(super) fn run_playback_ios(
    message_rx: Receiver<ProcessingMessage>,
    command_rx: Receiver<PlaybackCommand>,
    event_tx: crossbeam::channel::Sender<ThreadEvent>,
    sample_rate: u32,
    buffer_ms: u32,
    channels: usize,
    frame_size: usize,
    recycle_tx: SyncSender<Vec<f32>>,
) -> Result<(), String> {
    // Create ring buffer
    let buffer_capacity = playback_buffer_capacity(sample_rate, channels, buffer_ms)
        .max(frame_size.saturating_mul(channels));
    let (mut producer, consumer) = RingBuffer::<f32>::new(buffer_capacity);

    // Create shared state
    let state = Arc::new(PlaybackState::new(buffer_capacity));

    // Create CoreAudio AudioUnit
    let _audio_unit = AudioUnitHandle::new(sample_rate, channels, consumer, Arc::clone(&state))?;

    event_tx
        .try_send(ThreadEvent::PlaybackChannelsChanged(channels))
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
    let mut pause_dropping = false;
    let mut resume_waiting_for_flush = false;
    let mut pending_frame: Option<crate::AudioFrame> = None;

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
                PlaybackCommand::Pause => {
                    state.flush_requested.store(true, Ordering::Relaxed);
                    pause_dropping = true;
                    end_of_stream = false;
                    drain_start = None;
                    if let Some(frame) = pending_frame.take() {
                        recycle_tx.try_send(frame.data).ok();
                    }
                }
                PlaybackCommand::Resume => {
                    resume_waiting_for_flush = true;
                }
                PlaybackCommand::UpdateSampleRate(new_rate) => {
                    if new_rate != sample_rate {
                        log::warn!(
                            "[Playback Thread iOS] Sample rate change {}→{} not supported at runtime on iOS",
                            sample_rate,
                            new_rate
                        );
                    }
                }
                PlaybackCommand::UpdateChannels(new_ch) => {
                    if new_ch != channels {
                        log::warn!(
                            "[Playback Thread iOS] Channel count change {}→{} not supported at runtime on iOS",
                            channels,
                            new_ch
                        );
                    }
                }
                PlaybackCommand::Reconfigure(request) => {
                    if !request.ticket.try_begin_execution() {
                        request
                            .reply_tx
                            .send(Err(
                                "iOS playback reconfiguration was cancelled before execution"
                                    .to_string(),
                            ))
                            .ok();
                    } else if !request.ticket.try_complete_execution() {
                        request
                            .reply_tx
                            .send(Err(
                                "iOS playback reconfiguration was cancelled before completion"
                                    .to_string(),
                            ))
                            .ok();
                    } else if request.requested.sample_rate == sample_rate
                        && request.requested.channels == channels
                    {
                        request
                            .reply_tx
                            .send(Ok(super::super::PlaybackConfiguration {
                                sample_rate,
                                channels,
                            }))
                            .ok();
                    } else {
                        request
                            .reply_tx
                            .send(Err(format!(
                                "iOS RemoteIO runtime reconfiguration from {}Hz/{}ch to {}Hz/{}ch is unsupported; rebuild the engine",
                                sample_rate,
                                channels,
                                request.requested.sample_rate,
                                request.requested.channels,
                            )))
                            .ok();
                    }
                }
                PlaybackCommand::Stop => {
                    state.flush_requested.store(true, Ordering::Relaxed);
                    flush_dropping = true;
                    end_of_stream = false;
                    drain_start = None;
                    if let Some(frame) = pending_frame.take() {
                        recycle_tx.try_send(frame.data).ok();
                    }
                }
                PlaybackCommand::Shutdown => {
                    log::debug!("[Playback Thread iOS] Shutting down");
                    if let Some(frame) = pending_frame.take() {
                        recycle_tx.try_send(frame.data).ok();
                    }
                    break;
                }
            }
        }

        if resume_waiting_for_flush && !state.flush_requested.load(Ordering::Relaxed) {
            resume_waiting_for_flush = false;
            pause_dropping = false;
        }

        // Read from message queue
        let message = if let Some(frame) = pending_frame.take() {
            Ok(ProcessingMessage::Frame(frame))
        } else {
            message_rx.try_recv()
        };
        match message {
            Ok(ProcessingMessage::Frame(frame)) => {
                if flush_dropping || pause_dropping {
                    recycle_tx.try_send(frame.data).ok();
                    continue;
                }

                if frame.num_channels != channels {
                    event_tx
                        .try_send(ThreadEvent::ProcessingError(format!(
                            "iOS playback requires {channels} channels, received {}",
                            frame.num_channels
                        )))
                        .ok();
                    recycle_tx.try_send(frame.data).ok();
                    continue;
                }

                // Write to ring buffer
                let frame_samples = frame.data.len();
                if producer.slots() < frame_samples {
                    pending_frame = Some(frame);
                    std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_RINGBUFFER));
                    continue;
                }
                match producer.write_chunk_uninit(frame_samples) {
                    Ok(chunk) => {
                        write_chunk_bulk(chunk, &frame.data);
                    }
                    Err(_) => {
                        pending_frame = Some(frame);
                        std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_RINGBUFFER));
                        continue;
                    }
                }
                recycle_tx.try_send(frame.data).ok();
            }
            Ok(ProcessingMessage::EndOfStream) => {
                if flush_dropping || pause_dropping {
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
                        event_tx.try_send(ThreadEvent::PlaybackDrained).ok();
                        end_of_stream = false;
                        drain_start = None;
                        continue;
                    }
                    if let Some(start) = drain_start {
                        if start.elapsed() > drain_timeout {
                            log::warn!("[Playback Thread iOS] Drain timeout, signaling completion");
                            event_tx.try_send(ThreadEvent::PlaybackDrained).ok();
                            end_of_stream = false;
                            drain_start = None;
                            continue;
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
                            event_tx.try_send(ThreadEvent::PlaybackDrained).ok();
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
