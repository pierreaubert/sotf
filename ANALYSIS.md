# SotF Dependency Analysis

Audit of every external crate declared in workspace `Cargo.toml` `[workspace.dependencies]`
plus the seven vendored crates under `crates/3rdparties/`. For each entry the table records
how many workspace members import it, how many `.rs` files reference it, the workspace-level
feature flags, and a recommendation: **Keep**, **Fork-trim**, **Rewrite/Remove**.

This is a surface-level audit. It does not measure binary size, compile time, or
transitive dependencies. Symbol-level usage is out of scope; the "Use sites" column is
a coarse signal, not a contract.

---

## Summary

- **119 external crates** declared. **7 vendored crates** under `crates/3rdparties/`.
- **Keep: 88** — foundational, broadly used, or stable thin wrappers we have no reason to touch.
- **Fork-trim: 18** — narrow usage of a heavy crate; vendoring + deleting unused modules wins compile time and audit surface.
- **Rewrite/Remove: 13** — declared but unused (dead deps), or trivially small (single helper) and easy to inline.
- **Top action items** (see end-of-doc shortlist):
  1. Delete 11 declared-but-unused crates (`console_error_panic_hook`, `spec_math`, `wasm-bindgen` family, `web-sys`, 5 unused `objc2-*` framework crates, `metaflac`).
  2. Pick a single async runtime: `tokio`, `smol`, `pollster`, `async-task`, `futures` all coexist.
  3. Migrate off legacy `objc 0.2` + `cocoa 0.26` to `objc2*`; both currently coexist.
  4. Trim `symphonia-*` codec spread (12 crates) to the formats SotF actually plays.
  5. Document the cost of pinning `gpui` / `wgpu` / `gpui_*` to Zed forks; plan the un-fork trigger.

---

## Methodology

### Per-crate metrics

- **Importers** = number of `crates/**/Cargo.toml` files that declare the crate
  (`grep -rlE "^<crate> *=" --include=Cargo.toml crates/`).
- **Use sites** = number of `*.rs` files that reference the crate by Rust name
  (`-` → `_`), matched as a path prefix (`crate::`).
- **Features** = literal value from root `Cargo.toml` `[workspace.dependencies]`.

The use-site count under-reports crates accessed by free-function syntax (e.g. `which("ffmpeg")`
without `which::`); this is acceptable noise — the signal is order-of-magnitude.

### Bucket heuristics

- **Keep** when (a) ≥3 importers, OR (b) foundational (`serde`, `tokio`, `anyhow`,
  `log`, `thiserror`, `parking_lot`, `num-traits`, `chrono`, `clap`, …), OR (c) thin
  wrapper around a system API with active upstream.
- **Fork-trim** when crate is large or pulls heavy transitive cone, has 1–2 importers, and we
  use a narrow API. Goal: copy the crate into `crates/3rdparties/`, delete unused modules
  + features, drop transitive cost.
- **Rewrite/Remove** when (a) declared but no use sites (dead dep), OR (b) crate is a
  ≤200-line helper we already wrap thinly, OR (c) upstream is unmaintained AND the surface
  is small enough that ownership cost is lower than fork cost.

---

## External Dependencies Table

Sorted: bucket → importers (desc).

### Keep

| Crate | Importers | Use sites | Workspace features | Note |
|---|---:|---:|---|---|
| `serde` | 67 | 168 | `derive`, `rc` | Foundational. Used everywhere. |
| `serde_json` | 62 | 168 | default | Foundational pair with serde. |
| `log` | 44 | 225 | default | Logging facade. Foundational. |
| `gpui` | 19 | 524 | git tag `v1.0.0` (Zed fork) | Core UI framework for `app-gpui`. Pinning is a known cost — see cross-cutting. |
| `tokio` | 19 | 42 | `rt-multi-thread`, `fs`, `macros`, `io-util`, `process`, `signal` | Async runtime. Keep — but consolidate (see cross-cutting). |
| `rustfft` | 21 | 49 | default | DSP staple. |
| `env_logger` | 16 | 24 | default | Standard logger backend. |
| `hound` | 16 | 43 | default | WAV I/O — used across engine, plugins, autoeq. |
| `thiserror` | 16 | 13 | default | Error-derive. Foundational. |
| `ndarray` | 14 | 269 | `rayon`, `serde`, `default-features=false` | Heavy linear-algebra usage in autoeq + math-*. Keep. |
| `realfft` | 14 | 13 | default | DSP staple alongside `rustfft`. |
| `clap` | 13 | 42 | `derive` | CLI parser. Foundational across binaries. |
| `anyhow` | 12 | 38 | default | Error propagation. Foundational. |
| `parking_lot` | 12 | 21 | `deadlock_detection` | Mutex/RwLock used hot-path; deadlock detection is valuable. |
| `chrono` | 11 | 18 | `serde` | Timestamps and durations. |
| `criterion` | 11 | 12 | default | Bench framework (dev-dep). |
| `tempfile` | 12 | 58 | default | Test scaffolding (dev-dep). |
| `symphonia` | 9 | 6 | `default-features=false` | Decoder umbrella; see codec spread under cross-cutting. |
| `num-complex` | 9 | 69 | default | Math foundation for FFT/DSP. |
| `uuid` | 9 | 7 | `v4`, `v5`, `serde` | Identity tokens; small + stable. |
| `rand` | 8 | 31 | default | RNG. Foundational. |
| `directories` | 8 | 4 | default | XDG paths. Stable, small. |
| `audioadapter-buffers` | 7 | 6 | default | Audio buffer adapters; specific to DSP pipeline. |
| `libc` | 7 | 43 | default | System bindings; stable. |
| `rayon` | 7 | 13 | default | Data parallelism. |
| `reqwest` | 7 | 9 | `json` | HTTP client (spinorama API + downloads). Heavy but standard. |
| `walkdir` | 7 | 3 | default | Library scanning. |
| `arc-swap` | 6 | 7 | default | Lock-free Arc swap; engine + plugins. |
| `image` | 6 | 9 | default | Album art + QR rendering. |
| `rusqlite` | 6 | 11 | `bundled` | Library DB. Bundled SQLite — see cross-cutting. |
| `cpal` | 5 | 21 | default | Cross-platform audio I/O — non-trivial to replace. |
| `rtrb` | 5 | 9 | default | Lock-free ring buffer (audio thread). |
| `nalgebra` | 4 | 9 | `default-features=false`, `std` | Linear algebra in math-*. |
| `approx` | 4 | 9 | default | Float comparison in tests. |
| `ctrlc` | 4 | 1 | default | Signal handling for daemons. |
| `signal-hook` | 4 | 2 | default | Same family — could consolidate with `ctrlc` (one or the other). |
| `urlencoding` | 4 | 5 | default | Tiny but used in 4 spots — keep (replacing with manual encoding is more code). |
| `id3` | 4 | 0* | default | Active behind player metadata path; *use-site grep undercounts (no `id3::` prefix in some files). |
| `bincode` | 3 | 2 | `serde` | Cache serialization. Stable. |
| `block2` | 3 | 3 | default | objc2 block bindings. |
| `core-foundation` | 3 | 12 | default | macOS interop. |
| `notify` | 3 | 1 | default | File watcher. |
| `num_cpus` | 3 | 5 | default | Parallelism sizing. |
| `oxiblas-ndarray` | 3 | 4 | default | BLAS acceleration for ndarray. |
| `plotly` | 3 | 9 | default | See Fork-trim discussion below — kept for now since 3 importers. |
| `plotly_static` | 3 | 1 | `chromedriver`, `webdriver_download` | Image export of plots; heavy chrome dep — see Fork-trim. |
| `rust-embed` | 3 | 4 | default | Static asset bundling. |
| `serde_yaml` | 3 | 2 | default | Config loading. |
| `serial_test` | 3 | 8 | default | Test serialization (dev-dep). |
| `smol` | 3 | 9 | default | See cross-cutting on async runtime duplication. Keep until consolidation decided. |
| `wgpu` | 3 | 6 | git fork | GPU backend; pinned to Zed fork. |
| `bytemuck` | 2 | 11 | `derive` | Cast helpers; foundational. |
| `cbindgen` | 2 | 2 | default | Build-time C header gen for FFI. |
| `crossterm` | 2 | 20 | default | TUI input. |
| `delaunator` | 2 | 4 | default | Triangulation. Stable, small upstream. |
| `dirs` | 2 | 5 | default | Coexists with `directories` — could consolidate but cheap. |
| `glam` | 2 | 12 | default | Math types for GPU. |
| `gpui_linux` | 2 | 3 | git tag | GPUI backend. |
| `gpui_macos` | 2 | 3 | git tag, vendored | See vendored section. |
| `gpui_wgpu` | 2 | 2 | git tag | GPUI backend. |
| `gpui_windows` | 2 | 3 | git tag | GPUI backend. |
| `insta` | 2 | 1 | `json` | Snapshot tests (dev-dep). |
| `objc2` | 2 | 2 | default | Modern Apple bindings. Will grow. |
| `objc2-app-kit` | 2 | 2 | default | Modern Apple bindings. |
| `objc2-foundation` | 2 | 2 | default | Modern Apple bindings. |
| `ratatui` | 2 | 12 | default | TUI — used by `app-tui`. |
| `strsim` | 2 | 3 | default | Fuzzy match. |
| `unicode-normalization` | 2 | 1 | default | Library scan (NFC). |
| `objc2-core-foundation` | 1 | 1 | default | Modern Apple bindings. |
| `pollster` | 1 | 3 | default | Block-on for `wgpu`. See cross-cutting. |
| `proc-macro2` | 1 | 1 | default | Macro foundation. |
| `quote` | 1 | 1 | default | Macro foundation. |
| `syn` | 1 | 1 | `full`, `parsing`, `extra-traits` | Macro foundation. |
| `crossbeam` | 1 | 1 | default | Channel/queue primitives. |
| `csv` | 1 | 3 | default | Bench data ingest. |
| `metal` | 1 | 3 | default | macOS GPU backend. |
| `midir` | 1 | 3 | default | MIDI I/O. |
| `mimalloc` | 1 | 1 | default | Allocator. Single import is the binary entry point — that's correct. |
| `nnnoiseless` | 1 | 4 | default + vendored patch | Noise suppression; see vendored section. |
| `ratatui-image` | 1 | 3 | `serde`, `crossterm`, `image-defaults` | TUI album art. |
| `regex` | 1 | 2 | default | Library matching. |
| `rfd` | 1 | 9 | default | Native file dialogs. |
| `rubato` | 10 | 6 | default | Resampling. |
| `byteorder` | 1 | 8 | default | Used in IAMF / drivers. |
| `num-traits` | 1 | 4 | default | Generic-numeric helpers. |
| `which` | 3 | 0* | default | *Used as `which("ffmpeg")` free-function syntax — grep undercount. |

### Fork-trim

| Crate | Importers | Use sites | Features | Why fork-trim |
|---|---:|---:|---|---|
| `plotly` | 3 | 9 | default | Pulls a large viz/HTML template tree; SotF needs only Scatter + Layout. Vendor + delete the rest. |
| `plotly_static` | 3 | 1 | `chromedriver`, `webdriver_download` | Downloads chrome at build/run time. Replace with a 1-shot static-image renderer or remove entirely if PNG export isn't on the critical path. |
| `nokhwa` | 1 | 1 | `default-features=false` | 50k+ LoC camera framework used solely for QR-code scanning (one import in `app-gpui`). Trim to the macOS AVFoundation backend, drop everything else. |
| `rqrr` | 1 | 1 | default | Fine as-is in size, but it's the only reason we keep `nokhwa`. Decision is coupled. |
| `qrcode` | 1 | 1 | default | Same family — single use site. Vendor with `nokhwa`/`rqrr` as a "QR" feature module. |
| `ort` | 2 | 1 | default | ONNX runtime, large native dep. Used by a single denoiser plugin. Vendor + pin to CPU-only build to drop CUDA/TensorRT bindings. |
| `nih_plug` | (transitively via `plugins-nih`) | n/a | git | Plugin framework; we use the host side narrowly. Vendor and trim non-essential targets (LV2/VST3 surfaces we don't ship). |
| `symphonia-bundle-flac` | 8 | 4 | default | Codec — keep. |
| `symphonia-bundle-mp3` | 7 | 3 | default | Codec — keep. |
| `symphonia-codec-aac` | 7 | 2 | default | Confirm we ship AAC; otherwise drop. |
| `symphonia-codec-alac` | 3 | 1 | default | Confirm ALAC need; otherwise drop. |
| `symphonia-codec-pcm` | 5 | 4 | default | Keep. |
| `symphonia-codec-vorbis` | 3 | 1 | default | Confirm Vorbis need; otherwise drop. |
| `symphonia-format-isomp4` | 7 | 2 | default | Container — keep with AAC. |
| `symphonia-format-ogg` | 7 | 2 | default | Container — keep with Vorbis. |
| `symphonia-format-riff` | 9 | 5 | default | WAV container — keep. |
| `symphonia-metadata` | 7 | 1 | default | Used narrowly; vendor + keep only the tag readers we surface. |
| `symphonia-core` | 10 | 4 | default | Keep — but the family-wide cleanup is one PR. |
| `metaheuristics-nature` | 1 | 0* | default | One importer (`autoeq`), narrow algorithm subset. Vendor + delete unused metaheuristics. *Use-site grep undercount. |

### Rewrite / Remove

These are declared but have no `*.rs` references, OR are tiny enough to inline.

| Crate | Importers | Use sites | Why |
|---|---:|---:|---|
| `console_error_panic_hook` | 0 | 0 | Declared in workspace, no importer, no use site. **Delete.** WASM-only utility — we don't ship WASM. |
| `spec_math` | 0 | 0 | Declared, never used. **Delete.** |
| `wasm-bindgen` | 0 | 0 | Declared, never used. **Delete.** No WASM target ships. |
| `wasm-bindgen-futures` | 0 | 0 | Same. **Delete.** |
| `wasm-bindgen-rayon` | 0 | 0 | Same. **Delete.** |
| `web-sys` | 0 | 0 | Same. **Delete.** Frees 6 large feature flags. |
| `objc2-vision` | 0 | 0 | Declared, never used. **Delete.** |
| `objc2-av-foundation` | 0 | 0 | Same. **Delete.** |
| `objc2-core-image` | 0 | 0 | Same. **Delete.** |
| `objc2-core-media` | 0 | 0 | Same. **Delete.** |
| `objc2-core-video` | 0 | 0 | Same. **Delete.** |
| `metaflac` | 4 | 0 | 4 importers declare it, no Rust file uses it. Probably stale after a refactor — verify, then **delete**. |
| `lazy_static` | 0 | 0 in our code (only inside vendored `zed-font-kit`) | **Delete from workspace.** Use `std::sync::OnceLock` if anything ever needed it. |
| `dispatch` | 0 | 0 in our code | **Delete from workspace.** Modern code goes through `objc2`/`block2`. |
| `md-5` | 1 | 0\* | Single use site is `md5::` not `md_5::` — confirm aliasing then either rewrite (md5 is 50 lines) or **delete** in favor of `sha2` if we already pull it. |
| `base64` | 2 | 1 | Two importers, one use site. ~200 lines. **Inline-rewrite** as a trimmed helper if we want to drop the dep, otherwise keep. Lean toward keep. |
| `strsim` | 2 | 3 | Tiny crate. Borderline — keep unless we want zero deps for fuzzy-match. |
| `build_html` | 1 | 2 | Single importer; HTML building is template-replace. Could **rewrite** in 50 lines. |
| `fontdue` | 1 | 2 | Single importer. Heavy font crate. If usage is bitmap-only, vendor; if text-shaping not needed, **rewrite** path is open. Borderline fork-trim/rewrite — depends on use. |

---

## Vendored Crates (`crates/3rdparties/`)

| Crate | Size | Patch reason | Recommendation |
|---|---:|---|---|
| `psm` | 216 KB | Assembly files use ELF directives instead of Mach-O on tvOS/watchOS/visionOS. | **Keep vendored** until upstream merges Tier-3 Apple support, or until we drop tvOS/visionOS targets. Track an upstream PR. |
| `mach2` | 256 KB | `compile_error!` gate doesn't include `tvos`. | **Keep vendored**. Same Tier-3 trigger as `psm`. One-line patch — try upstreaming. |
| `coreaudio-rs` | 228 KB | `cfg(target_os = "ios")` doesn't include `tvos`. | **Keep vendored**. One-line cfg fix — upstreamable. |
| `zed-font-kit` | 33 MB | Pulls fontconfig/freetype on tvOS instead of CoreText; broken `core-text` imports on git rev. | **Keep vendored**, but **trim**: largest vendored crate by 10×. We use a narrow path (CoreText loader). Delete the FreeType / fontconfig back-ends entirely since we never build with them. Significant footprint win. |
| `gpui_macos` | 484 KB | Strips private CGS symbols (`CGSMainConnectionID`, `CGSSetWindowBackgroundBlurRadius`) that fail Mac App Store validation. | **Keep vendored** — patch is load-bearing for MAS shipping. Document in `crates/3rdparties/README` so the next gpui rev keeps the patch alive. |
| `nnnoiseless` | 692 KB | Local patch. | Single importer (`plugins-denoiser`). **Trim**: check what we actually use vs. the example/training paths in upstream and delete training data + binaries. |
| `sofa-reader` | 140 KB | Internal — not patched, just hosted in-tree. | **Keep**. It's effectively a first-party crate with a generic name. Consider moving out of `3rdparties/` since it's not a fork. |

---

## Cross-cutting Issues

### 1. Async runtime duplication

The workspace simultaneously declares `tokio`, `smol`, `pollster`, `async-task` (transitively),
and `futures` (transitively). The root `Cargo.toml` itself has the comment:

> `# ideally we would use only threads or only tokio`

**Recommendation**: pick `tokio` (19 importers, broadest features already enabled), drop
`smol` (3 importers — migrate them), keep `pollster` only inside the `wgpu`-using crates
where it's the canonical block-on. The audio path remains thread-based; nothing here changes
that.

### 2. Apple bindings — legacy vs modern

| Family | Importers | Use sites | Status |
|---|---:|---:|---|
| `objc` 0.2 + `cocoa` 0.26 | 4 + 1 | 24 + 11 | Legacy, unmaintained. |
| `objc2` + `objc2-foundation` + `objc2-app-kit` | 2 each | 2 each | Modern, active. |
| `objc2-vision`/`av-foundation`/`core-image`/`core-media`/`core-video` | 0 each | 0 each | Declared, unused. **Delete.** |

**Recommendation**: stop adding `objc 0.2` / `cocoa 0.26` call sites; migrate the 35 existing
ones to `objc2` over time. Delete the unused `objc2-*` framework crates now (free win).

### 3. GPU stack pinning

`gpui`, `gpui_macos`, `gpui_linux`, `gpui_windows`, `gpui_wgpu`, and `wgpu` are all pinned
to **git tags / branches in Zed forks** (`zed-industries/zed.git tag v1.0.0`,
`zed-industries/wgpu.git branch v29`). This:

- Locks us out of `cargo update` for these crates.
- Couples our update cadence to Zed's release cadence.
- Forces local patches (see `gpui_macos` vendoring).

**Recommendation**: document the un-fork trigger explicitly. The realistic path is "wait for
upstream gpui to stabilize"; in the meantime treat the pin as deliberate. Don't try to
fork-trim — the surface is the framework itself.

### 4. Symphonia codec spread

12 separate `symphonia-*` crates declared. SotF actually plays:

- **WAV** (riff + pcm) — confirmed in tests.
- **FLAC** (bundle-flac) — confirmed.
- **MP3** (bundle-mp3) — confirmed.

The presence of **AAC**, **ALAC**, **Vorbis**, **OGG**, **isomp4** is plausible but unverified
in this audit. If user-facing format support doesn't include them, dropping 6 codec/format
crates is a clean compile-time win. If it does, the umbrella `symphonia` crate with feature
flags can replace the explicit per-codec deps for some of them.

**Recommendation**: list shipped formats in `app-gpui` / `app-tui` UI, drop unused symphonia-*
crates accordingly.

### 5. `rusqlite` bundled

`rusqlite = { version = "0.38", features = ["bundled"] }` — bundles SQLite source and compiles
it. Pros: no system SQLite dependency, deterministic builds. Cons: ~600 KB of C compilation
each clean build, slows CI. **Recommendation**: keep `bundled` for distribution builds (esp.
macOS App Store + Windows MSI), expose a `system-sqlite` feature for local dev to cut clean
build times.

### 6. Foundational duplications worth de-duping

- `directories` (8 importers) and `dirs` (2 importers) — same purpose. Migrate `dirs` →
  `directories`, drop `dirs`.
- `ctrlc` (4) and `signal-hook` (4) — pick one. `ctrlc` is simpler and matches how we use it.

---

## Action shortlist (highest leverage first)

1. **Delete 11 unused workspace deps.** One PR, mechanical edit to `Cargo.toml`:
   `console_error_panic_hook`, `spec_math`, `wasm-bindgen`, `wasm-bindgen-futures`,
   `wasm-bindgen-rayon`, `web-sys`, `objc2-vision`, `objc2-av-foundation`, `objc2-core-image`,
   `objc2-core-media`, `objc2-core-video`. Verify with `cargo check --workspace`. Free win.

2. **Delete `metaflac`, `lazy_static`, `dispatch`** from workspace deps after confirming
   no use sites. Same PR as #1 ideally.

3. **Drop unused symphonia codecs.** Audit which formats ship, then strip `symphonia-codec-aac`,
   `symphonia-codec-alac`, `symphonia-codec-vorbis`, `symphonia-format-isomp4`,
   `symphonia-format-ogg` if not in the supported-format list. Each one removed is a transitive
   savings.

4. **Trim `zed-font-kit` vendored copy.** Largest vendored crate (33 MB). Delete the
   FreeType + fontconfig back-ends and their assets — we only build the CoreText path.
   Significant repo-size win.

5. **Pick one async runtime.** Migrate the 3 `smol` importers to `tokio`. Keep `pollster`
   only inside `wgpu`-adjacent code.

6. **Vendor + trim `nokhwa`.** One use site (QR scanning); the crate ships drivers for every
   capture API. Keep the macOS AVFoundation back-end, delete the rest. Couples with `rqrr`
   + `qrcode` into a single internal "qr" module.

7. **Vendor + trim `ort`.** Single denoiser plugin uses it. Strip non-CPU execution providers
   (CUDA, TensorRT, DirectML); huge transitive savings.

8. **Vendor + trim `plotly` / `plotly_static`.** We use Scatter + Layout. The chromedriver
   download path in `plotly_static` should become opt-in or be replaced.

9. **Plan `objc 0.2` / `cocoa 0.26` retirement.** 35 use sites total. Track in a follow-up
   issue; migrate file-by-file to `objc2` as the surrounding code is touched. Don't do a
   big-bang rewrite.

10. **Document GPUI/WGPU fork pinning.** Add a section to `CLAUDE.md` explaining when
    each pin can be dropped (when upstream gpui reaches X, when MAS validation no longer
    rejects CGS symbols, etc.). Future devs need to know what's deliberate vs. legacy.

---

## How to re-run this audit

```bash
# Inventory of declared external deps
awk '/^\[workspace.dependencies\]/{f=1;next} /^\[/{f=0} f && /^[a-z][a-zA-Z0-9_-]+ ?=/{
  if ($0 !~ /path *= *"crates\//) {gsub(/ /,"",$1); print $1}
}' Cargo.toml | sort -u

# Per-crate metrics
for d in $(...above...); do
  rs=$(echo "$d" | tr '-' '_')
  imp=$(grep -rlE "^${d} *=" --include=Cargo.toml crates/ | wc -l)
  uses=$(grep -rlE "(^|[^a-zA-Z0-9_])${rs}::" --include='*.rs' crates/ | wc -l)
  printf "%-32s %4s %4s\n" "$d" "$imp" "$uses"
done
```

The use-site count under-reports free-function-style usage (e.g. `which("...")`), so any
zero-count crate should be confirmed by a broader grep before deletion.
