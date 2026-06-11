# sotf-plugin-hal-input

macOS HAL input plugin — reads audio from CoreAudio HAL driver.

## Architecture

- `lib.rs` — Main plugin struct, implements `Plugin` trait
- `params.rs` — Parameter definitions and JSON deserialization


## Key Public API

- Main plugin struct implementing `sotf_host::plugin::Plugin`
- Plugin parameters via `params.rs`

## Testing

```bash
cargo test -p sotf-plugin-hal-input
```

## Important Notes

- macOS only — requires CoreAudio HAL driver (sotf-macos-hal)
- Plugin trait (not InPlacePlugin) — generates output without input
