# Tech Stack - SotF (Sound of the Future)

## Core Technologies
- **Language:** Rust (Edition 2024).
- **Runtime:** Tokio (multi-threaded) for async tasks and service management.
- **Serialization:** Serde (JSON, YAML) for configuration and data persistence.

## Audio Engine
- **Audio I/O:** `cpal` for cross-platform hardware abstraction.
- **Decoding:** `symphonia` for high-performance audio format support (FLAC, MP3, etc.).
- **DSP Engine:** Native, multi-threaded implementation with modular plugin support.
- **Platform Specifics:** macOS HAL (Hardware Abstraction Layer) driver for system-wide processing.

## User Interfaces
- **Graphical (GPUI):** Built using the Zed GPUI framework for high-performance, GPU-accelerated UI rendering.
- **Terminal (TUI):** Built using `ratatui` (via `crates/app-tui`) for an efficient keyboard-driven experience.

## Mathematics & Optimization
- **Linear Algebra:** `ndarray`, `blas-src`, `lapack-src` (with Accelerate/OpenBLAS/MKL depending on platform).
- **Optimization:** Custom solvers (`math-de`, `math-solvers`) and potential integration with NLopt for acoustic fitting.

## Target Platforms
- **macOS:** Primary development platform with HAL driver support.
- **Linux:** Supported via ALSA/Pipewire.
- **Windows:** Supported via WASAPI.
