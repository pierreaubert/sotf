// Lock-free helpers for mutating the shared AudioEngineState snapshot.

use super::super::AudioEngineState;
use arc_swap::ArcSwap;
use std::sync::Arc;

/// Mutate the current state snapshot in-place.
///
/// Uses `Arc::make_mut` so the inner `AudioEngineState` is only cloned when
/// other readers still hold a reference to the previous `Arc`. This avoids the
/// unconditional full-state copy that the previous `(**state.load()).clone()`
/// pattern performed on every error path.
pub(super) fn update_engine_state<F>(state: &Arc<ArcSwap<AudioEngineState>>, f: F)
where
    F: FnOnce(&mut AudioEngineState),
{
    let mut current = state.load_full();
    f(Arc::make_mut(&mut current));
    state.store(current);
}
