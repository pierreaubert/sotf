use super::super::{DecoderThread, ProcessingThread};
use super::consts::SPIN_MS_SLEEP_MANAGER;
use super::error::ConfigError;

pub(in crate::engine::manager_thread) fn wait_for_processing_ack(
    processing: &ProcessingThread,
    timeout: std::time::Duration,
) -> Result<(), String> {
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        if let Some(response) = processing.try_recv_response() {
            match response {
                super::super::ProcessingResponse::Ok => return Ok(()),
                super::super::ProcessingResponse::Error(e) => return Err(e),
                _ => continue,
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_SLEEP_MANAGER));
    }

    Err(format!(
        "Timed out waiting for processing thread acknowledgment after {}ms",
        timeout.as_millis()
    ))
}

pub(in crate::engine::manager_thread) fn wait_for_plugin_chain_update(
    processing: &ProcessingThread,
    timeout: std::time::Duration,
) -> Result<(usize, usize), ConfigError> {
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        if let Some(response) = processing.try_recv_response() {
            match response {
                super::super::ProcessingResponse::PluginChainUpdated {
                    output_channels,
                    latency_samples,
                    ..
                } => return Ok((output_channels, latency_samples)),
                super::super::ProcessingResponse::Error(reason) => {
                    return Err(ConfigError::ProcessingError { reason });
                }
                super::super::ProcessingResponse::PluginData(_)
                | super::super::ProcessingResponse::Ok => {
                    continue;
                }
            }
        }

        std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_SLEEP_MANAGER));
    }

    Err(ConfigError::TimeoutError {
        waited_ms: timeout.as_millis() as u64,
    })
}

pub(in crate::engine::manager_thread) fn wait_for_decoder_ack(
    decoder: &DecoderThread,
    timeout: std::time::Duration,
) -> Result<(), String> {
    let start = std::time::Instant::now();

    while start.elapsed() < timeout {
        if let Some(response) = decoder.try_recv_response() {
            return match response {
                super::super::DecoderResponse::Ok => Ok(()),
                super::super::DecoderResponse::Error(e) => Err(e),
            };
        }

        std::thread::sleep(std::time::Duration::from_millis(SPIN_MS_SLEEP_MANAGER));
    }

    Err(format!(
        "Timed out waiting for decoder thread acknowledgment after {}ms",
        timeout.as_millis()
    ))
}
