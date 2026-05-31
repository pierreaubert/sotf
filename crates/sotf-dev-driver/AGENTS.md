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
| `wait_until` | Poll a property until it matches; with optional `timeout=`.  |
| `sleep`      | Real-time wait (escape hatch).                               |
| `focus`      | Sugar: `focus library` → `action SwitchToLibrary`.           |
| `key`        | Synthetic keystroke (`gpui::Keystroke::parse` syntax).       |
| `click`      | Click a `dev_track`-registered element by selector.          |
| `elements`   | Print every tracked selector to stdout (debugging aid).      |

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
