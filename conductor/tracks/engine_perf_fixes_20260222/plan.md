# Implementation Plan

## Phase 1: Engine Hot Path Optimization
- [x] Task: Eliminate allocations in the real-time audio callback
    - [x] Identify and replace `Vec` allocations with pre-allocated arrays or `rtrb` (ring buffers) in the hot path.
- [x] Task: Remove locks from the hot path
    - [x] Identify any `parking_lot::Mutex` or `RwLock` inside the audio callback.
    - [x] Replace locks with atomic operations or `arc_swap::ArcSwap` for parameter reads.
- [x] Task: Remove debug/trace statements
    - [x] Search for and remove or disable `log::trace`, `log::debug` inside the audio processing loop.
- [x] Task: Conductor - User Manual Verification 'Engine Hot Path Optimization' (Protocol in workflow.md)

## Phase 2: Parameter Update Mechanism Fixes
- [x] Task: Fix parameter update scratchiness
    - [x] Analyze how parameter changes are dispatched to plugins.
    - [x] Ensure parameter smoothing (e.g., using `Smoother` struct) is applied consistently to prevent discontinuous jumps.
    - [x] Implement double-buffering or `ArcSwap` for complex parameter structures (e.g., filter coefficients) to ensure atomic updates.
- [x] Task: Conductor - User Manual Verification 'Parameter Update Mechanism Fixes' (Protocol in workflow.md)