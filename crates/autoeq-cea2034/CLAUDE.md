# autoeq-cea2034 (lib: `autoeq_cea2034`, version: 0.3.0)

CEA2034 (Spinorama) speaker measurement metrics.

## Purpose

Implements the CEA2034 standard for speaker measurement data, used by the AutoEQ optimization pipeline.

## Key Types

- `Curve` - Frequency response data
- `DirectivityCurve` - Directivity measurement
- `DirectivityData` - Complete directivity dataset

## Dependencies

- `ndarray` - Array processing
- `serde` - Serialization
- `tokio` - Async support

## Testing

```bash
cargo test -p autoeq-cea2034 --lib
cargo check -p autoeq-cea2034 && cargo clippy -p autoeq-cea2034
```
