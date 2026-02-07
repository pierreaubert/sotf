# SOTF (Sound of the Future) Project Context

## Project Overview

SOTF is a comprehensive Rust workspace for audio optimization, playback, and acoustic simulation. It integrates:
1.  **AutoEQ:** Tools to optimize frequency response for headphones and speakers using Spinorama data.
2.  **Audio Engine:** A native, multi-threaded audio processing engine with a modular plugin system (EQ, Upmixing, etc.).
3.  **Simulation:** High-performance BEM (Boundary Element Method) and FEM libraries for acoustic modeling.
4.  **Players:** TUI and experimental GUI (GPUI) music players utilizing the native engine.

## Architecture

### 1. Audio Engine (`engine`)
A native Rust engine replacing external dependencies like CamillaDSP.
-   **Threading Model:**
    -   `Decoder`: Reads/decodes files (Symphonia) to PCM.
    -   `Processing`: Applies plugin chain (EQ, Compressor, etc.).
    -   `Playback`: Outputs to hardware (cpal).
    -   `Manager`: Coordinates threads and config.
-   **Plugins (`plugins`):** JSON-configurable DSP modules (PEQ, Convolution, Upmixer, Loudness). Hot-reloadable.
-   **Decoding:** Supports FLAC, MP3, AAC, ALAC, Vorbis, WAV via `symphonia`.

### 2. AutoEQ (`autoeq`)
CLI tools for frequency response optimization.
-   **Workflow:** Measurement Input (Spinorama/CSV) -> Target Curve -> Optimization (DE/NLopt) -> PEQ Filters.
-   **Solvers:** Differential Evolution (`math-de`), Genetic Algorithms, Local search (Cobyla).

### 3. Math Libraries
-   **`math-bem`:** BEM solver for acoustic scattering. Features FMM (Fast Multipole Method) and Burton-Miller formulation.
-   **`math-fem`:** Finite Element Method solver.
-   **`math-solvers`:** Linear algebra solvers (GMRES, ILU) with parallel execution.

### 4. Applications
-   **`player/app-tui`:** Production-ready terminal music player. Supports library scanning and ReplayGain.
-   **`player/app-gpui`:** Experimental GUI player using Zed's GPUI framework.
-   **`sotf-macos-hal`:** macOS CoreAudio HAL driver for system-wide audio processing.

## Build and Run

The project uses `just` as a command runner.

### Prerequisites
-   **Rust:** `rustup` (stable).
-   **Just:** `cargo install just`.
-   **System Dependencies:**
    -   **macOS:** Xcode (Accelerate framework used for BLAS).
        - **Note:** On macOS there is a bug with hdf5. Use `HDF5_DIR=/opt/homebrew/Cellar/hdf5@1.10/1.10.11 cargo build` to compile.
    -   **Linux:** OpenBLAS (`libopenblas-dev`), ALSA (`libasound2-dev`).
    -   **Windows:** Intel MKL or OpenBLAS.

### Key Commands

| Command | Description |
| :--- | :--- |
| `just build` | Build all default members in release mode. |
| `just dev` | Build in debug mode. |
| `just test` | Run all tests (Rust + TypeScript). |
| `just qa` | Run Quality Assurance benchmarks (AutoEQ optimization tests). |
| `just demo` | Run various demos (UI kit, plotting, etc.). |
| `just prod-hal` | Build macOS HAL driver (Mac only). |

### Running Binaries

-   **TUI Player:** `cargo run --release --bin sotf_player_tui`
-   **AutoEQ:** `cargo run --release --bin autoeq -- [ARGS]`
-   **Room Sim:** `cargo run --release --bin roomeq`
-   **Audio Engine Demo:** `cargo run --release --example audio_engine_demo`

## Directory Structure

-   `autoeq/`: Core AutoEQ CLI and optimization logic.
-   `engine/`: Core audio processing (threads, I/O, DSP).
-   `plugins/`: DSP plugin implementations.
-   `player/`: Shared player logic and TUI/GUI apps.
-   `math-*/`: Mathematical libraries (BEM, FEM, DE, Solvers).
-   `gpui-*/`: UI libraries based on GPUI.
-   `sotf-macos-hal/`: macOS Audio Server Plug-in.
-   `builds/`: Cross-compilation configurations.

## Development Conventions

-   **Verification:** ALWAYS verify changes with `cargo check` or `cargo build`.
-   **Testing:** Use `just test` for general testing and `just qa` for optimization logic validation.
-   **Formatting:** `just fmt` (rustfmt).
-   **Cross-Compilation:** The project supports static binaries for Linux (musl) and Windows (static CRT). macOS binaries are universal (Intel/ARM).
-   **Panic Strategy:** `unwind` is used in release mode to support test execution.

## Interactive Session Guidelines

-   **Complex Tasks:** Use `codebase_investigator` to map dependencies or understand flow before refactoring.
-   **Planning:** Use `write_todos` for multi-step implementations.
-   **Verification:** Run relevant tests after changes. For audio engine changes, verify compilation of `engine`.
