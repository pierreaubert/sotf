use super::super::{DecoderThread, ProcessingThread};
use super::consts::SPIN_MS_SLEEP_MANAGER;
use super::error::ConfigError;

pub(in crate::engine::manager_thread) fn wait_for_processing_ack(
    processing: &ProcessingThread,
    request_id: u64,
    timeout: std::time::Duration,
) -> Result<(), String> {
    let deadline = std::time::Instant::now() + timeout;
    let mut reconciling_claimed_request = false;

    loop {
        if let Some(response) = processing.try_recv_response_for(request_id) {
            match response {
                super::super::ProcessingResponse::Ok => return Ok(()),
                super::super::ProcessingResponse::Error(e) => return Err(e),
                _ => continue,
            }
        }

        if !reconciling_claimed_request && std::time::Instant::now() >= deadline {
            if !reconciling_claimed_request && !processing.cancel_request(request_id) {
                reconciling_claimed_request = true;
                continue;
            }
            processing.abandon_request(request_id);
            return Err(format!(
                "Timed out waiting for processing thread acknowledgment after {}ms",
                timeout.as_millis()
            ));
        }

        if reconciling_claimed_request && processing.is_finished() {
            processing.abandon_request(request_id);
            return Err("Processing thread stopped while completing a claimed command".to_string());
        }

        std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_SLEEP_MANAGER));
    }
}

pub(in crate::engine::manager_thread) fn wait_for_parameter_update(
    processing: &ProcessingThread,
    request_id: u64,
    timeout: std::time::Duration,
) -> Result<(usize, u32, usize), String> {
    let deadline = std::time::Instant::now() + timeout;
    let mut reconciling_claimed_request = false;
    loop {
        if let Some(response) = processing.try_recv_response_for(request_id) {
            match response {
                super::super::ProcessingResponse::ParameterUpdated {
                    output_channels,
                    output_sample_rate,
                    latency_samples,
                } => return Ok((output_channels, output_sample_rate, latency_samples)),
                super::super::ProcessingResponse::Error(e) => return Err(e),
                _ => continue,
            }
        }
        if !reconciling_claimed_request && std::time::Instant::now() >= deadline {
            if !reconciling_claimed_request && !processing.cancel_request(request_id) {
                reconciling_claimed_request = true;
                continue;
            }
            processing.abandon_request(request_id);
            return Err(format!(
                "Timed out waiting for parameter update after {}ms",
                timeout.as_millis()
            ));
        }
        if reconciling_claimed_request && processing.is_finished() {
            processing.abandon_request(request_id);
            return Err(
                "Processing thread stopped while completing a claimed parameter update".to_string(),
            );
        }
        std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_SLEEP_MANAGER));
    }
}

pub(in crate::engine::manager_thread) fn wait_for_plugin_chain_update(
    processing: &ProcessingThread,
    request_id: u64,
    generation: u64,
    ticket: &super::super::HostUpdateTicket,
    timeout: std::time::Duration,
) -> Result<(usize, u32, usize), ConfigError> {
    let deadline = std::time::Instant::now() + timeout;
    let mut reconciling_committed_update = false;

    loop {
        if let Some(response) = processing.try_recv_response_for(request_id) {
            match response {
                super::super::ProcessingResponse::PluginChainUpdated {
                    generation: response_generation,
                    output_channels,
                    output_sample_rate,
                    latency_samples,
                    ..
                } if response_generation == generation => {
                    return Ok((output_channels, output_sample_rate, latency_samples));
                }
                super::super::ProcessingResponse::PluginChainUpdated { .. } => continue,
                super::super::ProcessingResponse::Error(reason) => {
                    return Err(ConfigError::ProcessingError { reason });
                }
                super::super::ProcessingResponse::PluginData(_)
                | super::super::ProcessingResponse::ParameterUpdated { .. }
                | super::super::ProcessingResponse::Ok => {
                    continue;
                }
            }
        }

        if !reconciling_committed_update && std::time::Instant::now() >= deadline {
            if !reconciling_committed_update && !ticket.cancel() {
                // Processing atomically claimed the update before our timeout.
                // It is no longer cancellable, so reconcile its matching ACK
                // instead of returning a false failure for a committed host.
                reconciling_committed_update = true;
                continue;
            }

            processing.invalidate_host_update(generation);
            return Err(ConfigError::TimeoutError {
                waited_ms: timeout.as_millis() as u64,
            });
        }

        if reconciling_committed_update && processing.is_finished() {
            processing.invalidate_host_update(generation);
            return Err(ConfigError::ProcessingError {
                reason: "Processing thread stopped after claiming a host update before it acknowledged the terminal state".to_string(),
            });
        }

        std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_SLEEP_MANAGER));
    }
}

pub(in crate::engine::manager_thread) fn wait_for_decoder_ack(
    decoder: &DecoderThread,
    request_id: u64,
    timeout: std::time::Duration,
) -> Result<(), String> {
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        if let Some(response) = decoder.try_recv_response_for(request_id) {
            return match response {
                super::super::DecoderResponse::Ok => Ok(()),
                super::super::DecoderResponse::Error(e) => Err(e),
            };
        }

        std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_SLEEP_MANAGER));
    }

    decoder.abandon_request(request_id);
    Err(format!(
        "Timed out waiting for decoder thread acknowledgment after {}ms",
        timeout.as_millis()
    ))
}
