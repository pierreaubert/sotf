# sotf-dev-driver

Scenario driver for the SotF GPUI dev API. Reads a line-based `.scn`
script and translates each verb into an HTTP call against a running
`SotF` instance with the `dev-api` feature enabled.

## Run a scenario

```bash
# Terminal 1: launch SotF with the dev API + an isolated QA config dir.
QA_DIR=$(mktemp -d)
cargo run -p sotf-gpui --features dev-api --bin SotF -- --qa "$QA_DIR"

# Terminal 2: drive a scenario.
cargo run -p sotf-dev-driver -- crates/sotf-dev-driver/scenarios/smoke.scn -v
```

The `--qa <dir>` flag points SotF at a clean, throwaway config directory so
runs are reproducible (no leftover library scans, plugin presets, or window
geometry from a previous session). Each scenario in `scenarios/` assumes the
process was launched this way — `mktemp -d` per run, deleted after.

Override the URL with `--url http://127.0.0.1:9999`. Override the
server port with `SOTF_DEV_API_PORT=9999`.

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

## DSL

| Verb         | Effect                                                       |
|--------------|--------------------------------------------------------------|
| `action`     | Dispatch a named GPUI Action (with optional inline JSON).    |
| `query`      | Read a property by string path; print value when `-v`.       |
| `assert`     | Compare a property to a literal; fail script on mismatch.    |
| `wait_until` | Poll a property until it matches; with optional `timeout=`.  |
| `sleep`      | Real-time wait (escape hatch).                               |
| `focus`      | Sugar: `focus library` → `action SwitchToLibrary`.           |
| `key`        | Synthetic keystroke (`gpui::Keystroke::parse` syntax).        |
| `click`      | Click a `dev_track`-registered element by selector.           |
| `elements`   | List every currently tracked selector (debugging aid).        |

Comparison clauses accept trailing `tolerance=<f>` (numbers) and
`timeout=<dur>` (`wait_until` only). Durations: `Nms` / `Ns` / `Nm`
or bare seconds.

```
focus       library
assert      screen.focused == "Library"
action      PlayPause
wait_until  playback.is_playing == true   timeout=2s
action      VolumeUpLarge
assert      playback.volume == 0.85       tolerance=0.01
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
- Line-based DSL with `action / query / assert / wait_until / sleep /
  focus / key / click / elements`.
