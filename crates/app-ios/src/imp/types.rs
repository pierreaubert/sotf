use gpui::*;
use std::path::PathBuf;

/// Remote control commands that require AppState/Queue access. The GPUI event
/// loop drains this queue on each tick through `sotf_ios_pop_remote_command`.
#[derive(Debug, Clone)]
pub enum RemoteCommand {
    NextTrack,
    PrevTrack,
    /// File paths imported from the iOS document picker. The consumer should
    /// either add them to the library or push them onto the playback queue.
    ImportFiles(Vec<PathBuf>),
    /// A QR code payload was scanned by the native camera view.
    QrPayloadScanned,
}
