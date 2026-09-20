# SOTF — Agent and Contributor Guide

Sound of the Future (SOTF) is a cross-platform Rust audio workspace containing
the playback engine, shared player logic, a large realtime DSP plugin family,
desktop and terminal applications, systemwide audio components, network
services, mobile targets, developer tooling, and release infrastructure.

This file is the repository-wide operating contract for automated agents and
contributors. A more specific `AGENTS.md` in a subdirectory takes precedence
for that subtree. Explicit task instructions take precedence over this file.

## What “done” means

A change is complete only when all of the following are true:

1. The requested behavior is implemented at the correct ownership layer.
2. Existing user changes and unrelated files are preserved.
3. The narrowest meaningful tests pass, followed by the required broader gates.
4. Realtime, platform, persistence, and compatibility risks have been checked
   when they apply.
5. The final report states what changed, what was run, and any residual risk or
   verification that could not be performed.

Compilation is not proof of behavior. A test that never exercises the changed
path is not proof either.

---

## Repository map and ownership

The workspace is defined by the root `Cargo.toml`. Package names, paths, and
features in Cargo manifests are authoritative; do not infer them from folder
names or old documentation.

### Core runtime and shared logic

- `crates/sotf-player/` — shared playback, library, EQ workflow, configuration,
  and application-facing business logic.
- `crates/sotf-media-controls/` — platform media-control integration.
- `../sotf-daw/` — DAW core workspace (engine, plugins, MIDI, IAMF, driver
  transport). This workspace depends on it via `../sotf-daw` path
  dependencies; never the reverse. `sotf-streaming` stays here and is wired
  into the engine through its optional `streaming`/`hls` features.

### Applications

- `crates/app-gpui/` — native GPUI desktop application (`sotf-desktop`).
- `crates/app-tui/` — terminal application (`sotf-tui`).
- `crates/app-cli/` — internal and utility command-line binaries.
- `crates/app-ios/`, `crates/app-tvos/` — Apple mobile/living-room targets.

Application crates should remain thin. Put reusable business logic in
`sotf-player`, engine behavior in `sotf-engine`, and DSP behavior in the
appropriate plugin or math dependency. Never duplicate business rules between
GPUI, TUI, CLI, iOS, and tvOS.

### Plugins and plugin hosting (moved to `../sotf-daw`)

- `../sotf-daw/crates/sotf-plugins/` — plugin registry, integration, stress
  tools, and plugin-level test suites.
- `../sotf-daw/crates/sotf-plugins/crates/sotf-host/` — internal and external
  plugin host.
- `../sotf-daw/crates/sotf-plugins/crates/sotf-plugin-*/` — focused DSP plugin
  crates.
- `../sotf-daw/crates/sotf-plugins/crates/plugins-ffi/` — C/Swift-facing FFI.
- `../sotf-daw/crates/sotf-plugins/crates/plugins-nih/` — CLAP/VST3 integration.
- `../sotf-daw/crates/sotf-plugins/crates/plugins-gpui/` — plugin UI integration.
- `../sotf-daw/crates/sotf-plugins/crates/plugins-bridge/`, `plugins-spatial/`,
  and `plugins-denoiser/` — specialized bridges and packaging layers.

### Systemwide audio (moved to `../sotf-systemwide`)

- `../sotf-systemwide/crates/daemon/` — systemwide audio daemon.
- `../sotf-systemwide/crates/driver-common/` — shared driver protocol/types.
- `../sotf-systemwide/crates/driver-hal/` — macOS HAL-side transport.
- `../sotf-systemwide/builds/`, `../sotf-systemwide/Justfile`, and
  `../sotf-systemwide/scripts/` — build, package, install, and test
  orchestration. `driver-common`/`driver-hal` live in `../sotf-daw` and are
  consumed here via `../sotf-daw` path dependencies.

### Services and network playback

- `crates/sotf-server/crates/sotf-streaming/` — streaming primitives.
- `sotf-services`, `sotf-service-spotify`, `sotf-service-tidal` — provider
  abstractions and integrations.
- `sotf-mpd`, `sotf-dlna`, `sotf-cast`, `sotf-federation`, and `sotf-tls` —
  protocols, discovery, federation, and transport security.

### Tests, tools, docs, and vendored code

- `crates/sotf-testkit/`, `crates/sotf-test-macros/` — shared test contracts.
- `crates/sotf-dev-api/`, `crates/sotf-dev-driver/` — deterministic UI and
  end-to-end driving infrastructure.
- `crates/sotf-tools/`, `crates/sotf-docs-gen/`, `scripts/` — maintenance and
  generation tools.
- `crates/3rdparties/` — vendored/forked upstream code. Treat changes here as
  upstream-maintenance work and keep them isolated.
- `data_generated/`, `dist/`, `target/`, `target-static/`, generated Xcode
  products, and generated docs are outputs, not normal edit targets.

### External sibling projects

AutoEQ and numerical/acoustic foundations are supplied from sibling projects
and git dependencies rather than local `autoeq/` or `math-*` workspace crates:

- `autoeq` — optimizer and RoomEQ implementation.
- `math-audio` — DSP, IIR/FIR, RIR, optimisation, geometry, and test functions.
- `gpui-toolkit` — GPUI components and design system.
- `sofa-reader` — SOFA/HRTF data support.
- `symphonia-add-ons` — additional codec and container support.

Before changing an external dependency, confirm whether the task belongs in
that sibling repository. Do not patch around an upstream ownership problem in
SOTF merely because SOTF is the current checkout.

---

## Sources of truth

Read the relevant source before changing behavior:

- `Cargo.toml` and crate manifests — membership, package names, features, and
  dependency ownership.
- `Justfile` plus imported `*.just` files — supported local/CI/release commands.
- `TESTING.md` — test tiers, contracts, coverage, and performance policy.
- `GPUI.md` — mandatory before modifying `crates/app-gpui/`.
- `TEST-CLIENTSERVER.md` — provider/live integration testing and credentials.
- `RELEASE-HOWTO.md` and release workflows — release and signing procedures.
- `.github/workflows/` and `.gitea/workflows/` — actual CI and release gates.
- The format/protocol specification — mandatory before fixing a standards
  compliance issue.

If documentation and executable configuration disagree, verify intent and fix
the stale documentation as part of the same scoped change when appropriate.
Never silently choose the easier interpretation.

For PDF, Word, PowerPoint, or Excel documents, use the configured MarkItDown
reader and consume its Markdown output instead of opening the binary directly.

---

## Required workflow

### 1. Protect the worktree

Before editing:

- Run `git status --short` and inspect any diff that overlaps the task.
- Assume all pre-existing changes belong to the user.
- Never reset, restore, reformat, or overwrite unrelated work.
- Keep generated artifacts, dependency updates, and drive-by cleanup out of the
  patch unless the task requires them.
- Do not use destructive commands or installation/uninstallation recipes
  without explicit authorization and an exact target.

### 2. Establish scope and ownership

- Identify the user-visible behavior, responsible crate, enabled feature path,
  and relevant tests before coding.
- For non-trivial implementation work, find or create the corresponding issue
  before writing a formal implementation plan or making broad changes.
- Use the architecture owner instead of duplicating logic in a convenient
  caller.
- For a rename or signature change, inspect callers and downstream impact first.
- If a request spans more than three files or several crates, establish the
  dependency direction and rollout order before editing.

Trivial documentation, typo, and narrowly scoped test corrections do not need a
new issue unless the user requests one.

### 3. Research efficiently

This repository has a TokenSave semantic graph. Use it before broad source
scans:

1. Run `tokensave_status` and note the active branch and last sync time.
2. Start unfamiliar code exploration with `tokensave_context`.
3. Use `tokensave_search` for known symbols, then `tokensave_callers`,
   `tokensave_callees`, `tokensave_impact`, `tokensave_node`,
   `tokensave_files`, `tokensave_type_hierarchy`, and `tokensave_test_map` as
   appropriate.
4. Use `tokensave_affected` to select tests after source edits.
5. Use `rg`/`rg --files` for raw strings, configuration, docs, logs, known
   paths, and filename discovery.

If the graph is stale and the task modifies this repository, sync it before
relying on structural results. If the task is read-only, disclose staleness and
fall back to source inspection rather than mutating the index. For a sibling
project, pass its absolute `graph_root`; use branch-specific tools for another
branch of the currently served project.

If a graph query cannot answer a structural question, inspect the active
database recorded in `.tokensave/branch-meta.json` before performing a broad
scan. Do not use exploration subagents as a substitute for the semantic graph.

When the local environment supplies RTK, prefix shell commands with `rtk` and
use `rtk proxy` only when exact unfiltered output is required. Do not assume RTK
exists in CI or in scripts committed to the repository.

### 4. Verify that code is active

Before editing a Rust source file, confirm that it is compiled in the relevant
configuration:

- Trace its `mod` declaration or crate root inclusion.
- Check `#[cfg(...)]` and Cargo feature gates.
- Check the target platform and binary/library target.
- Distinguish current code from commented-out, generated, example, benchmark,
  or abandoned code.

A correct change in an inactive file is still an incorrect fix.

### 5. Implement the smallest coherent change

- Follow existing naming, error, and module patterns in the owning crate.
- Prefer a root-cause fix with a focused regression test.
- Avoid speculative abstractions, broad renames, and unrelated formatting.
- Preserve public API, serialized schema, preset, and configuration
  compatibility unless a breaking change is explicitly requested.
- When compatibility must break, add migration/version handling and document
  it.

### 6. Review the patch before testing

Inspect `git diff --check`, the complete diff, and `git status --short`.
Specifically look for:

- accidental generated files or lockfile churn;
- debug output, temporary flags, hard-coded paths, and secrets;
- changed feature behavior or platform-specific compilation;
- missing error context, boundary validation, or regression tests;
- new allocations, locks, logging, or I/O on realtime paths.

### 7. Verify from narrow to broad

Start with the changed crate or test target, then run the completion gates for
the change class. Do not begin with a multi-hour workspace command when a
focused compile or unit test can expose the first failure faster.

If a required command cannot run because of platform, hardware, credentials,
or unavailable tools, report the exact command and reason. Never represent a
skipped gate as passing.

### 8. Handoff clearly

The completion summary must include:

- behavior changed and ownership location;
- tests/checks run and their results;
- tests not run and why;
- compatibility, realtime, platform, or migration risk that remains;
- any manual verification the user should perform.

Do not commit, push, open a PR, install artifacts, sign, notarize, or publish
unless the task or active workflow explicitly authorizes that action.

---

## Build, formatting, lint, and test policy

Use `just --list` to discover current recipes. The root `Justfile` imports
domain-specific recipes; prefer those recipes when they encode required
features, platform setup, or multiple packages.

### Baseline commands

For a Rust change, the normal minimum is:

```bash
cargo fmt --all -- --check
cargo test -p <exact-package-name> [focused-test-filter]
cargo check --workspace
cargo clippy --workspace
cargo test --workspace
git diff --check
```

Run `cargo fmt --all` to apply formatting when needed, then re-run the check.
Use the exact package name from its manifest; directory names and binary names
are not always package names.

Workspace-wide commands may require platform libraries or features unavailable
on the current host. In that case, run the strongest applicable `just` recipe
and targeted crate gates, then report the workspace limitation precisely.

### Deterministic PR tier

`TESTING.md` is authoritative. The standard deterministic aggregate is:

```bash
just test-pr
```

It covers:

```bash
just test-unit-core
just test-integration-engine
just test-integration-player
just test-device-fakes
just test-realtime-safety
```

Use fake-device tests for deterministic selection and orchestration. Tests that
require physical or virtual audio hardware belong in explicit engine QA,
systemwide, or portability tiers; never make them silently part of a unit tier.

### Plugins and host

Choose the smallest relevant gate, then broaden:

```bash
just plugins-check
just plugins-clippy
just plugins-test-lib
just plugins-test-integration
just plugins-test-host
just plugins-test-suites
just plugins-test-all
```

Plugin-format work may also require AU, CLAP, VST3, FFI, bridge, or cross-format
validation. A unit test of the DSP crate does not validate its format wrapper.

### GPUI

After the focused component/workflow test, use:

```bash
cargo check -p sotf-gpui --all-targets
cargo test -p sotf-gpui --lib --tests
cargo clippy -p sotf-gpui --all-targets -- -D warnings
./venv/bin/python scripts/check-design-tokens.py
```

See `GPUI.md` for focused test examples. If the change belongs in the sibling
`gpui-toolkit` repository, run that repository's `just qa-gpui-obvious` gate
there rather than copying toolkit behavior into SOTF.

### Performance, fuzzing, and broad validation

- `just perf-smoke` — release-mode realtime/deadline smoke gate.
- `just perf-regression <baseline.csv> <candidate.csv> [tolerance]` — controlled
  before/after comparison; keep sample rate, block size, chain, track count,
  and runner class identical.
- `just test-proptest` — high-case property tests.
- `just test-nightly` — broad nextest/property/coverage/performance tier.
- `just qa-engine` and its sample-rate/channel variants — engine QA matrix.
- `just qa-plugins` and plugin-specific QA recipes — algorithm/format QA.
- `just dev-driver-smoke`, `dev-driver-roomeq`, `dev-driver-tui`, and
  `dev-driver-full` — application workflows at increasing cost.

Do not run multi-hour QA, fuzzers, installers, signing, or release recipes by
default. Run them when the change's risk or the user request calls for them.

### Python and scripts

- Use `./venv/bin/python`, never the system Python.
- Use `./venv/bin/pyright` for Python type checking.
- Run `./venv/bin/python scripts/check_struct_sizes.py` after Rust structural
  changes.
- Do not commit `__pycache__`, notebook outputs, temporary plots, or regenerated
  data unless they are explicit deliverables.

### Documentation-only changes

Documentation-only work does not require compiling the Rust workspace unless it
changes executable examples, manifests, generated docs, or build instructions.
At minimum, validate referenced paths and commands, inspect Markdown structure,
run `git diff --check`, and confirm the diff contains no generated output.

---

## Rust and API rules

### Safety and structure

- Never introduce or expand `unsafe` without asking first. Explain the invariant,
  why a safe alternative is insufficient, and how it will be tested/reviewed.
- A struct may not exceed 30 fields without approval and a documented
  decomposition plan. Prefer focused sub-structs over state bags.
- Run `./venv/bin/python scripts/check_struct_sizes.py`; a new allowlist entry
  requires a concrete decomposition rationale and plan.
- Avoid new panics in library, decode, manager, host, and callback paths. Return
  contextual errors or use a proven invariant.
- Validate external inputs at the boundary: files, network payloads, plugin
  parameters, persisted configuration, channel layouts, sample rates, and frame
  counts are untrusted.
- Keep units explicit in names or types (`sample_rate_hz`, `latency_frames`,
  `gain_db`, seconds versus milliseconds).
- Preserve finite-number invariants. NaN and infinity must not leak into audio,
  optimizer state, meters, serialized output, or UI models.

### Compatibility

Treat the following as compatibility surfaces:

- public Rust APIs used by other workspace crates;
- serde field names/defaults and configuration migrations;
- plugin identifiers, parameter IDs/ranges/defaults, preset versions, and UI
  schemas;
- FFI layout, ownership, error, thread, and lifetime contracts;
- daemon/driver protocol messages and shared-memory formats;
- CLI flags, exit behavior, machine-readable output, and artifact names.

For new serialized fields, define backward-compatible defaults where possible.
For renamed fields or variants, add aliases/migrations and tests. Never make a
persisted-state break look like a refactor.

### Dependencies and features

- Add shared dependencies through `[workspace.dependencies]` when appropriate.
- Preserve `default-features = false` decisions unless the task explicitly
  changes the dependency surface.
- Test the feature combination actually used by the affected application or
  platform, not only the crate's default features.
- Do not update `Cargo.lock` or unrelated dependency versions incidentally.
- Treat changes under `crates/3rdparties/` and `[patch]` entries as isolated
  upstream forks with explicit rationale.

---

## Realtime audio and DSP invariants

Realtime correctness is a functional requirement, not an optional optimization.

### Callback and process paths

On realtime paths, do not introduce:

- heap allocation or container growth;
- mutex/RwLock acquisition or other potentially blocking synchronization;
- filesystem/network I/O, process spawning, sleeps, or waits;
- formatting/logging in the steady-state callback;
- unbounded loops or work proportional to uncontrolled external input.

Allocate and validate during `build()`, configuration, or control-thread phases.
Reuse buffers in `process()`; `Option::take()` is a common ownership pattern.
Cache per-block decisions during build/reconfiguration rather than locking every
plugin to rediscover them each frame.

Where allocation cannot be proven absent by inspection, add or run the realtime
allocation gate. Where latency/deadline behavior matters, test release builds;
debug timings are not representative.

### Block, channel, and latency contracts

- Process exactly the host/context frame contract unless a documented API says
  otherwise.
- STFT plugins return `context.num_frames`, not the transient number of frames
  drained internally; output is pre-zeroed to protect the ring-buffer contract.
- Support and test relevant variable block sizes, including zero/small/tail
  blocks where the API permits them.
- Channel count may change between plugins (for example stereo to multichannel).
  Never assume the graph's input and output layouts are identical.
- Validate buffer lengths before indexed access and keep interleaved/planar
  assumptions explicit.
- Report latency in the units required by the host, update it after relevant
  parameter changes, and test reset/flush/tail behavior.
- Bypass must preserve the documented channel/layout/latency contract.

### Numerical behavior

- Define behavior for silence, denormals, non-finite input, extreme parameters,
  unsupported sample rates, and degenerate filter designs.
- Smooth audible parameter changes where required; do not silently change the
  meaning of existing smoothing modes.
- Prefer bounded, stable normalization over gain derived from an unbounded or
  near-zero denominator.
- Add toleranced signal tests for amplitude, phase/latency, finite output, and
  channel independence where relevant.
- Benchmarks supplement correctness tests; they do not replace them.

### Engine and host-specific contracts

The engine architecture is Decoder → Processing → Playback, coordinated by the
Manager. Trace bugs through the complete signal and control chain.

- Decoder tests cover format detection, malformed/truncated data, EOF, seek,
  metadata, reusable destinations, variable frame sizes, and finite/aligned
  output.
- Processing tests cover block sizes, channel layouts, sample rates,
  bypass/reset, latency, parameter bounds, non-finite input, and allocation.
- Manager tests cover command ordering, restart/fatal paths, gapless
  transitions, event delivery, and idempotent shutdown.
- Playback/device tests cover negotiation, fallback, disconnect, reconnect,
  callback conversion, and clipping.
- Output clipping (`sample.clamp(-1.0, 1.0)`) belongs at the cpal callback
  boundary so upstream processing remains observable and testable.

The plugin host's `NodeBuffer::clear()` resets `actual_len`; it does not zero
backing storage. `read()` must return an empty slice while `actual_len == 0` and
only the initialized prefix otherwise. Preserve that visibility contract and
add a stale-data regression test whenever buffer lifecycle code is touched.

### Plugin parameter wiring

A DSP plugin parameter must be wired consistently through:

1. `rebuild_cached_parameters`;
2. `set_parameter`;
3. `get_parameter`.

Also update parameter metadata/schema, defaults, presets, automation behavior,
and UI exposure when those surfaces exist. A missing cached parameter can cause
silent rejection even when the setter appears correct. Test round-trip set/get,
boundary values, automation, rebuild/reset, and processing effect.

Typical internal instantiation uses a stable plugin type and JSON parameters:

```rust
PluginConfig {
    plugin_type: "EQ".into(),
    parameters: json!({
        "filters": [{
            "filter_type": "peak",
            "frequency": 1000.0,
            "q": 1.5,
            "gain_db": 3.0
        }]
    }),
}
```

Do not infer external format parameter IDs or ABI details from this internal
configuration example.

---

## Subsystem-specific guidance

### Player and application state

- Keep application-independent state transitions in `sotf-player`.
- Keep the GPUI/TUI/CLI layer responsible for presentation, input mapping, and
  platform glue only.
- Avoid parallel configuration models. If UI needs a derived representation,
  make conversion and ownership explicit.
- Test command/event ordering, failure recovery, persistence migration, and
  idempotent user actions.
- When clap uses flattened argument structs, check duplicate field/flag names
  before attributing parse failures to defaults.

### GPUI

Read `GPUI.md` in full before editing `crates/app-gpui/`.

- Reuse `gpui-toolkit` components, tokens, and interaction patterns before
  introducing app-local equivalents.
- Keep expensive work, blocking I/O, and audio control off the render path.
- Preserve focus, keyboard navigation, accessibility labels, scaling, narrow
  layouts, and theme behavior.
- Test state transitions and components, not only screenshots.
- Run design-token drift checks after token or styled-component work.
- If the defect is in a toolkit component, fix it in `gpui-toolkit` and update
  the dependency deliberately instead of copying the component into SOTF.

### AutoEQ and RoomEQ

- Filter placement must stay within actual measurement-data frequency bounds.
- Passband detection uses thresholds relative to the response peak, not
  absolute dB thresholds.
- Align response grids deliberately before comparing or optimizing multiple
  measurements.
- Keep smoothing, target-curve, passband, weighting, and normalization choices
  explicit in code and tests.
- The core filter implementation comes from the external AutoEQ/math crates;
  common filter types include peak, low/high shelf, low/high pass, band-pass,
  and notch.
- Validate optimization outputs for finite coefficients, stable filters,
  permitted frequency/Q/gain bounds, deterministic seeded behavior, and export
  round trips.
- Use representative fixtures and focused convergence tests; do not loosen
  tolerances merely to hide nondeterminism.

### Systemwide audio

- Preserve one clear owner for daemon state, HAL state, shared memory, and
  lifecycle transitions.
- Treat driver protocol, channel mapping, encrypted handoff, and shared-memory
  layout as versioned compatibility surfaces.
- Separate privileged installation from ordinary build/test workflows.
- Test install, upgrade, reconnect, daemon crash, device disappearance, and
  uninstall/recovery paths with explicit cleanup.
- Never run `install-*`, `uninstall-sotf`, privileged driver loads, or commands
  that replace a user's active audio route without explicit authorization.

### Services, providers, and networking

- Default tests use mocked provider APIs and deterministic fixtures.
- Live Spotify/TIDAL tests are explicit, environment-gated, and use dedicated
  QA accounts. Follow `TEST-CLIENTSERVER.md`.
- Never print, snapshot, commit, or embed tokens, passwords, cookies,
  certificates, signing identities, or account data.
- Add timeouts, cancellation, bounded retry/backoff, response-size limits, and
  contextual errors at network boundaries.
- Test malformed responses, auth expiry, rate limits, partial streams,
  disconnect/reconnect, and idempotent retry behavior.
- Protocol/discovery code must remain portable across the platforms declared by
  its Cargo feature gates.

The Spinorama speaker API currently exposes these resource shapes:

```text
GET http://api.spinorama.org/v1/speakers
GET http://api.spinorama.org/v1/speakers/{speaker}/versions
GET http://api.spinorama.org/v1/speakers/{speaker}/versions/{version}/measurements
```

Keep API access in the owning external AutoEQ/client layer. Encode path segments,
validate response schemas and status codes, and mock these endpoints in default
tests rather than duplicating HTTP logic in application crates.

### FFI and plugin formats

- Keep ownership and lifetime rules explicit across Rust/C/Swift/plugin-host
  boundaries.
- Do not unwind across FFI. Translate failures to the declared error contract.
- Validate pointers, lengths, alignment, thread affinity, and callback lifetime
  without adding `unsafe` unless separately approved.
- Test the actual format wrapper (AU/CLAP/VST3), state serialization, automation,
  bus/channel layouts, latency, and host reload—not only the underlying DSP.

### Platform and release work

- Keep platform-specific code behind the narrowest correct `cfg`/feature gate.
- Do not “fix” one platform by disabling another target or swallowing a missing
  backend.
- Cross-platform claims require the relevant macOS/Linux/Windows CI or an
  explicitly reported limitation.
- Release artifacts use lowercase
  `name-version-os-arch.format`; Debian packages retain Debian's underscore
  convention.
- User-facing binaries use kebab-case; internal CLI tools use snake_case;
  crates use kebab-case; Rust modules use snake_case.
- Signing, notarization, Store submission, driver installation, and publication
  are externally visible operations and require explicit authorization.

Build configuration invariants:

- Treat profile changes as performance/diagnostic changes. The current `dev`
  profile uses `opt-level = 1`; benchmark or reproduce CoreAudio/cpal issues
  before changing it globally.
- The everyday `release` profile uses thin LTO and multiple codegen units for
  iteration speed. The shipping `dist` profile uses fat LTO and one codegen
  unit. Do not use one as evidence for the performance or build cost of the
  other.
- Preserve `panic = "unwind"` where tests and host boundaries depend on it.
- macOS binaries rely on system frameworks and cannot be fully static; Linux
  musl and Windows static-CRT behavior are target-specific.

---

## Debugging playbooks

### Audio crackle, saturation, wrong speed, or underrun

Trace the full chain before editing:

1. Source sample rate, channel layout, and decoder frame contract.
2. Engine negotiation and buffer sizing.
3. Every plugin's frame/channel propagation and reported latency.
4. Allocations, locks, and variable work in process/callback paths.
5. Normalization, gain staging, finite values, conversion, and final clipping.

A downstream clamp may hide the symptom while leaving the cause intact.

### First fix failed

Stop and re-evaluate the model. Reproduce from a smaller boundary, identify what
the failed attempt disproved, and test a different root-cause hypothesis. Do not
stack variants of the same unsupported assumption. If raw logs or a fixture are
missing, request them before guessing.

### Configuration or CLI parse failures

Inspect flattened structs for duplicate option names, serde aliases/defaults,
feature-gated fields, migration order, and config precedence before changing
defaults.

### Platform-only failures

Separate compile-time gating, link/system dependencies, runtime backend
availability, permissions/entitlements, device state, and packaging/signing.
Record the exact layer that failed; “works on my machine” is not a portability
result.

---

## Review and PR policy

Use the repository's specialized review routes when they are available:

- `audio-optimizer` — DSP, FFT, SIMD, allocation, and realtime hot paths in the
  engine, host, plugins, AutoEQ, and math dependencies.
- `psychoacoustics-researcher` — target curves, audibility, loudness
  compensation, spatial/upmixer decisions, and perceptual validation.
- `feature-dev:code-architect` — multi-crate refactors or non-trivial features
  touching more than three files.
- `pr-review-toolkit:code-reviewer` — mandatory review for engine and plugin
  pull requests.

Workflow skills route as follows:

- `/review-pr` for PRs touching `crates/sotf-engine/` or
  `crates/sotf-plugins/`;
- `/code-review` for other crates;
- `/feature-dev` for non-trivial feature architecture and implementation.

- Every change under `crates/sotf-engine/` or `crates/sotf-plugins/` must be a
  dedicated PR and pass `pr-review-toolkit:code-reviewer` before merge.
- Other crates use the repository `/code-review` workflow.
- Non-trivial cross-crate features use `/feature-dev` or the designated
  architecture review workflow.
- DSP, FFT, SIMD, and realtime hot-path work should receive audio-specialist
  review. Psychoacoustic decisions require the relevant perceptual rationale,
  calibration assumptions, and listening/metric validation.

A substantive review must cover:

1. correctness defects and a test that would expose each defect;
2. algorithm and state-machine edge cases;
3. missing validation, errors, cancellation, or recovery behavior;
4. realtime allocation/locking/deadline impact where applicable;
5. compatibility and platform impact;
6. concrete, scoped fix suggestions.

Before handoff or PR creation:

- review the final diff and file list;
- run `git diff --check`;
- confirm no secrets, build products, temporary files, or unrelated churn;
- include exact verification commands and results;
- link the issue when one exists and state any follow-up work explicitly.

Surface-level approval is not sufficient for engine or plugin changes.
