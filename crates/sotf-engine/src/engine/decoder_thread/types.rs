use super::super::{DecoderCommand, DecoderMessage, DecoderResponse, ThreadEvent};
use super::consts::SPIN_MS_SLEEP_DECODER;
use super::decoder_state::DecoderState;
use sotf_types::DsdOutputMode;
use std::sync::mpsc::{Receiver, Sender, SyncSender};
use std::time::Duration;

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(not(all(target_os = "macos", feature = "hal")), allow(dead_code))]
pub(super) struct HalInputGuardTrip {
    pub(super) peak: f32,
    pub(super) invalid_samples: usize,
    pub(super) over_limit_samples: usize,
}

/// Action returned by decode loop
pub(super) enum DecoderLoopAction {
    Continue,
    Stop,
    Interrupted(DecoderCommand),
}

/// Main decoder thread function
pub(super) fn run_decoder_thread(
    message_tx: SyncSender<DecoderMessage>,
    command_rx: Receiver<DecoderCommand>,
    response_tx: Sender<DecoderResponse>,
    event_tx: Sender<ThreadEvent>,
    target_sample_rate: u32,
    frame_size: usize,
    recycle_rx: Receiver<Vec<f32>>,
    dsd_output: DsdOutputMode,
) -> Result<(), String> {
    let mut state = DecoderState::new(recycle_rx, dsd_output);

    log::info!(
        "[Decoder Thread] Started - target {}Hz, frame size {}",
        target_sample_rate,
        frame_size
    );

    loop {
        // Check for commands (non-blocking when playing/silent, blocking when stopped)
        let is_active = (state.decoder.is_some() && !state.paused) || state.silent_source;
        let command = if is_active {
            command_rx.try_recv().ok()
        } else {
            // Bounded wait when stopped/paused.  This keeps shutdown latency low
            // and lets us detect a dropped command sender promptly instead of
            // blocking forever on a channel that will never receive a command.
            match command_rx.recv_timeout(Duration::from_millis(SPIN_MS_SLEEP_DECODER)) {
                Ok(cmd) => Some(cmd),
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => break,
            }
        };

        if let Some(cmd) = command {
            match cmd {
                DecoderCommand::Play(source) => {
                    message_tx.send(DecoderMessage::Flush).ok();
                    state.stop();
                    #[cfg(feature = "streaming")]
                    state.clear_stream_metadata(&event_tx);
                    if let Err(e) = state.play(source, target_sample_rate, frame_size) {
                        log::debug!("[Decoder Thread] Play failed: {}", e);
                        event_tx.send(ThreadEvent::DecoderError(e)).ok();
                        response_tx
                            .send(DecoderResponse::Error(
                                "Failed to start playback".to_string(),
                            ))
                            .ok();
                    } else {
                        response_tx.send(DecoderResponse::Ok).ok();
                    }
                }
                DecoderCommand::PlayAt(source, position) => {
                    message_tx.send(DecoderMessage::Flush).ok();
                    state.stop();
                    #[cfg(feature = "streaming")]
                    state.clear_stream_metadata(&event_tx);
                    if let Err(e) = state.play(source, target_sample_rate, frame_size) {
                        log::debug!("[Decoder Thread] PlayAt (load) failed: {}", e);
                        event_tx.send(ThreadEvent::DecoderError(e)).ok();
                        response_tx
                            .send(DecoderResponse::Error(
                                "Failed to start playback".to_string(),
                            ))
                            .ok();
                    } else if let Err(e) = state.seek(position) {
                        log::debug!("[Decoder Thread] PlayAt (seek) failed: {}", e);
                        event_tx.send(ThreadEvent::DecoderError(e)).ok();
                        response_tx
                            .send(DecoderResponse::Error(
                                "Failed to seek during play_at".to_string(),
                            ))
                            .ok();
                    } else {
                        event_tx.send(ThreadEvent::SeekComplete).ok();
                        response_tx.send(DecoderResponse::Ok).ok();
                    }
                }
                DecoderCommand::StartSilentSource(channels) => {
                    state.start_silent_source(channels);
                }
                DecoderCommand::Pause => {
                    state.paused = true;
                    log::debug!("[Decoder Thread] Paused");
                    response_tx.send(DecoderResponse::Ok).ok();
                }
                DecoderCommand::Resume => {
                    state.paused = false;
                    log::debug!("[Decoder Thread] Resumed");
                    response_tx.send(DecoderResponse::Ok).ok();
                }
                DecoderCommand::Seek(position) => {
                    message_tx.send(DecoderMessage::Flush).ok();
                    if let Err(e) = state.seek(position) {
                        log::debug!("[Decoder Thread] Seek failed: {}", e);
                        response_tx.send(DecoderResponse::Error(e)).ok();
                    } else {
                        event_tx.send(ThreadEvent::SeekComplete).ok();
                        response_tx.send(DecoderResponse::Ok).ok();
                    }
                }
                DecoderCommand::QueueNext(source) => {
                    log::debug!("[Decoder Thread] Queued next: {}", source.display_name());
                    state.queued_next = Some(source);
                    response_tx.send(DecoderResponse::Ok).ok();
                }
                DecoderCommand::CancelNext => {
                    log::debug!("[Decoder Thread] Cancelled queued next");
                    state.queued_next = None;
                    response_tx.send(DecoderResponse::Ok).ok();
                }
                DecoderCommand::Stop => {
                    state.stop();
                    #[cfg(feature = "streaming")]
                    state.clear_stream_metadata(&event_tx);
                    message_tx.send(DecoderMessage::Flush).ok();
                    log::debug!("[Decoder Thread] Stopped");
                    response_tx.send(DecoderResponse::Ok).ok();
                }
                DecoderCommand::Shutdown => {
                    log::debug!("[Decoder Thread] Shutting down");
                    break;
                }
            }
        }

        #[cfg(feature = "streaming")]
        state.poll_stream_metadata(&event_tx);

        // Generate frames based on mode
        if state.silent_source && !state.paused {
            // HAL Input / Silent Source mode
            match state.process_hal_input(
                &message_tx,
                &command_rx,
                &response_tx,
                frame_size,
                target_sample_rate,
            ) {
                Ok((true, _)) => {
                    // Frame processed successfully
                    // Don't sleep if connected - rely on backpressure from message_tx
                }
                Ok((false, pending_cmd)) => {
                    // No frame processed — either not enough data or send was
                    // interrupted by a command. If a command was consumed from
                    // command_rx by send_or_interrupt, handle it now so it isn't lost.
                    if let Some(cmd) = pending_cmd {
                        // Re-inject the command by processing it inline.
                        // This mirrors the command handling at the top of the loop.
                        match cmd {
                            DecoderCommand::Stop => {
                                state.stop();
                                #[cfg(feature = "streaming")]
                                state.clear_stream_metadata(&event_tx);
                                message_tx.send(DecoderMessage::Flush).ok();
                                log::debug!("[Decoder Thread] Stopped (from HAL interrupt)");
                                response_tx.send(DecoderResponse::Ok).ok();
                            }
                            DecoderCommand::Shutdown => {
                                log::debug!("[Decoder Thread] Shutting down (from HAL interrupt)");
                                return Ok(());
                            }
                            DecoderCommand::Pause => {
                                state.paused = true;
                                log::debug!("[Decoder Thread] Paused (from HAL interrupt)");
                                response_tx.send(DecoderResponse::Ok).ok();
                            }
                            DecoderCommand::Resume => {
                                state.paused = false;
                                log::debug!("[Decoder Thread] Resumed (from HAL interrupt)");
                                response_tx.send(DecoderResponse::Ok).ok();
                            }
                            DecoderCommand::Seek(position) => {
                                message_tx.send(DecoderMessage::Flush).ok();
                                if let Err(e) = state.seek(position) {
                                    response_tx.send(DecoderResponse::Error(e)).ok();
                                } else {
                                    event_tx.send(ThreadEvent::SeekComplete).ok();
                                    response_tx.send(DecoderResponse::Ok).ok();
                                }
                            }
                            DecoderCommand::Play(path) => {
                                message_tx.send(DecoderMessage::Flush).ok();
                                state.stop();
                                #[cfg(feature = "streaming")]
                                state.clear_stream_metadata(&event_tx);
                                if let Err(e) = state.play(path, target_sample_rate, frame_size) {
                                    log::debug!(
                                        "[Decoder Thread] Play failed (from HAL interrupt): {}",
                                        e
                                    );
                                    event_tx.send(ThreadEvent::DecoderError(e)).ok();
                                    response_tx
                                        .send(DecoderResponse::Error(
                                            "Failed to start playback".to_string(),
                                        ))
                                        .ok();
                                } else {
                                    response_tx.send(DecoderResponse::Ok).ok();
                                }
                            }
                            DecoderCommand::PlayAt(path, position) => {
                                message_tx.send(DecoderMessage::Flush).ok();
                                state.stop();
                                #[cfg(feature = "streaming")]
                                state.clear_stream_metadata(&event_tx);
                                if let Err(e) = state.play(path, target_sample_rate, frame_size) {
                                    log::debug!(
                                        "[Decoder Thread] PlayAt failed (from HAL interrupt): {}",
                                        e
                                    );
                                    event_tx.send(ThreadEvent::DecoderError(e)).ok();
                                    response_tx
                                        .send(DecoderResponse::Error(
                                            "Failed to start playback".to_string(),
                                        ))
                                        .ok();
                                } else if let Err(e) = state.seek(position) {
                                    log::debug!(
                                        "[Decoder Thread] PlayAt seek failed (from HAL interrupt): {}",
                                        e
                                    );
                                    event_tx.send(ThreadEvent::DecoderError(e)).ok();
                                    response_tx
                                        .send(DecoderResponse::Error(
                                            "Failed to seek during play_at".to_string(),
                                        ))
                                        .ok();
                                } else {
                                    event_tx.send(ThreadEvent::SeekComplete).ok();
                                    response_tx.send(DecoderResponse::Ok).ok();
                                }
                            }
                            DecoderCommand::StartSilentSource(channels) => {
                                state.start_silent_source(channels);
                            }
                            DecoderCommand::QueueNext(path) => {
                                log::debug!(
                                    "[Decoder Thread] Queued next (from HAL interrupt): {:?}",
                                    path
                                );
                                state.queued_next = Some(path);
                                response_tx.send(DecoderResponse::Ok).ok();
                            }
                            DecoderCommand::CancelNext => {
                                log::debug!(
                                    "[Decoder Thread] Cancelled queued next (from HAL interrupt)"
                                );
                                state.queued_next = None;
                                response_tx.send(DecoderResponse::Ok).ok();
                            }
                        }
                    } else {
                        std::thread::sleep(std::time::Duration::from_millis(1));
                    }
                }
                Err(e) => {
                    crate::rate_limited_log!(warn, 5, "[Decoder Thread] HAL input error: {}", e);
                    state.stop();
                    #[cfg(feature = "streaming")]
                    state.clear_stream_metadata(&event_tx);
                }
            }

            // Handle sleep for non-HAL mode or disconnected HAL
            #[cfg(all(target_os = "macos", feature = "hal"))]
            {
                if state.hal_reader.as_ref().is_none_or(|r| !r.is_connected()) {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                }
            }
            #[cfg(not(all(target_os = "macos", feature = "hal")))]
            {
                // Non-HAL silent source mode: sleep to maintain frame rate
                let frame_duration_ms =
                    (frame_size as f64 / target_sample_rate as f64 * 1000.0) as u64;
                std::thread::sleep(std::time::Duration::from_millis(frame_duration_ms));
            }
        } else if state.decoder.is_some() && !state.paused {
            // File playback mode: decode from file
            match state.decode_chunk(
                &message_tx,
                &command_rx,
                &response_tx,
                &event_tx,
                frame_size,
                target_sample_rate,
            ) {
                Ok(DecoderLoopAction::Continue) => {
                    // Continue
                }
                Ok(DecoderLoopAction::Stop) => {
                    // End of stream - stop
                    state.stop();
                    #[cfg(feature = "streaming")]
                    state.clear_stream_metadata(&event_tx);
                }
                Ok(DecoderLoopAction::Interrupted(cmd)) => {
                    // Handle interruption command immediately
                    match cmd {
                        DecoderCommand::Play(path) => {
                            message_tx.send(DecoderMessage::Flush).ok();
                            state.stop();
                            #[cfg(feature = "streaming")]
                            state.clear_stream_metadata(&event_tx);
                            if let Err(e) = state.play(path, target_sample_rate, frame_size) {
                                log::debug!("[Decoder Thread] Play failed: {}", e);
                                event_tx.send(ThreadEvent::DecoderError(e)).ok();
                                response_tx
                                    .send(DecoderResponse::Error(
                                        "Failed to start playback".to_string(),
                                    ))
                                    .ok();
                            } else {
                                response_tx.send(DecoderResponse::Ok).ok();
                            }
                        }
                        DecoderCommand::PlayAt(path, position) => {
                            message_tx.send(DecoderMessage::Flush).ok();
                            state.stop();
                            #[cfg(feature = "streaming")]
                            state.clear_stream_metadata(&event_tx);
                            if let Err(e) = state.play(path, target_sample_rate, frame_size) {
                                log::debug!("[Decoder Thread] PlayAt (load) failed: {}", e);
                                event_tx.send(ThreadEvent::DecoderError(e)).ok();
                                response_tx
                                    .send(DecoderResponse::Error(
                                        "Failed to start playback".to_string(),
                                    ))
                                    .ok();
                            } else if let Err(e) = state.seek(position) {
                                log::debug!("[Decoder Thread] PlayAt (seek) failed: {}", e);
                                event_tx.send(ThreadEvent::DecoderError(e)).ok();
                                response_tx
                                    .send(DecoderResponse::Error(
                                        "Failed to seek during play_at".to_string(),
                                    ))
                                    .ok();
                            } else {
                                event_tx.send(ThreadEvent::SeekComplete).ok();
                                response_tx.send(DecoderResponse::Ok).ok();
                            }
                        }
                        DecoderCommand::StartSilentSource(channels) => {
                            state.start_silent_source(channels);
                        }
                        DecoderCommand::Pause => {
                            state.paused = true;
                            log::debug!("[Decoder Thread] Paused");
                            response_tx.send(DecoderResponse::Ok).ok();
                        }
                        DecoderCommand::Resume => {
                            state.paused = false;
                            log::debug!("[Decoder Thread] Resumed");
                            response_tx.send(DecoderResponse::Ok).ok();
                        }
                        DecoderCommand::Seek(position) => {
                            message_tx.send(DecoderMessage::Flush).ok();
                            if let Err(e) = state.seek(position) {
                                log::debug!("[Decoder Thread] Seek failed: {}", e);
                                response_tx.send(DecoderResponse::Error(e)).ok();
                            } else {
                                event_tx.send(ThreadEvent::SeekComplete).ok();
                                response_tx.send(DecoderResponse::Ok).ok();
                            }
                        }
                        DecoderCommand::QueueNext(path) => {
                            log::debug!(
                                "[Decoder Thread] Queued next (from interrupt): {:?}",
                                path
                            );
                            state.queued_next = Some(path);
                            response_tx.send(DecoderResponse::Ok).ok();
                        }
                        DecoderCommand::CancelNext => {
                            log::debug!("[Decoder Thread] Cancelled queued next (from interrupt)");
                            state.queued_next = None;
                            response_tx.send(DecoderResponse::Ok).ok();
                        }
                        DecoderCommand::Stop => {
                            state.stop();
                            #[cfg(feature = "streaming")]
                            state.clear_stream_metadata(&event_tx);
                            message_tx.send(DecoderMessage::Flush).ok();
                            log::debug!("[Decoder Thread] Stopped");
                            response_tx.send(DecoderResponse::Ok).ok();
                        }
                        DecoderCommand::Shutdown => {
                            log::debug!("[Decoder Thread] Shutting down");
                            break;
                        }
                    }
                }
                Err(e) => {
                    crate::rate_limited_log!(warn, 5, "[Decoder Thread] Error: {}", e);
                    state.stop();
                    #[cfg(feature = "streaming")]
                    state.clear_stream_metadata(&event_tx);
                }
            }
        } else {
            // Idle: small sleep to avoid busy loop when paused/stopped
            std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_SLEEP_DECODER));
        }
    }

    // log::debug!("[Decoder Thread] Stopped");
    Ok(())
}
