# zed-font-kit

Cross-platform font loading library (fork of Servo's font-kit, via Zed).

## Purpose

Provides cross-platform font discovery and loading for GPUI rendering.

## Platform Backends

- **macOS/iOS**: CoreText
- **Windows**: DirectWrite
- **Linux/Other**: FreeType + Fontconfig

## Features

- `source` (default) - Font source discovery
- `loader-freetype` - FreeType font loading
- `source-fontconfig` - Fontconfig font discovery
- `source-fontconfig-dlopen` - Dynamic Fontconfig loading

## License

MIT OR Apache-2.0 (Servo project license)

## Testing

```bash
cargo test -p zed-font-kit --lib
cargo check -p zed-font-kit && cargo clippy -p zed-font-kit
```

## Notes

- This is an external fork — minimize changes unless necessary
- Used by GPUI for font rendering
