# convexhull3d

A Rust implementation of the **Quickhull algorithm** for computing 3D convex hulls.

```
    Input Points                          Convex Hull
    ════════════                          ═══════════
         •                                    ╱╲
        •••                                  ╱  ╲
       • • •           ────────►           ╱────╲
        •••                               ╱      ╲
         •                               ╱________╲
```

Based on the [convhull_3d C library](https://github.com/leomccormack/convhull_3d) by Leo McCormack.

## Quick Start

```rust
use convexhull3d::{ConvexHull3D, Vertex};

let vertices = vec![
    Vertex::new(0.0, 0.0, 0.0),
    Vertex::new(1.0, 0.0, 0.0),
    Vertex::new(0.0, 1.0, 0.0),
    Vertex::new(0.0, 0.0, 1.0),
];

let hull = ConvexHull3D::build(&vertices).unwrap();

println!("Faces: {}", hull.num_faces());      // 4
println!("Volume: {:.4}", hull.volume());     // 0.1667
```

## Visual Examples

### Platonic Solids

The crate includes generators for all Platonic solids:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│  TETRAHEDRON          CUBE               OCTAHEDRON         ICOSAHEDRON     │
│                                                                             │
│       △                ┌───┐                 △                    ⬡         │
│      ╱ ╲              ╱│   │╲               ╱│╲                  ╱│╲        │
│     ╱   ╲            ╱ │   │ ╲             ╱ │ ╲               ╱╱ │ ╲╲      │
│    ╱─────╲          └──┴───┴──┘          ◇──┼──◇             ◇───┼───◇     │
│                                            ╲│╱                 ╲╲│╱╱        │
│   4 vertices        8 vertices            6 vertices          12 vertices   │
│   4 faces           12 faces              8 faces             20 faces      │
└─────────────────────────────────────────────────────────────────────────────┘
```

```rust
use convexhull3d::{ConvexHull3D, testdata};

// Generate and compute hull for each solid
let shapes = [
    ("Tetrahedron", testdata::tetrahedron_vertices()),
    ("Cube",        testdata::cube_vertices(2.0)),
    ("Octahedron",  testdata::octahedron_vertices()),
    ("Icosahedron", testdata::icosahedron_vertices()),
];

for (name, vertices) in shapes {
    let hull = ConvexHull3D::build(&vertices).unwrap();
    println!("{}: {} vertices → {} faces",
             name, vertices.len(), hull.num_faces());
}
```

**Output:**
```
Tetrahedron: 4 vertices → 4 faces
Cube: 8 vertices → 12 faces
Octahedron: 6 vertices → 8 faces
Icosahedron: 12 vertices → 20 faces
```

### Sphere Point Distributions

Generate uniform point distributions on spheres:

```
┌────────────────────────────────────────────────────────────────────────────┐
│  RANDOM SPHERE              FIBONACCI SPHERE           T-DESIGN SPHERE     │
│                                                                            │
│      · ·  · ·                    · · · ·                   · · · · ·       │
│    ·  ·    ·  ·                · · · · · ·               · · · · · · ·     │
│   · ·   ··   · ·              · · · · · · ·             · · · · · · · ·    │
│   ·  · ·  · ·  ·              · · · · · · ·             · · · · · · · ·    │
│    ·   ··   ·                  · · · · · ·               · · · · · · ·     │
│      · ·  · ·                    · · · ·                   · · · · ·       │
│                                                                            │
│  Non-uniform density         Uniform density            Optimal uniformity │
│  936 pts → ~14k faces        500 pts → ~3k faces        180 pts → 1k faces │
└────────────────────────────────────────────────────────────────────────────┘
```

```rust
use convexhull3d::{ConvexHull3D, testdata};

// Random sphere (non-uniform)
let random = testdata::random_sphere_points(936, 1.0);
let hull = ConvexHull3D::build(&random).unwrap();
println!("Random: {} pts → {} faces", random.len(), hull.num_faces());

// Fibonacci sphere (uniform)
let fibonacci = testdata::fibonacci_sphere_points(500, 1.0);
let hull = ConvexHull3D::build(&fibonacci).unwrap();
println!("Fibonacci: {} pts → {} faces", fibonacci.len(), hull.num_faces());

// T-Design (optimal uniformity)
let tdesign = testdata::tdesign_180_sphere();
let hull = ConvexHull3D::build(&tdesign).unwrap();
println!("T-Design: {} pts → {} faces", tdesign.len(), hull.num_faces());
```

### Interior Points Are Ignored

The convex hull only includes points on the outer boundary:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                                                                             │
│   INPUT: Cube with 100 interior points    OUTPUT: Just the cube hull       │
│                                                                             │
│        •───────────•                           •───────────•                │
│       ╱ · · · · · ╱│                          ╱           ╱│                │
│      •─·─·─·─·─·─• │                         •───────────• │                │
│      │ · · · · · │ │         ────►           │           │ │                │
│      │ · · · · · │ •                         │           │ •                │
│      │ · · · · · │╱                          │           │╱                 │
│      •───────────•                           •───────────•                  │
│                                                                             │
│      108 vertices                            8 vertices, 12 faces           │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
```

```rust
use convexhull3d::{ConvexHull3D, testdata};

// Cube with 100 random interior points
let vertices = testdata::cube_with_interior_points(2.0, 100);
println!("Input: {} vertices", vertices.len());  // 108

let hull = ConvexHull3D::build(&vertices).unwrap();
println!("Hull: {} faces", hull.num_faces());    // 12 (just the cube!)
```

## Geometric Properties

Calculate volume and surface area:

```rust
use convexhull3d::{ConvexHull3D, Vertex};

// Unit tetrahedron
let vertices = vec![
    Vertex::new(0.0, 0.0, 0.0),
    Vertex::new(1.0, 0.0, 0.0),
    Vertex::new(0.5, 0.866, 0.0),
    Vertex::new(0.5, 0.289, 0.816),
];

let hull = ConvexHull3D::build(&vertices).unwrap();

println!("Volume: {:.6}", hull.volume());           // ≈ 0.117851
println!("Surface Area: {:.6}", hull.surface_area()); // ≈ 1.732051
```

## Export & Visualization

### OBJ Export (for 3D software)

```rust
use convexhull3d::{ConvexHull3D, export_obj, testdata};

let vertices = testdata::icosahedron_vertices();
let hull = ConvexHull3D::build(&vertices).unwrap();

export_obj(&hull, "icosahedron.obj").unwrap();
```

Opens in Blender, MeshLab, or any 3D modeling software.

### Interactive HTML Visualization

```rust
use convexhull3d::{ConvexHull3D, export_html, testdata};

let vertices = testdata::fibonacci_sphere_points(200, 1.0);
let hull = ConvexHull3D::build(&vertices).unwrap();

export_html(&hull, "sphere_hull.html", "Fibonacci Sphere").unwrap();
```

The HTML export creates an **interactive side-by-side visualization**:

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                        Fibonacci Sphere Hull                                │
├─────────────────────────────────┬───────────────────────────────────────────┤
│                                 │                                           │
│       Original Points           │         Convex Hull Mesh                  │
│                                 │                                           │
│           · · · ·               │              ╱╲    ╱╲                     │
│         · · · · · ·             │            ╱╲  ╲╱╱  ╱╲                    │
│        · · · · · · ·            │           ╱  ╲╱  ╲╱  ╲                   │
│        · · · · · · ·            │          ╱────────────╲                   │
│         · · · · · ·             │          ╲────────────╱                   │
│           · · · ·               │           ╲  ╱╲  ╱╲  ╱                    │
│                                 │            ╲╱  ╲╱╲  ╱                     │
│                                 │              ╲╱    ╲╱                     │
│     [Rotate with mouse]         │        [Cameras synchronized]             │
│                                 │                                           │
└─────────────────────────────────┴───────────────────────────────────────────┘
```

Features:
- **Synchronized dual views**: rotate one, both rotate
- **Red point cloud**: original input vertices
- **Blue mesh**: computed convex hull
- **Wireframe overlay**: see the triangulation
- **Interactive controls**: rotate, zoom, pan

## Performance

| Input | Vertices | Faces | Time |
|-------|----------|-------|------|
| Tetrahedron | 4 | 4 | < 1ms |
| Cube | 8 | 12 | < 1ms |
| Icosahedron | 12 | 20 | < 1ms |
| T-Design 180 | 180 | ~1,092 | ~10ms |
| Random Sphere | 936 | ~14,678 | ~170ms |
| T-Design 840 | 840 | ~24,898 | ~2.5s |

**Complexity**: O(n log n) expected, O(n²) worst case

## Algorithm

The **Quickhull algorithm** (Barber et al., 1996):

```
1. INITIALIZE
   ├── Find extreme points (min/max in X, Y, Z)
   └── Form initial tetrahedron

2. ASSIGN POINTS
   └── Each point → nearest visible face

3. ITERATE (until no outside points)
   ├── Select face with furthest outside point
   ├── Find all faces visible from that point
   ├── Determine horizon edges
   └── Create new faces from point to horizon

4. OUTPUT
   └── Return vertices + triangular faces
```

## API Reference

### Types

| Type | Description |
|------|-------------|
| `Vertex` | 3D point (x, y, z) with vector operations |
| `Face` | Triangle (3 vertex indices) |
| `ConvexHull3D` | Computed hull with vertices, faces, properties |

### Core Functions

```rust
// Build hull from points
ConvexHull3D::build(&[Vertex]) -> Result<ConvexHull3D, ConvexHullError>

// Properties
hull.vertices() -> &[Vertex]
hull.faces() -> &[Face]
hull.num_faces() -> usize
hull.volume() -> f64
hull.surface_area() -> f64

// Export
export_obj(&hull, "file.obj") -> Result<()>
export_html(&hull, "file.html", "Title") -> Result<()>
```

### Test Data Generators

```rust
// Spheres
testdata::random_sphere_points(n, radius)
testdata::fibonacci_sphere_points(n, radius)
testdata::tdesign_180_sphere()
testdata::tdesign_840_sphere()
testdata::tdesign_5100_sphere()

// Platonic solids
testdata::tetrahedron_vertices()
testdata::cube_vertices(size)
testdata::octahedron_vertices()
testdata::icosahedron_vertices()

// Complex
testdata::cube_with_interior_points(size, n)
testdata::load_obj_vertices(path)
```

## Running Tests

```bash
# All tests
cargo test -p convexhull3d

# With timing info
cargo test -p convexhull3d -- --nocapture

# Generate visualizations (requires AUTOEQ_DIR)
AUTOEQ_DIR=/path/to/output cargo test -p convexhull3d
```

## References

1. Barber, C.B., Dobkin, D.P., Huhdanpaa, H.T. (1996). *The Quickhull algorithm for convex hulls*. ACM Trans. Mathematical Software, 22(4):469-483.
2. [convhull_3d C Library](https://github.com/leomccormack/convhull_3d) - Leo McCormack
3. [Qhull](http://www.qhull.org/) - Reference implementation

## License

GPL-3.0-or-later
