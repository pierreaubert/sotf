use crate::app::state::app::FederationScanMessage;
use sotf_audio_player::federation_config::{ConnectionStatus, FederationSourceEntry};
use std::net::IpAddr;

/// Run a structured diagnostic test against a federation source.
/// Delegates to the shared implementation in sotf-player.
pub fn test_federation_connection(source: &FederationSourceEntry) -> ConnectionStatus {
    sotf_audio_player::federation_scan::run_connection_diagnostic(source)
}

/// Scan a federation source using the shared pipeline.
/// Sends progress messages via `tx`. Checks `cancel` flag between albums.
pub(super) async fn scan_federation_source_async(
    source: &FederationSourceEntry,
    tx: &std::sync::mpsc::Sender<FederationScanMessage>,
    cancel: &std::sync::atomic::AtomicBool,
) {
    use sotf_audio_player::federation_scan;

    let tx_fetched = tx.clone();
    let fetched_cb: federation_scan::FetchProgressFn = Box::new(move |total| {
        let _ = tx_fetched.send(FederationScanMessage::FetchedAlbums { total });
    });
    let tx_progress = tx.clone();
    let progress_cb: federation_scan::ScanProgressFn = Box::new(move |a, t| {
        let _ = tx_progress.send(FederationScanMessage::Progress {
            albums_merged: a,
            tracks_merged: t,
        });
    });

    let result = federation_scan::sync_federation_source(
        source,
        cancel,
        Some(&fetched_cb),
        Some(&progress_cb),
    )
    .await;

    let _ = tx.send(FederationScanMessage::Done(result));
}

pub(super) fn run_sotf_api_request<T, F>(future: F) -> Result<T, String>
where
    F: std::future::Future<Output = sotf_audio_player::sotf_api_client::SotfApiResult<T>>,
{
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|err| format!("failed to start SOTF API runtime: {err}"))?;
    rt.block_on(future).map_err(|err| err.to_string())
}

pub(super) fn pairing_qr_host(bind_address: &str) -> Option<String> {
    if let Ok(IpAddr::V4(addr)) = bind_address.parse::<IpAddr>()
        && !addr.is_unspecified()
        && !addr.is_loopback()
    {
        return Some(addr.to_string());
    }

    sotf_tls::cert_gen::local_ip_addresses()
        .into_iter()
        .find_map(|addr| match addr {
            IpAddr::V4(v4) if !v4.is_loopback() => Some(v4.to_string()),
            _ => None,
        })
}
