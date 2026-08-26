# sotf-dev-driver

Scenario driver for the SotF dev API. Reads a line-based `.scn`
script and translates each verb into an HTTP call against a running
SotF instance with the `dev-api` feature enabled.

Supported apps:
- **GPUI** (`sotf-desktop`) — desktop GUI app (primary target)
- **TUI** (`sotf-tui`) — terminal UI app (requires `dev-api` feature)
- **CLI** (`player-cli`, `sotf-recorder-cli`) — tested via Rust integration tests in `crates/app-cli/tests/`

## Run a scenario

```bash
# Terminal 1: launch SotF with the debug-only dev API + an isolated QA config dir.
QA_DIR=$(mktemp -d)
cargo run -p sotf-gpui --features dev-api --bin sotf-desktop -- --qa "$QA_DIR"

# Terminal 2: drive a scenario.
cargo run -p sotf-dev-driver -- crates/sotf-dev-driver/scenarios/smoke.scn -v
```

The `--qa <dir>` flag points SotF at a clean, throwaway config directory so
runs are reproducible (no leftover library scans, plugin presets, or window
geometry from a previous session). Each scenario in `scenarios/` assumes the
process was launched this way — `mktemp -d` per run, deleted after.
The app refuses to compile `dev-api` in release builds, and even debug builds
only start the HTTP endpoint when `--qa <dir>` is present.

Override the URL with `--url http://127.0.0.1:9999`. Override the
server port with `SOTF_DEV_API_PORT=9999`.

## Run a TUI scenario

The TUI app also exposes a dev API when compiled with the `dev-api`
feature. Scenarios use the same `.scn` DSL as GPUI, but actions map to
TUI state transitions instead of GPUI actions.

```bash
# Terminal 1: launch TUI with dev API
cargo run -p sotf-tui --features dev-api --bin sotf-tui -- --qa "$QA_DIR"

# Terminal 2: drive a scenario
cargo run -p sotf-dev-driver -- crates/sotf-dev-driver/scenarios/tui_smoke.scn --url http://127.0.0.1:7777 -v
```

TUI-specific notes:
- `action SwitchTo<Screen>` transitions between TUI screens (Library, Queue, Configure, Plugins, Devices, Playlists).
- `key <keystroke>` synthesises crossterm key events (e.g. `enter`, `esc`, `ctrl-c`, `space`).
- No `click` verb — TUI is keyboard-driven.
- The suite runner supports TUI via `app_bin = "target/debug/sotf-tui"` in the suite TOML.

## Run a suite

Suites start a fresh app process per scenario, assign a free dev-api
port, seed fixtures (GPUI only), run the `.scn`, collect app
stdout/stderr under `target/qa-gpui`, and then call `/quit`.

```bash
cargo build -p sotf-gpui --bin sotf-desktop --features "onnx, hal, gpu-2d, gpu-3d, iamf, dev-api"
cargo run -p sotf-dev-driver -- run-suite crates/sotf-dev-driver/suites/smoke.toml -v
```

The RoomEQ wizard matrix can be run with:

```bash
cargo run -p sotf-dev-driver -- run-suite crates/sotf-dev-driver/suites/roomeq_matrix.toml -v
```

Suite entries use `[[scenario]]`:

```toml
[[scenario]]
name = "recording-fake-capture"
path = "crates/sotf-dev-driver/scenarios/recording_fake_capture.scn"
timeout = "30s"

[scenario.fake_recording]
channels = 2
points = 48
```

`seed_demo_audio = true` copies the checked-in demo audio fixtures into
the scenario artifact directory and sends them to `/qa/seed`.
`require_virtual_audio = true` skips the scenario unless `AEQ_E2E_DEVICE`
is set; use it for BlackHole/SotF HAL loopback smoke tests.

RoomEQ fixture scenarios use `[scenario.room_eq]`. Fixtures must live under
`crates/sotf-dev-driver/testkit/roomeq/`; the runner rejects sibling checkout
and absolute-path fixtures, then copies the data into the per-scenario
artifact `dist/` tree. The existing optimizer matrix uses `/qa/room-eq` only
as an optimizer-adapter integration test. UI end-to-end scenarios must use a
fixture solely to arrange the environment and then click visible workflow
controls.

```toml
[scenario.room_eq]
fixture_dir = "crates/sotf-dev-driver/testkit/roomeq/stereo_reference"
dist_path = "fixtures/roomeq/stereo_reference"
target = "NearField"      # NearField | MidField | FarField
loss = "Flat"             # Flat | Epa
processing = "Iir"        # Iir | MixedPhase
crossover = "Lr24"        # Lr24 | Lr48
num_filters = 7
max_iter = 20
population = 24
start = true
```

## CLI integration tests

The CLI binaries (`player-cli` and `sotf-recorder-cli`) are tested via
Rust integration tests in `crates/app-cli/tests/integration_tests.rs`.
These tests invoke the actual compiled binaries and assert on exit codes,
stdout, and stderr.

```bash
cargo test -p app-cli --test integration_tests
```

Covered commands:
- `player-cli devices` — lists audio devices
- `player-cli replay-gain <file>` — analyzes ReplayGain
- `player-cli play <file>` — argument parsing with filters, rack, LUFS, loudness compensation
- `player-cli play /nonexistent` — error handling
- `sotf-recorder-cli --list-devices` — lists devices
- `sotf-recorder-cli` — missing-arg validation, channel-config validation

## Per-screen scenarios

One scenario per main user-facing screen lives in `scenarios/`:

| File                  | Screen        |
|-----------------------|---------------|
| `library.scn`         | Library       |
| `studio.scn`          | Studio        |
| `plugin_graph.scn`    | PluginGraph   |
| `recording.scn`       | Recording     |
| `room_eq.scn`         | RoomEq        |
| `headphone_eq.scn`    | HeadphoneEq   |
| `spinorama.scn`       | Spinorama     |

Each focuses the screen, asserts `screen.focused`, and returns to Library.
These are seeds — extend them with screen-specific actions and queries.

## Regression scenarios

Bug fixes in `app-gpui` get a companion `.scn` named after the bug
(e.g. `queue_stale_index.scn`). The header comment carries the date,
the file:line of the bug, the file:line of the fix, and a reproducible
run command. See [`AGENTS.md`](./AGENTS.md#regression-scenario-convention)
for the full convention. When the bug is pure logic with no GPUI
involvement, prefer a unit test in the implicated crate over a `.scn`.

## DSL

| Verb         | Effect                                                       |
|--------------|--------------------------------------------------------------|
| `action`     | Dispatch a named GPUI Action (with optional inline JSON).    |
| `query`      | Read a property by string path; print value when `-v`.       |
| `assert`     | Compare a property to a literal; fail script on mismatch.    |
| `assert_snapshot` | Compare a QA screenshot with a baseline PNG; write expected/actual/diff artifacts on mismatch. |
| `assert_accessible` | Require a rendered accessibility role/name pair.         |
| `assert_inaccessible` | Require that a role/name pair is absent from the current rendered tree. |
| `assert_focused` | Require that an accessibility element ID owns keyboard focus. |
| `wait_until` | Poll a property until it matches; with optional `timeout=`.  |
| `wait_idle`  | Wait for rendered selectors to remain stable; optional timeout duration. |
| `sleep`      | Real-time wait (escape hatch).                               |
| `focus`      | Sugar: `focus library` → `action SwitchToLibrary`.           |
| `key`        | Synthetic keystroke (`gpui::Keystroke::parse` syntax).        |
| `type`       | Type text through the focused control using real key events; quote as JSON for escapes. |
| `click`      | Click a `dev_track`-registered element by selector.           |
| `hover`      | Move the pointer over a tracked selector.                      |
| `drag`       | Drag from one tracked selector to another.                     |
| `scroll`     | Scroll a selector by a signed vertical pixel delta.            |
| `resize`     | Resize the app content area to a width and height in pixels.   |
| `screenshot` | Capture a PNG below the scenario's isolated QA directory.      |
| `assert_visible` | Require a rendered selector with non-empty bounds.         |
| `assert_absent` | Require that a selector is not rendered.                   |
| `assert_in_viewport` | Require a selector to fit in the current viewport.    |
| `assert_non_overlapping` | Require two selectors not to overlap.              |
| `assert_enabled` / `assert_selected` / `assert_expanded` | Compare explicit semantic state. |
| `export_room_eq_json` | Export completed RoomEQ DSP JSON to the QA artifact. |
| `elements`   | Print every tracked selector to stdout (debugging aid).       |
| `accessibility` | Print the rendered accessibility tree.                     |

Comparison clauses support `==`, `!=`, `>`, `>=`, `<`, and `<=`. They
accept trailing `tolerance=<f>` for numeric equality and `timeout=<dur>`
for `wait_until`. Durations: `Nms` / `Ns` / `Nm` or bare seconds.

```
focus       library
assert      screen.focused == "Library"
action      PlayPause
wait_until  playback.is_playing == true   timeout=2s
wait_idle   2s
action      VolumeUpLarge
assert      playback.volume == 0.85       tolerance=0.01
# After a visible input has been focused with `click`:
type        Focused playlist name
screenshot  playlists-compact
assert_snapshot playlists-compact crates/sotf-dev-driver/baselines/playlists-compact.png 0.02
assert_accessible button New Playlist
assert      roomeq.average_post_score <= 35
action      Stop
```

Action names are namespaced (`player_ui::PlayPause`) but the driver
also accepts the bare name when it's unambiguous — see
`actions.rs` in `crates/app-gpui/app/actions.rs`.

## Available query paths

See `crates/app-gpui/app/dev_api/queries.rs` — currently:

- `playback.volume`        → number
- `playback.is_playing`    → bool
- `playback.muted`         → bool
- `screen.focused`         → string (Screen variant)
- `queue.length`           → number
- `queue.current_index`    → number | null
- `playlists.count`, `playlists.first_name`, `playlists.dialog`,
  `playlists.active_track_count`, `playlists.undo_available`
- `library.directory_count`, `library.album_count`, `library.track_count`,
  `library.filtered_album_count`, `library.search_query`, `library.sort_order`,
  `library.channel_filter`
- `recording.step`, `recording.channel_count`, `recording.done_count`,
  `recording.all_done`, `recording.status`
- `roomeq.step`, `roomeq.measurement_count`, `roomeq.speaker_config_count`,
  `roomeq.optimization_status`, `roomeq.result_count`, `roomeq.filter_count`,
  `roomeq.has_dsp_output`, `roomeq.dsp_channel_count`,
  `roomeq.average_pre_score`, `roomeq.average_post_score`,
  `roomeq.wizard.target`, `roomeq.wizard.loss`,
  `roomeq.wizard.processing`, `roomeq.wizard.crossover`, `roomeq.status`,
  `roomeq.error`
- `roomeq.export.path`, `roomeq.export.exists`, `roomeq.export.bytes`,
  `roomeq.export.channel_count`, `roomeq.export.plugin_count`,
  `roomeq.export.filter_count`, `roomeq.export.version`
- `settings.theme`, `settings.language`, `settings.release_channel`
- `audio.input_device`, `audio.output_device`, and device counts

Adding a new path is two lines in that file (one match arm).

## Tracked elements (`click` selectors)

To make a widget clickable from a scenario, wrap the outer builder
chain with `dev_track(...)`:

```rust
#[cfg(feature = "dev-api")]
use crate::app::dev_api::DevTrackExt;

let wrapper = div().id("transport-play-wrapper").on_click(...).child(...);
#[cfg(feature = "dev-api")]
let wrapper = wrapper.dev_track("transport.play");
wrapper
```

The wrapper records its painted bounds each frame; `/click` looks up
the most recent bounds and synthesises a left-mouse-down + up at the
centre. Reference example:
`crates/app-gpui/components/home/footer.rs` (selector `transport.play`).

`elements` (verb) or `GET /elements` lists every currently registered
selector along with its bounds — handy when a scenario can't find the
button it expects.

## Status

All seven phases of the original plan are wired:

- Action dispatch via `gpui::Action` registry.
- Query allow-list (`crates/app-gpui/app/dev_api/queries.rs`).
- Synthetic keystrokes via `Window::dispatch_keystroke`.
- ElementId registry + click synthesis via `Window::dispatch_event`.
- Line-based DSL with `action / query / assert / assert_snapshot / assert_accessible / assert_inaccessible / wait_until / wait_idle / sleep /
  focus / key / type / click / hover / drag / scroll / resize / screenshot / elements /
  accessibility`.
