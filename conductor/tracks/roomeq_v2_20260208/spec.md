# Track: RoomEQ v2 - Core Optimizer Refactor

## 1. Overview
This track focuses on a major refactor of the `roomeq` optimization engine to support the "RoomEQ v2" Version 1.0 specification. The primary goal is to **orchestrate** existing components (MSO, DBA, basic modes) into a strict, specification-compliant pipeline, update the configuration schema, and ensure the entire ecosystem (tests, datagen, documentation) adapts to the new input format. A key new addition will be the integration of Group Delay Optimization (GD-Opt).

## 2. Functional Requirements

### 2.1 Configuration & Schema
*   **Unified Config:** Update `RoomConfig` to explicitly support the v2 specification structure.
    *   **Processing Modes:** `LowLatency` (IIR), `PhaseLinear` (FIR), `Hybrid` (Mixed).
    *   **Bass Management:** Consolidated configuration for Single, MSO, and DBA strategies.
*   **Ecosystem Compatibility:** `autoeq-datagen` and `convert_recording` must be updated to produce/consume the new v2 config format.

### 2.2 Pipeline Orchestration (The "Conductor" Engine)
The optimizer must enforce the following execution sequence:
1.  **Sanity Check:** Detect polarity errors or dead channels.
2.  **Bass Logic:** Execute the selected strategy (Single, MSO, or DBA) to produce a unified "Virtual Sub" response.
    *   *Constraint:* Ensure Pre-Correction/Linearization of drivers happens *before* complex bass optimization if required.
3.  **Main Alignment:** Align all channels to T-Zero (furthest speaker).
4.  **Processing Mode Execution:** Apply filters based on the selected mode (A, B, or C).

### 2.3 Processing Modes Logic
*   **Mode A (Low-Latency):** Enforce IIR-only filters and Linkwitz-Riley (LR4) crossovers.
*   **Mode B (Phase-Linear):** Enforce FIR filters, Linear Phase "Brick Wall" crossovers, and windowing.
*   **Mode C (Hybrid):** Enforce IIR for Bass (< 250Hz) and FIR for Mids/Highs.

### 2.4 Calibration Algorithms (New & Refined)
*   **Group Delay Optimization (GD-Opt):**
    *   Compute Group Delay derivative from unwrapped phase.
    *   **IIR Mode:** Generate All-Pass filters for Mains to match Sub's GD.
    *   **FIR/Hybrid Mode:** Generate Excess Phase inversion for Sub.
*   **Timbre Matching (Voice of God):** Ensure satellite channels are matched to the reference target (Magnitude + Phase).

## 3. Non-Functional Requirements
*   **Backward Compatibility:** While the internal engine changes, provide clear error messages or migration paths for old config files if possible (though v2 is a breaking change).
*   **Maintainability:** The pipeline code should be modular, making it easy to swap out the "Bass Logic" or "Main Alignment" modules.

## 4. Acceptance Criteria
*   **Schema Validation:** `autoeq-datagen` produces valid v2 configs that `roomeq` accepts without error.
*   **Pipeline Verification:**
    *   Running a 2.1 system in **Mode A** results in a `dsp_chain` with only IIR Peaking/Shelf/All-Pass filters and LR4 crossovers.
    *   Running a system in **Mode C** results in a `dsp_chain` with IIR filters for low freq and Convolution (FIR) for high freq.
*   **Bass Logic:** MSO and DBA configurations correctly trigger their respective existing logic and integrate the result into the main alignment.
*   **GD-Opt:** The output chain contains specific All-Pass filters (IIR) or Phase corrections (FIR) explicitly targeting the crossover region delay.

## 5. Out of Scope
*   **UI Implementation:** No changes to TUI/GPUI (config provided via JSON).
*   **Hardware Integration:** No real-time hardware handshakes.
```