# Implementation Plan

## Phase 1: Engine Hot Path Optimization
- [ ] Task: Eliminate allocations in the real-time audio callback
    - [ ] Identify and replace `Vec` allocations with pre-allocated arrays or `rtrb` (ring buffers) in the hot path.
- [ ] Task: Remove locks from the hot path
    - [ ] Identify any `parking_lot::Mutex` or `RwLock` inside the audio callback.
    - [ ] Replace locks with atomic operations or `arc_swap::ArcSwap` for parameter reads.
- [ ] Task: Remove debug/trace statements
    - [ ] Search for and remove or disable `log::trace`, `log::debug` inside the audio processing loop.
- [ ] Task: Conductor - User Manual Verification 'Engine Hot Path Optimization' (Protocol in workflow.md)

## Phase 2: Parameter Update Mechanism Fixes
- [ ] Task: Fix parameter update scratchiness
    - [ ] Analyze how parameter changes are dispatched to plugins.
    - [ ] Ensure parameter smoothing (e.g., using `Smoother` struct) is applied consistently to prevent discontinuous jumps.
    - [ ] Implement double-buffering or `ArcSwap` for complex parameter structures (e.g., filter coefficients) to ensure atomic updates.
- [ ] Task: Conductor - User Manual Verification 'Parameter Update Mechanism Fixes' (Protocol in workflow.md)