# Specification: Engine Performance and Bug Fixes

## Overview
Optimize the `engine` crate for real-time audio performance and fix identified bugs, specifically targeting parameter update issues that cause scratching sounds.

## Functional Requirements
- Refactor the audio processing hot path to ensure absolute real-time safety.
- Eliminate all heap allocations in the real-time audio callback.
- Remove all `parking_lot` locks or any other mutexes/locks from the hot path.
- Implement or verify lock-free parameter update mechanisms (e.g., using `ArcSwap` or double-buffering) to prevent scratching audio artifacts.
- Remove debugging/trace statements from the hot path.

## Non-Functional Requirements
- **Performance:** Maintain low latency and zero buffer underruns.
- **Stability:** Prevent audio artifacts (scratching, popping) during rapid parameter changes.

## Acceptance Criteria
- No locks are acquired in the audio processing thread.
- No memory allocations occur in the audio processing thread.
- Rapid parameter changes do not result in audio glitches or scratchiness.
- Existing unit and integration tests for the engine pass.

## Out of Scope
- Architectural changes to the UI or non-engine crates.
- Adding new DSP algorithms or plugins.