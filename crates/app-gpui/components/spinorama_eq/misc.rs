/// Create a runtime for one Spinorama request.
///
/// Requests already execute on worker threads, so sharing a mutable process-wide
/// runtime only couples unrelated requests and turns an initialization failure into
/// a process panic. Returning the creation error keeps failure recovery in the UI.
pub(super) fn spinorama_runtime() -> Result<tokio::runtime::Runtime, String> {
    tokio::runtime::Runtime::new()
        .map_err(|error| format!("Failed to create Spinorama network runtime: {error}"))
}
