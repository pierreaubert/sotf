# math-delaunay (lib: `math_delaunay`, version: 0.4.1)

Delaunay triangulation and Voronoi diagram -- port of d3-delaunay.

## Key Types

- `Delaunay` -- Triangulation from 2D points (`from_points()`)
- `Voronoi` -- Voronoi diagram from a Delaunay triangulation

## Module Layout

- `delaunay.rs` -- Triangulation construction and queries
- `voronoi.rs` -- Voronoi diagram and cell polygon extraction

## Dependencies

- `delaunator` -- Core triangulation algorithm

## Testing

```bash
cargo test -p math-delaunay --lib
cargo check -p math-delaunay && cargo clippy -p math-delaunay
```
