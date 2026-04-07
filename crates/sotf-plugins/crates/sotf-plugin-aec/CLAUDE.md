# sotf-plugin-aec

Acoustic Echo Cancellation — PBFDAF with two-path and post-filter.

## Architecture

- `lib.rs` — Main `AecPlugin`, implements `Plugin` trait
- `params.rs` — Parameter definitions and JSON deserialization

## Key Public API

- `AecPlugin` implementing `sotf_host::plugin::Plugin`

## Testing

```bash
cargo test -p sotf-plugin-aec
```

## Important Notes

- PBFDAF (Partitioned Block Frequency Domain Adaptive Filter) algorithm
- Two-path architecture for robust convergence
- Post-filter for residual echo suppression
- Plugin trait (not InPlacePlugin) — requires both microphone and reference signals
