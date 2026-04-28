# SOTF — Project Instructions

Sound of the Future: a Rust audio DSP workspace covering AutoEQ optimization,
a native multi-threaded audio engine, plugins, and macOS-specific tooling.

---

## Workspace layout

Cargo workspace, primary language Rust (Python for visualization in `./venv`).

```
crates/
  engine/                 native audio engine (4 threads: decode, process, playback, manager)
  player/                 shared playback API and business logic
  player-tui/             production-quality terminal player
  player-gpui/            experimental GPUI player
  player-midi/            MIDI integration
  sotf-plugins/           plugin implementations (EQ, compressor, upmixer, analyzers, etc.)
  plugins-ffi/, plugins-au/  Audio Unit FFI + Swift wrapper
  autoeq/                 EQ optimization CLI (autoeq, roomeq, benchmarks)
  autoeq-cea2034/         CEA2034 / Spinorama metrics
  math-de/                Differential Evolution (SciPy fork) + NLopt + metaheuristics
  math-iir/               IIR filters and parametric EQ (Biquad)
  math-rir/               Room Impulse Response analysis
  math-bem/               Boundary Element Method (experimental)
  math-testfunctions/, math-convexhull3d/
  sotf-macos-hal/         CoreAudio HAL driver
  sotf-macos-configbar/   menubar config app
```

Workspace runner: `just` (install with `cargo install just`). See `justfile`.

---

## Build / Test

- `cargo check --workspace` and `cargo clippy --workspace` after every change.
- `cargo test --workspace` (or `cargo test -p <exact-package-name>`) before reporting a task complete. Compilation alone is not proof.
- `plugins` and `engine` crates: build with `--no-default-features` if `hdf5-metno-sys` is in the dep tree.
- Python: use `./venv/bin/python` and `./venv/bin/pyright`. Never the system Python.
- `just test` runs Rust + TypeScript. `just qa` runs optimization benchmarks.

### Build configuration notes

- Debug on macOS: keep `opt-level=0`. Aggressive debug-mode optimization causes segfaults in cpal/CoreAudio device enumeration.
- Release: LTO + single codegen-unit.
- Panic strategy: `unwind` (so tests run).
- macOS cannot produce fully static binaries (system frameworks). Linux musl can. Windows MSVC uses static CRT.
- BLAS: Accelerate (macOS), OpenBLAS (Linux + Windows ARM), Intel MKL (Windows x64).

---

## Code review and PR rules

- **`engine/` and `sotf-plugins/`**: every change is a dedicated PR and must run through `pr-review-toolkit:code-reviewer` before merge.
- Other crates: use `/code-review`.
- Code reviews must include: (1) correctness bugs with test cases, (2) edge cases in algorithm logic, (3) missing error handling, (4) concrete fix suggestions. Surface-level reviews will be rejected.

---

## Specialized agents

- `audio-optimizer` — DSP code, FFT, SIMD, hot paths in `sotf-plugins/`, `engine/`, `math-iir/`, `autoeq/`.
- `pr-review-toolkit:code-reviewer` — before committing to `engine/` or `sotf-plugins/`.
- `psychoacoustics-researcher` — EQ algorithms, loudness compensation, upmixer spatial decisions, target curves.
- `feature-dev:code-architect` — multi-crate refactors or features touching >3 files.

## Skills

- `/review-pr` — PRs touching `engine/` or `sotf-plugins/`.
- `/code-review` — other crates.
- `/feature-dev` — non-trivial features needing guided architecture.

---

## Code rules

- **Never use `unsafe` without asking.**
- Business logic goes in `player/src` (or the relevant `math-*` / `autoeq` crate). App crates (`player-tui`, `player-gpui`) are thin UI wrappers — never duplicate logic across them.
- Before editing a Rust file, verify it is actively compiled (not commented out, not behind a disabled feature flag, not dead code). Check `mod` declarations and Cargo features.
- Read `GPUI.md` before any `player-gpui/` work.
- Verify compilation (`cargo check`) before claiming a task done. The Edit tool reports success when bytes hit disk, not when code compiles.

### Conventions

- Binaries: `snake_case` (`sotf_player_cli`, `autoeq_download_speakers`).
- Crates: `kebab-case` (`math-iir`, `sotf-plugins`).
- Module files: `snake_case` (`signal_analysis.rs`, `plugin_eq.rs`).

---

## Code research and editing — tool selection

This project has tokensave initialised (731K-node graph). It provides semantic
search, faster and more precise than grep on this codebase.

- **Search / explore**: `tokensave_context` first (10-call budget per task), or targeted tools (`tokensave_search`, `tokensave_callers`, `tokensave_callees`, `tokensave_impact`, `tokensave_files`, `tokensave_node`, `tokensave_type_hierarchy`, `tokensave_test_map`). Other tokensave tools have no per-session cap.
- **Native `Grep` / `Read` / `Glob`** only for raw text (error strings, log lines, TODOs, JSON/YAML/MD content), known file paths, or filename patterns.
- **Edits**: single known location → `Edit`. Multi-file or symbol-aware → `tokensave_multi_str_replace` / `tokensave_ast_grep_rewrite`. Run `tokensave_impact` or `tokensave_rename_preview` before any rename or signature change.
- **Never** spawn `Agent(Explore)` for code research while tokensave is available. Use tokensave instead — semantic search is the point.

Detailed rules: see auto-memory `feedback_tokensave_tool_selection.md`.

---

## Domain knowledge

### Room EQ / AutoEQ

- Filters must be placed within measurement-data frequency bounds. Verify optimizer frequency ranges against actual data bounds.
- Passband detection uses **relative-to-peak** thresholds, not absolute dB values.
- Core filter type is `autoeq_iir::Biquad`. Filter types: Peak, Lowshelf, Highshelf, Lowpass, Highpass, Bandpass, Notch.

### Audio engine

- 4-thread architecture: Decoder → Processing → Playback → Manager. See `engine/src/engine/`.
- Per-frame allocations in audio callbacks cause crackling (~21ms frame period). Pre-allocate in `build()`, reuse via `Option::take()` in `process()`.
- STFT plugins must return `context.num_frames` (not the actual draining count) to prevent ring-buffer underrun. Output buffer is pre-zeroed.
- Cache per-frame checks (e.g. `has_variable_frame_plugin`) during `build()`. Locking plugin mutexes per frame causes audio-thread jitter.
- `NodeBuffer::clear()` resets `actual_len` only; it does not zero data. `read()` returns the full buffer when `actual_len == 0` (stale-data risk; mitigated by host safety pad).
- Output clipping (`sample.clamp(-1.0, 1.0)`) goes in the cpal callback to prevent saturation.

### Plugin instantiation

```rust
PluginConfig {
    plugin_type: "EQ".into(),
    parameters: json!({
        "filters": [{"filter_type": "peak", "frequency": 1000.0, "q": 1.5, "gain_db": 3.0}]
    })
}
```

Channel count can change between plugins (e.g. upmixer 2ch → 5ch).

### DSP plugin parameter registration

Plugin parameters need wiring in **three** places: `rebuild_cached_parameters`, `set_parameter`, `get_parameter`. Missing `cached_parameters` causes silent rejection.

---

## Debugging

- Audio bugs (crackling, saturation, speed): trace the full signal chain. Surface fixes mask root causes. Check sample-rate mismatches, hot-path allocation, plugin propagation, normalization. Prefer bounded/clamped over unbounded normalization.
- clap CLI errors: check for **duplicate field names across flattened structs** (e.g. shared `enabled`) before chasing default-value theories.
- A failed first fix means re-analyze the root cause — do not iterate on the same wrong model. If the user says "step back" or "we're going in circles," stop and propose something fundamentally different.
- Bug reports without error output: ask for raw logs before guessing.
- Before implementing a fix against a format/spec, read the spec.

---

## API integration (spinorama.org)

```
GET http://api.spinorama.org/v1/speakers
GET http://api.spinorama.org/v1/speakers/{speaker}/versions
GET http://api.spinorama.org/v1/speakers/{speaker}/versions/{version}/measurements
```
