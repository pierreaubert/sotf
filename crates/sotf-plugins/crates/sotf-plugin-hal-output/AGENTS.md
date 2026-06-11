# sotf-plugin-hal-output

macOS HAL output plugin — writes audio to CoreAudio HAL driver.

## Architecture

- `lib.rs` — Main plugin struct, implements `Plugin` trait
- `params.rs` — Parameter definitions and JSON deserialization


## Key Public API

- Main plugin struct implementing `sotf_host::plugin::Plugin`
- Plugin parameters via `params.rs`

## Testing

```bash
cargo test -p sotf-plugin-hal-output
```

## Important Notes

- macOS only — requires CoreAudio HAL driver (sotf-macos-hal)
- Plugin trait (not InPlacePlugin) — consumes input without producing output
