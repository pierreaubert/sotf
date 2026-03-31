// ============================================================================
// sotf-federation — Library Federation
// ============================================================================
//
// Multi-source library providers, merge engine, and sync for SOTF.
//
// This crate provides:
// - `LibraryProvider` trait for any source contributing albums/tracks
// - Deterministic UUID generation for cross-instance identity
// - Source registry for managing configured providers
// - Merge engine for cross-source album deduplication (future)
// - Sync scheduler for periodic/event-driven sync (future)

pub mod dlna_provider;
pub mod identity;
pub mod local_provider;
pub mod mpd_provider;
pub mod provider;
pub mod registry;

pub use dlna_provider::{DlnaProvider, DlnaProviderConfig};
pub use identity::{album_uuid, track_uuid};
pub use local_provider::{LocalFilesProvider, LocalProviderConfig, LocalTrackInfo};
pub use mpd_provider::{MpdProvider, MpdProviderConfig};
pub use provider::{
    LibraryEvent, LibraryProvider, ProviderAlbum, ProviderCapabilities, ProviderError,
    ProviderFuture, ProviderTrack, SourceId, SourceType,
};
pub use registry::{SourceConfig, SourceRegistry};
