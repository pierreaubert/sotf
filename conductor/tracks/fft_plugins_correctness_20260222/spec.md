# Specification: FFT Plugins Sound Quality & Correctness

## Overview
Improve the sound quality and algorithmic correctness of the FFT-based plugins, specifically the **Stereo-to-Multichannel Upmixer** and the **Crosstalk Cancellation (XTC)** plugin. This track addresses audible artifacts, volume saturation, and steering inaccuracies that degrade the user experience.

## Functional Requirements

### 1. XTC Plugin Stabilization
- **Gain Management:** Investigate and fix the volume saturation issue. Ensure the internal gain scaling and OLA (Overlap-Add) normalization prevent digital clipping.
- **Algorithm Stability:** Review the matrix inversion and regularization logic to ensure it remains stable across all frequency bins, particularly at low frequencies and during rapid parameter updates.
- **Soft Limiting:** Verify if a soft-limiting or peak-safety mechanism is needed within the frequency domain processing to handle aggressive cancellation boosts.

### 2. Upmixer Logic Refinement
- **Steering & Separation:** Address artifacts in surround and top channels. Improve the direct/ambient decomposition to prevent voice "leakage" or smearing in non-front channels.
- **Phase Alignment:** Verify phase consistency between direct and ambient paths across all output channels to eliminate audible phasing or "hollow" sound.
- **Voice Handling:** Specifically optimize the steering logic for vocal content to ensure it remains focused and artifact-free in the upmixed soundstage.

### 3. DSP Infrastructure
- **OLA Normalization:** Audit the STFT framework used by both plugins to ensure mathematically correct scaling for the chosen window (Hann) and overlap (e.g., 50% or 75%).

## Non-Functional Requirements
- **Audio Integrity:** The primary goal is artifact-free, high-fidelity audio.
- **Real-Time Safety:** All fixes must maintain zero heap allocations in the audio hot path.

## Acceptance Criteria
- XTC plugin no longer saturates or clips under standard operating conditions.
- Upmixer produces clear audio in surround and height channels without audible artifacts on vocal tracks.
- Existing and new validation benchmarks for both plugins pass with "Clean" status.
- No regression in CPU performance compared to the current baseline.

## Out of Scope
- Optimizing other FFT plugins like PND or Binaural (unless shared infrastructure is affected).
- Implementing new UI features for these plugins.
