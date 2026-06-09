# Symphonia Add-ons

Instructions for crates under `crates/symphonia-add-ons/`.

## Scope

- Keep these crates focused on Symphonia-compatible format readers, audio
  decoders, and their small supporting DSP/bitstream libraries.
- Do not put SOTF player scan policy, database logic, UI behavior, or product
  workflow code in this directory.
- Prefer public APIs that mirror Symphonia conventions: `FormatReader`,
  `AudioDecoder`, registry helpers, explicit codec IDs, packet durations, and
  metadata revisions.

## Licensing

- Preserve upstream notices and crate-specific licenses when code is imported or
  adapted.
- Keep MPL/SOTF workspace crates and GPL-compatible imported crates clearly
  separated in manifests, README files, and changelogs.
- Do not vendor C/C++ code without a deliberate licensing and maintenance
  decision.

## Code Rules

- The root repository rules still apply.
- Prefer pure Rust and keep crate-level `#![forbid(unsafe_code)]` where it is
  already present.
- SIMD or platform intrinsics require an explicit reason, scalar fallbacks, and
  focused tests for all affected paths.
- Hot decode and conversion paths should avoid per-packet heap allocation. Reuse
  buffers and make output sizes predictable from codec parameters when possible.
- Keep endian, bit-order, frame-size, and channel-layout assumptions close to the
  parser code and covered by tests.

## Verification

Run the narrow package tests for the touched crate first. When touching a bridge
used by `symphonia-format-sacd`, also run the SACD tests and the extraction
example checks when a fixture is available.

```bash
cargo test -p symphonia-format-sacd
cargo test -p symphonia-codec-dsd
cargo test -p symphonia-codec-dst
cargo test -p symphonia-codec-wavpack
cargo test -p dst-decoder
cargo test -p rdsd2pcm
```
