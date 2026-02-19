# Specification: GPUI Audio Performance Optimization

## Overview
This track addresses audible performance issues (crackling, pops, and dropouts) specifically within the `app-gpui` application. While the underlying audio engine and DSP plugins perform correctly in the TUI application, the GPUI implementation suffers from real-time processing instability, particularly when high-load plugins are active, visualizers are rendering, or the user is interacting with the UI.

## Problem Statement
The GPUI application appears to be introducing latency or lock contention that prevents the audio engine from meeting its real-time deadlines. This manifests as buffer underruns when:
1. High-load plugins (Convolution, Upmixer, Binaural) are active.
2. Real-time visualizers (Spectrum Analyzer, Level Meters) are rendering.
3. UI interactions (resizing windows, moving sliders) occur.

## Functional Requirements
- **Real-Time Data Decoupling:** Ensure that the data path for visualizers (Spectrum, Meters) uses non-blocking primitives (e.g., lock-free rings or atomic swaps) to prevent the GPUI main thread from stalling the audio processing thread.
- **Event Throttling:** Implement or verify throttling for UI-driven parameter updates (e.g., slider movements) to prevent an explosion of messages to the audio engine.
- **Interaction Stability:** Optimize window management and rendering tasks to ensure they do not consume resources or hold locks required by the audio callback.
- **Resource Monitoring:** Implement or leverage existing instrumentation to identify exact points of contention between GPUI and the Audio Engine.

## Non-Functional Requirements
- **Real-Time Integrity:** Zero audible artifacts during playback across all supported plugin configurations.
- **UI Responsiveness:** Maintain a fluid 60 FPS (or native refresh rate) UI without sacrificing audio stability.
- **Platform Consistency:** Ensure fixes apply to all GPUI-supported platforms (macOS, Linux, Windows).

## Acceptance Criteria
- **Parity with TUI:** `app-gpui` must play audio with the same plugin configurations as `app-tui` without audible glitches.
- **Interaction Stress Test:** Rapidly resizing the window or adjusting complex plugin sliders does not cause audio dropouts.
- **Visualizer Stability:** Full-screen spectrum visualization remains active without impacting playback quality.

## Out of Scope
- Optimization of the core audio engine DSP logic (unless a GPUI-specific integration bug is discovered).
- Visual or functional redesign of the GPUI components.
- Performance tuning for the TUI application.
