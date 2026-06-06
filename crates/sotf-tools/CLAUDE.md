# tools

Utility binaries for test data generation and file conversion. No library.

## Binaries

- `generate-audio-tests` - Generate deterministic test audio signals (sine, sweep, noise, IMD) for validation
- `generate-upmixer-golden` - Generate upmixer golden reference files (depends on `sotf-plugins` internals)
- `export-design-tokens` - Export app-specific SOTF GPUI themes to `design-tokens/tokens.json` (Tokens Studio format)
- `import-design-tokens` - Import app-specific `design-tokens/tokens.json` back into Rust theme files in `app-gpui`
- `sofa-to-sqlite` - Convert SOFA (HRTF) files to SQLite

## Testing

```bash
cargo check -p sotf-tools && cargo clippy -p sotf-tools && cargo test -p sotf-tools
```

## Usage

```bash
cargo run --bin generate-audio-tests --release
cargo run --bin generate-upmixer-golden --release
cargo run --bin export-design-tokens --release
cargo run --bin import-design-tokens --release
cargo run --bin sofa-to-sqlite --release
```

## Notes

- Reverse-depends on `sotf-gpui` (`../app-gpui`) for the design-token round-trip, which
  means editing this crate forces an `app-gpui` rebuild.
- Generic gpui-toolkit token export/import/validation lives in
  `crates/gpui-toolkit/gpui-design-tools`; keep this crate as the SOTF app-theme adapter.
- `generate-upmixer-golden` and noise-based audio fixtures use fixed seeds so regenerated
  golden files are byte-stable.
