# sotf-dev-driver

Scenario driver for the SotF GPUI dev API. Reads line-based `.scn` scripts and translates each verb into an HTTP call against a running `SotF` instance with the `dev-api` feature enabled.

## Architecture

- `src/main.rs` — full driver: argument parsing, scenario parser, HTTP client, verb dispatcher.

## Verbs

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
| `key`        | Synthetic keystroke (`gpui::Keystroke::parse` syntax).       |
| `type`       | Type text through the focused control using real key events; quote as JSON for escapes. |
| `click`      | Click a `dev_track`-registered element by selector.          |
| `hover`      | Move the pointer over a tracked selector.                     |
| `drag`       | Drag from one tracked selector to another.                    |
| `scroll`     | Scroll a selector by a signed vertical pixel delta.           |
| `resize`     | Resize the app content area to a width and height in pixels.  |
| `screenshot` | Capture a PNG below the scenario's isolated QA directory.     |
| `assert_visible` | Require a rendered selector with non-empty bounds.         |
| `assert_absent` | Require that a selector is not rendered.                   |
| `assert_in_viewport` | Require a selector to fit in the current viewport.    |
| `assert_non_overlapping` | Require two selectors not to overlap.              |
| `assert_enabled` / `assert_selected` / `assert_expanded` | Compare explicit semantic state. |
| `elements`   | Print every tracked selector to stdout (debugging aid).      |
| `accessibility` | Print the rendered accessibility tree.                     |

## Testing

```bash
cargo check -p sotf-dev-driver && cargo clippy -p sotf-dev-driver
cargo test -p sotf-dev-driver
```

## Important Notes

- Requires SotF running with `--features dev-api` and an isolated `--qa <dir>` config directory.
- Default URL `http://127.0.0.1:7777`; override via `--url` or `SOTF_DEV_API_PORT`.
- Scenarios live under `scenarios/`, one per main user-facing screen.
- See `crates/app-gpui/app/dev_api/queries.rs` for the available query path allow-list.
- Click selectors must be wrapped with `dev_track(...)` in the GPUI builder chain to be addressable.

## Regression-scenario convention

Whenever a UI bug is fixed in `app-gpui`, add a `.scn` scenario under
`scenarios/` named after the bug (e.g. `queue_stale_index.scn`,
`library_focus_loop.scn`). The header comment must include:

1. Date and a one-paragraph bug description (file:line, what panicked /
   misbehaved, the trigger conditions).
2. Where the fix landed (file:line) so a future reader can audit it.
3. A reproducible run command (`mktemp -d` for QA dir, the `cargo run`
   invocations).

The scenario doesn't have to deterministically reproduce the original
race — many UI bugs depend on paint/event timing that an HTTP driver
can't reliably hit. What it must do is exercise the same code path
(register `dev_track` selectors on the implicated elements if needed,
then `click` / `key` / `action` against them) so that a future
refactor that re-introduces the bug fails the scenario at least
sometimes. Document any non-determinism in the scenario header.

When the bug is in pure logic (no GPUI involvement), prefer a Rust
unit test in the corresponding crate over a `.scn` scenario.
