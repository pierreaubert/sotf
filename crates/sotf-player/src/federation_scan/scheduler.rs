use super::fetch::fetch_source_albums;
use super::types::{FederationScanResult, ScanProgressFn, merge_albums_to_db};
use crate::database::MusicDatabase;
use crate::federation_config::FederationSourceEntry;
use std::sync::atomic::{AtomicBool, Ordering};

/// Progress callback invoked after a source's provider snapshot has been fetched.
pub type FetchProgressFn = Box<dyn Fn(usize) + Send>;

#[derive(Debug, Clone, Default)]
pub struct FederationSyncSummary {
    pub sources_total: usize,
    pub sources_synced: usize,
    pub sources_failed: usize,
    pub albums: usize,
    pub tracks: usize,
    pub results: Vec<FederationScanResult>,
}

impl FederationSyncSummary {
    fn push(&mut self, result: FederationScanResult) {
        self.sources_total += 1;
        if result.error.is_some() {
            self.sources_failed += 1;
        } else {
            self.sources_synced += 1;
            self.albums += result.albums;
            self.tracks += result.tracks;
        }
        self.results.push(result);
    }
}

/// Sync every enabled federation source sequentially.
///
/// Each source is fetched, full-refreshed into the local DB, marked available on
/// success, and marked unavailable on provider/merge failure. Sequential sync
/// keeps database writes deterministic and avoids overlapping full-refresh
/// unmerge/merge cycles for the same source.
pub async fn sync_enabled_federation_sources(
    sources: &[FederationSourceEntry],
    cancel: &AtomicBool,
    on_fetched: Option<&FetchProgressFn>,
    on_progress: Option<&ScanProgressFn>,
) -> FederationSyncSummary {
    let mut summary = FederationSyncSummary::default();

    for source in sources.iter().filter(|source| source.is_enabled) {
        if cancel.load(Ordering::Relaxed) {
            break;
        }
        summary.push(sync_federation_source(source, cancel, on_fetched, on_progress).await);
    }

    summary
}

/// Sync one federation source into the default music database.
pub async fn sync_federation_source(
    source: &FederationSourceEntry,
    cancel: &AtomicBool,
    on_fetched: Option<&FetchProgressFn>,
    on_progress: Option<&ScanProgressFn>,
) -> FederationScanResult {
    let source_id = source.source_id.clone();
    let albums = match fetch_source_albums(source).await {
        Ok(albums) => albums,
        Err(result) => {
            let _ = update_source_sync_state(&source_id, false, false);
            return result;
        }
    };
    if let Some(cb) = on_fetched {
        cb(albums.len());
    }

    let result = merge_albums_to_db(&source_id, &albums, cancel, on_progress);
    match result.error.as_deref() {
        None => {
            if let Err(err) = update_source_sync_state(&source_id, true, true) {
                return FederationScanResult {
                    source_id,
                    albums: result.albums,
                    tracks: result.tracks,
                    error: Some(err),
                };
            }
        }
        Some("cancelled") => {}
        Some(_) => {
            let _ = update_source_sync_state(&source_id, false, false);
        }
    }

    result
}

fn update_source_sync_state(
    source_id: &str,
    available: bool,
    update_sync_time: bool,
) -> Result<(), String> {
    let path =
        MusicDatabase::default_path().ok_or_else(|| "no database path configured".to_string())?;
    let db =
        MusicDatabase::open_secondary(path).map_err(|e| format!("failed to open database: {e}"))?;
    db.set_source_availability(source_id, available)?;
    if update_sync_time {
        db.update_federation_source_sync_time(source_id)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::federation_config::SourceConnectionConfig;

    #[tokio::test]
    async fn sync_enabled_federation_sources_skips_disabled_sources() {
        let cancel = AtomicBool::new(false);
        let sources = vec![FederationSourceEntry {
            source_id: "peer:disabled".to_string(),
            display_name: "Disabled".to_string(),
            priority: 0,
            is_enabled: false,
            connection: SourceConnectionConfig::Peer {
                host: "127.0.0.1".to_string(),
                port: 8732,
                accepted_fingerprint: None,
                auth_token: Some("secret".to_string()),
            },
            is_available: None,
        }];

        let summary = sync_enabled_federation_sources(&sources, &cancel, None, None).await;

        assert_eq!(summary.sources_total, 0);
        assert!(summary.results.is_empty());
    }
}
