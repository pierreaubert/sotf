use sotf_services::*;

/// Holds the runtime used to drive async HTTP calls when no ambient tokio
/// runtime is available.
pub(super) struct AsyncRuntime {
    /// Fallback runtime used when the caller is a plain thread with no
    /// ambient tokio runtime.
    ///
    /// Note: this does *not* cover being called from a current-thread
    /// runtime's driver thread — `Runtime::block_on` panics there, so that
    /// calling context is unsupported.
    ///
    /// `Option` so `Drop` can move it out: dropping a `Runtime` inside an
    /// async context panics, so in that case it is handed to a dedicated
    /// thread instead.
    pub(super) fallback: Option<tokio::runtime::Runtime>,
}

impl AsyncRuntime {
    pub(super) fn new() -> Result<Self, ServiceError> {
        let fallback = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|e| ServiceError::Other(format!("Failed to build tokio runtime: {}", e)))?;
        Ok(Self {
            fallback: Some(fallback),
        })
    }

    /// Drive `fut` to completion.
    ///
    /// Supported calling contexts:
    /// - a plain thread with no ambient runtime — uses the embedded fallback
    ///   current-thread runtime;
    /// - a multi-thread runtime worker thread — yields the worker via
    ///   `block_in_place` and blocks on the ambient handle.
    ///
    /// Calling from a current-thread runtime's driver thread is *not*
    /// supported (`block_on` would panic there). This satisfies the "Tidal
    /// blocking inside async runtime" concern from the review without
    /// requiring the public trait to become async.
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
