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
    /// Recycled audio storage that could not fit in a realtime-local pool.
    Buffer(Vec<f32>),
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
                // A third-party plugin is allowed to have a broken destructor,
                // but it must not take down the reclaimer and push every later
                // destruction back toward the realtime pipeline.
                while let Ok(item) = rx.recv() {
                    match item {
                        GcItem::Shutdown => break,
                        item => {
                            if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                                drop(item);
                            }))
                            .is_err()
                            {
                                log::error!(
                                    "[GC Thread] A retired audio object panicked while being dropped; continuing"
                                );
                            }
                        }
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
        if let Some(handle) = self.handle.take()
            && super::join_timeout(handle, std::time::Duration::from_secs(5)).is_err()
        {
            log::warn!("[GC Thread] Shutdown join timed out; thread left detached");
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

    struct PanickingDrop;

    impl Drop for PanickingDrop {
        fn drop(&mut self) {
            panic!("intentional destructor panic");
        }
    }

    struct DropSignal(std::sync::mpsc::Sender<()>);

    impl Drop for DropSignal {
        fn drop(&mut self) {
            self.0.send(()).ok();
        }
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

    #[test]
    fn panicking_destructor_does_not_kill_gc_thread() {
        let (drop_tx, drop_rx) = std::sync::mpsc::channel();
        let mut gc = GcThread::new().unwrap();
        gc.sender()
            .send(GcItem::Boxed(Box::new(PanickingDrop)))
            .unwrap();
        gc.sender()
            .send(GcItem::Boxed(Box::new(DropSignal(drop_tx))))
            .unwrap();

        assert!(
            drop_rx
                .recv_timeout(std::time::Duration::from_secs(1))
                .is_ok(),
            "GC must continue receiving after a destructor panic"
        );
        gc.shutdown();
    }
}
