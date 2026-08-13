// ============================================================================
// GC Thread - Background Deallocation for Audio Threads
// ============================================================================
//
// Receives Arc/Box garbage from audio-critical threads and drops it on a
// low-priority background thread, keeping deallocations off the audio path.

use sotf_plugins::PluginHost;
use std::any::Any;
use std::sync::Arc;

use super::PreparedTransitionDelay;

/// Sender half for the GC. Clone this and hand it to any thread
/// that produces Arc garbage (processing thread, manager thread, etc.)
pub type GcSender = crossbeam::channel::Sender<GcItem>;

/// Items the GC thread can drop
pub enum GcItem {
    /// An Arc<dyn Any> to be dropped on the GC thread
    AnyArc(Arc<dyn Any + Send + Sync>),
    /// A boxed value to drop
    Boxed(Box<dyn Any + Send>),
    /// A host replaced without a crossfade.
    PluginHost(Box<PluginHost>),
    /// A completed transition, including its heap-backed alignment storage.
    HostTransition {
        host: Box<PluginHost>,
        old_path_delay: PreparedTransitionDelay,
        new_path_delay: PreparedTransitionDelay,
    },
    /// Shutdown signal
    Shutdown,
}

pub struct GcThread {
    sender: GcSender,
    handle: Option<std::thread::JoinHandle<()>>,
}

impl GcThread {
    pub fn new() -> Result<Self, String> {
        // Host updates are serialized by the manager. A bounded queue therefore
        // provides ample headroom without allocating channel segments on the
        // processing thread.
        let (tx, rx) = crossbeam::channel::bounded::<GcItem>(64);
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

#[cfg(test)]
mod tests {
    use super::*;
    use sotf_plugins::{
        Parameter, ParameterId, ParameterValue, Plugin, PluginInfo, ProcessContext,
    };

    struct DropThreadPlugin {
        dropped_on: std::sync::mpsc::Sender<String>,
    }

    impl Drop for DropThreadPlugin {
        fn drop(&mut self) {
            let name = std::thread::current()
                .name()
                .unwrap_or("unnamed")
                .to_string();
            self.dropped_on.send(name).ok();
        }
    }

    impl Plugin for DropThreadPlugin {
        fn info(&self) -> PluginInfo {
            PluginInfo::new("drop-thread-probe", "0.1", "test")
        }
        fn input_channels(&self) -> usize {
            1
        }
        fn output_channels(&self) -> usize {
            1
        }
        fn parameters(&self) -> Vec<Parameter> {
            Vec::new()
        }
        fn set_parameter(&mut self, _: ParameterId, _: ParameterValue) -> Result<(), String> {
            Err("no parameters".into())
        }
        fn get_parameter(&self, _: &ParameterId) -> Option<ParameterValue> {
            None
        }
        fn process(
            &mut self,
            input: &[f32],
            output: &mut [f32],
            context: &ProcessContext,
        ) -> Result<usize, String> {
            output[..input.len()].copy_from_slice(input);
            Ok(context.num_frames)
        }
    }

    #[test]
    fn retired_plugin_host_is_destroyed_on_gc_thread() {
        let (drop_tx, drop_rx) = std::sync::mpsc::channel();
        let mut host = PluginHost::new(1, 48_000);
        host.add_plugin(Box::new(DropThreadPlugin {
            dropped_on: drop_tx,
        }))
        .unwrap();
        host.build().unwrap();

        let mut gc = GcThread::new().unwrap();
        gc.sender()
            .send(GcItem::PluginHost(Box::new(host)))
            .unwrap();
        gc.shutdown();

        assert_eq!(drop_rx.recv().unwrap(), "gc");
    }
}
