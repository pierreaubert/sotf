// ============================================================================
// GC Thread - Background Deallocation for Audio Threads
// ============================================================================
//
// Receives Arc/Box garbage from audio-critical threads and drops it on a
// low-priority background thread, keeping deallocations off the audio path.

use std::any::Any;
use std::sync::Arc;

/// Sender half for the GC. Clone this and hand it to any thread
/// that produces Arc garbage (processing thread, manager thread, etc.)
pub type GcSender = crossbeam::channel::Sender<GcItem>;

/// Items the GC thread can drop
pub enum GcItem {
    /// An Arc<dyn Any> to be dropped on the GC thread
    AnyArc(Arc<dyn Any + Send + Sync>),
    /// A boxed value to drop
    Boxed(Box<dyn Any + Send>),
    /// Shutdown signal
    Shutdown,
}

pub struct GcThread {
    sender: GcSender,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl GcThread {
    pub fn new() -> Result<Self, String> {
        let (tx, rx) = crossbeam::channel::unbounded::<GcItem>();
        let handle = std::thread::Builder::new()
            .name("gc".to_string())
            .spawn(move || {
                // Simply receive and drop items. The drop happens on this
                // low-priority background thread, not the audio thread.
                while let Ok(item) = rx.recv() {
                    match item {
                        GcItem::Shutdown => break,
                        _ => { /* item dropped here */ }
                    }
                }
            })
            .map_err(|e| format!("Failed to spawn GC thread: {}", e))?;

        Ok(Self {
            sender: tx,
            handle: Some(handle),
        })
    }

    pub fn sender(&self) -> GcSender {
        self.sender.clone()
    }

    pub fn shutdown(&mut self) {
        self.sender.send(GcItem::Shutdown).ok();
        if let Some(h) = self.handle.take() {
            h.join().ok();
        }
    }
}

impl Drop for GcThread {
    fn drop(&mut self) {
        self.shutdown();
    }
}
