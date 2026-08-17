// ============================================================================
// Service-Stream Resolver Hook
// ============================================================================
//
// The engine knows nothing about specific streaming services (Spotify, Tidal,
// ...). A higher layer (e.g. sotf-player's `ServiceManager`) installs a
// resolver at startup via [`set_service_stream_resolver`]; when the decoder
// thread encounters an [`AudioSource::ServiceStream`] it calls the resolver to
// obtain either a directly decodable URL or a pre-decoded PCM stream.
//
// Keeping this as a single generic hook means the engine never depends on
// provider crates, and resolution happens uniformly on the decoder thread —
// including gapless next-track preloading.

use crate::decoder::source::ServiceId;
use std::io::Read;
use std::sync::{Arc, RwLock};

/// The result of resolving a service stream track into something the engine
/// can decode directly.
pub enum ResolvedServiceStream {
    /// A URL decodable by the normal HTTP streaming path (e.g. Tidal FLAC).
    Url {
        url: String,
        /// Hint for Symphonia format detection (e.g. "flac").
        format_hint: Option<String>,
        seekable: bool,
    },
    /// Pre-decoded interleaved f32 little-endian PCM (e.g. Spotify via
    /// librespot). Fed into [`crate::decoder::PcmDecoder`].
    ///
    /// The reader must deliver whole f32 samples (byte counts divisible by
    /// 4): `PcmDecoder` cannot carry a partial-sample remainder across reads,
    /// and a misaligned reader would corrupt every following sample.
    ///
    /// Two further contract points for resolver authors:
    ///
    /// - `channels` must match the engine's configured input channel count.
    ///   The plugin chain is built for the manager's (currently stereo)
    ///   placeholder spec before the resolver runs, and nothing re-checks the
    ///   real channel count at runtime — a mismatched resolver would feed
    ///   N-channel frames into a 2-channel host. Reporting the resolved spec
    ///   back to the manager is a known follow-up.
    /// - The reader must be interruptible or timeout-bound. `PcmDecoder`
    ///   blocks in `reader.read()` on the decoder thread, so a stalled reader
    ///   (e.g. network hang) leaves Pause/Stop/Shutdown unanswered until the
    ///   5 s decoder-shutdown timeout expires and the thread is abandoned.
    Pcm {
        sample_rate: u32,
        channels: u16,
        /// Metadata only — samples on the wire are always f32.
        bits_per_sample: u16,
        /// Total frames if known (None for live/infinite streams).
        total_frames: Option<u64>,
        reader: Box<dyn Read + Send>,
    },
}

impl std::fmt::Debug for ResolvedServiceStream {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ResolvedServiceStream::Url { url, seekable, .. } => f
                .debug_struct("Url")
                .field("url", url)
                .field("seekable", seekable)
                .finish(),
            ResolvedServiceStream::Pcm {
                sample_rate,
                channels,
                total_frames,
                ..
            } => f
                .debug_struct("Pcm")
                .field("sample_rate", sample_rate)
                .field("channels", channels)
                .field("total_frames", total_frames)
                .finish(),
        }
    }
}

/// Resolver callback: maps (service, track id) to a decodable stream.
/// The error string is surfaced to the user via
/// [`crate::decoder::AudioDecoderError::ServiceError`].
pub type ServiceStreamResolver =
    Arc<dyn Fn(ServiceId, &str) -> Result<ResolvedServiceStream, String> + Send + Sync>;

static RESOLVER: RwLock<Option<ServiceStreamResolver>> = RwLock::new(None);

/// Install (or replace) the service-stream resolver. Called once at app
/// startup by the layer that owns streaming-service credentials.
pub fn set_service_stream_resolver(resolver: ServiceStreamResolver) {
    let mut guard = RESOLVER.write().unwrap_or_else(|e| e.into_inner());
    *guard = Some(resolver);
}

/// Remove the installed resolver (mainly for tests and shutdown).
pub fn clear_service_stream_resolver() {
    let mut guard = RESOLVER.write().unwrap_or_else(|e| e.into_inner());
    *guard = None;
}

/// Returns `true` when a resolver is installed (cheap check for callers that
/// want to report "not configured" early).
pub fn has_service_stream_resolver() -> bool {
    let guard = RESOLVER.read().unwrap_or_else(|e| e.into_inner());
    guard.is_some()
}

/// Resolve a service stream. `None` means no resolver is installed.
///
/// The resolver `Arc` is cloned out and the lock released *before* the
/// resolver runs: resolvers do network I/O and may re-enter this module
/// (e.g. uninstalling themselves on an auth failure), so holding the read
/// guard across the call would deadlock or starve writers.
///
/// A panicking resolver is caught and reported as an error string rather
/// than unwinding into the decoder thread.
pub(crate) fn resolve_service_stream(
    service: ServiceId,
    track_id: &str,
) -> Option<Result<ResolvedServiceStream, String>> {
    let resolver = {
        let guard = RESOLVER.read().unwrap_or_else(|e| e.into_inner());
        guard.clone()
    };
    resolver.map(|resolver| {
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| resolver(service, track_id)))
            .unwrap_or_else(|_| Err("service stream resolver panicked".to_string()))
    })
}

// NOTE: no unit tests here — the resolver is process-global, so all
// install/resolve/clear behavior is exercised by the single combined test in
// `core.rs::tests::test_create_decoder_from_source_service_stream`, which
// cannot race with itself.
