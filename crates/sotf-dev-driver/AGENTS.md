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
| `elements`   | List every currently tracked selector (debugging aid).       |

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
