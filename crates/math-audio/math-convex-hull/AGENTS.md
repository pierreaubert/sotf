# math-convex-hull (lib: `math-convex-hull`, version: 0.3.2)

3D convex hull computation and computational geometry.

## Purpose

Computes 3D convex hulls, used in mesh processing and geometric operations.

## Dependencies

- `serde` - Serialization
- `rayon` - Parallel computation
- `rand` - Random generation for testing

## Testing

```bash
cargo test -p math-convex-hull --lib
cargo check -p math-convex-hull && cargo clippy -p math-convex-hull
```
