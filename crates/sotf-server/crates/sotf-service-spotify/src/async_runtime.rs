use sotf_services::*;

/// Holds the runtime used to drive async HTTP calls when no ambient tokio
/// runtime is available.
///
/// Copied from `sotf-service-tidal/src/async_runtime.rs` — the two provider
/// crates deliberately stay independent (no cross-crate dep).
pub(super) struct AsyncRuntime {
    /// Fallback runtime used when the caller is not already inside one (or is
    /// inside a current-thread runtime where `block_in_place` would panic).
    ///
    /// This is a *multi-thread* runtime on purpose: librespot's `Session`
    /// captures `Handle::current()` and spawns long-lived tasks (packet
    /// dispatch, keepalive) that must keep being driven after `block_on`
    /// returns. A multi-thread runtime's workers do exactly that; a
    /// current-thread runtime would only make progress while `block_on` is
    /// active, silently stalling the session.
    ///
    /// `Option` so `Drop` can move it out: dropping a `Runtime` inside an
    /// async context panics, so in that case it is handed to a dedicated
    /// thread instead.
    pub(super) fallback: Option<tokio::runtime::Runtime>,
}

impl AsyncRuntime {
    pub(super) fn new() -> Result<Self, ServiceError> {
        let fallback = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|e| ServiceError::Other(format!("Failed to build tokio runtime: {}", e)))?;
        Ok(Self {
            fallback: Some(fallback),
        })
    }

    /// Drive `fut` to completion. Uses the ambient tokio runtime when running
    /// inside a multi-thread runtime (yielding the worker via
    /// `block_in_place`), otherwise falls back to the embedded runtime. This
    /// keeps the public trait sync without requiring it to become async.
    ///
    /// Caveat: when called from the driver thread of an ambient
    /// *current-thread* runtime, the fallback arm is selected, but
    /// `Runtime::block_on` panics there ("Cannot start a runtime from within
    /// a runtime") — this API must be called from a plain thread or a
    /// multi-thread runtime, never from inside a current-thread `block_on`.
    pub(super) fn block_on<F: std::future::Future>(&self, fut: F) -> F::Output {
        match tokio::runtime::Handle::try_current() {
            Ok(handle) => match handle.runtime_flavor() {
                tokio::runtime::RuntimeFlavor::MultiThread => {
                    tokio::task::block_in_place(|| handle.block_on(fut))
                }
                _ => self.fallback().block_on(fut),
            },
            Err(_) => self.fallback().block_on(fut),
        }
    }

    fn fallback(&self) -> &tokio::runtime::Runtime {
        self.fallback
            .as_ref()
            .expect("fallback runtime missing (taken by Drop)")
    }
}

impl Drop for AsyncRuntime {
    fn drop(&mut self) {
        let Some(runtime) = self.fallback.take() else {
            return;
        };
        if tokio::runtime::Handle::try_current().is_ok() {
            // Inside any tokio context, dropping a Runtime panics ("Cannot
            // drop a runtime in a context where blocking is not allowed").
            // Move the shutdown onto a plain thread with no runtime context.
            std::thread::spawn(move || drop(runtime));
        } else {
            drop(runtime);
        }
    }
}
