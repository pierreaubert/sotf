# math-delaunay (lib: `math_delaunay`)

Delaunay triangulation and Voronoi diagram -- faithful port of [d3-delaunay](https://github.com/d3/d3-delaunay) to Rust, using [delaunator](https://crates.io/crates/delaunator) as the triangulation backend.

## Usage

```rust
use math_delaunay::Delaunay;

let points = vec\![(0.0, 0.0), (1.0, 0.0), (0.5, 1.0)];
let d = Delaunay::from_points(&points);
let v = d.voronoi([0.0, 0.0, 2.0, 2.0]);
if let Some(cell) = v.cell_polygon(0) {
    println\!("Cell 0 has {} vertices", cell.len());
}
```

## Key Types

- `Delaunay` -- Delaunay triangulation from a set of 2D points
- `Voronoi` -- Voronoi diagram derived from a Delaunay triangulation

## Module Layout

- `delaunay.rs` -- Triangulation construction and queries
- `voronoi.rs` -- Voronoi diagram computation and cell polygon extraction

## Dependencies

- `delaunator` -- Core triangulation algorithm

## Testing

```bash
cargo test -p math-delaunay --lib
cargo check -p math-delaunay && cargo clippy -p math-delaunay
```

## License

See the root workspace `LICENSE` file.
