# convexhull3d: N-Dimensional Convex Hull and Computational Geometry

A Rust implementation of the Quickhull algorithm for computing convex hulls in arbitrary dimensions, plus Delaunay triangulation. Based on the [convhull_3d C library](https://github.com/leomccormack/convhull_3d) and inspired by the [MATLAB Computational Geometry Toolbox](https://www.mathworks.com/matlabcentral/fileexchange/48509-computational-geometry-toolbox).

## Features

- **3D Convex Hull Computation**: Optimized Quickhull algorithm (Barber et al., 1996)
- **N-D Convex Hull**: Generalized algorithm supporting 1D through 10D spaces
- **Delaunay Triangulation**: Via paraboloid lifting technique
- **Geometric Properties**: Calculate volume and surface area of convex hulls
- **Export Capabilities**:
  - OBJ format for 3D modeling software
  - HTML with interactive Three.js visualization
- **Test Data Generation**: Built-in functions for generating test datasets
  - Random spherical distributions
  - Fibonacci sphere (uniform distribution)
  - T-Design approximations
  - Platonic solids (tetrahedron, cube, octahedron, icosahedron)
- **Comprehensive Testing**: Test suite matching the C++ convhull_3d library

## Usage

### Basic Example

```rust
use convexhull3d::{ConvexHull3D, Vertex};

// Define vertices
let vertices = vec![
    Vertex::new(0.0, 0.0, 0.0),
    Vertex::new(1.0, 0.0, 0.0),
    Vertex::new(0.0, 1.0, 0.0),
    Vertex::new(0.0, 0.0, 1.0),
];

// Build convex hull
let hull = ConvexHull3D::build(&vertices).unwrap();

// Get properties
println!("Faces: {}", hull.num_faces());
println!("Volume: {}", hull.volume());
println!("Surface Area: {}", hull.surface_area());
```

### Exporting Results

```rust
use convexhull3d::{ConvexHull3D, export_obj, export_html, testdata};

// Generate test data
let vertices = testdata::icosahedron_vertices();

// Compute hull
let hull = ConvexHull3D::build(&vertices).unwrap();

// Export to OBJ
export_obj(&hull, "icosahedron.obj").unwrap();

// Export to interactive HTML
export_html(&hull, "icosahedron.html", "Icosahedron Hull").unwrap();
```

### Generating Test Data

```rust
use convexhull3d::testdata;

// Random points on a sphere
let sphere_points = testdata::random_sphere_points(1000, 1.0);

// Fibonacci sphere (uniform distribution)
let uniform_sphere = testdata::fibonacci_sphere_points(500, 1.0);

// T-Design sphere approximations
let tdesign_180 = testdata::tdesign_180_sphere();
let tdesign_840 = testdata::tdesign_840_sphere();
let tdesign_5100 = testdata::tdesign_5100_sphere();

// Platonic solids
let cube = testdata::cube_vertices(2.0);
let octahedron = testdata::octahedron_vertices();
let icosahedron = testdata::icosahedron_vertices();
```

### N-Dimensional Convex Hull

The library supports convex hulls in arbitrary dimensions (1D through 10D):

```rust
use convexhull3d::{ConvexHullND, PointND};

// 2D square
let points_2d = vec![
    PointND::new(vec![0.0, 0.0]),
    PointND::new(vec![1.0, 0.0]),
    PointND::new(vec![1.0, 1.0]),
    PointND::new(vec![0.0, 1.0]),
];

let hull_2d = ConvexHullND::build(&points_2d).unwrap();
println!("2D hull: {} facets (edges)", hull_2d.num_facets());

// 4D hypercube
let mut points_4d = Vec::new();
for i in 0..16 {
    let x = if i & 1 != 0 { 1.0 } else { 0.0 };
    let y = if i & 2 != 0 { 1.0 } else { 0.0 };
    let z = if i & 4 != 0 { 1.0 } else { 0.0 };
    let w = if i & 8 != 0 { 1.0 } else { 0.0 };
    points_4d.push(PointND::new(vec![x, y, z, w]));
}

let hull_4d = ConvexHullND::build(&points_4d).unwrap();
println!("4D hypercube: {} facets (tetrahedra)", hull_4d.num_facets());
```

### Delaunay Triangulation

Compute Delaunay triangulations via paraboloid lifting:

```rust
use convexhull3d::{DelaunayMesh, PointND, circumcenter};

// 2D Delaunay triangulation
let points = vec![
    PointND::new(vec![0.0, 0.0]),
    PointND::new(vec![1.0, 0.0]),
    PointND::new(vec![1.0, 1.0]),
    PointND::new(vec![0.0, 1.0]),
    PointND::new(vec![0.5, 0.5]),
];

let mesh = DelaunayMesh::build(&points).unwrap();
println!("Delaunay mesh: {} simplices", mesh.num_simplices());

// Compute circumcenter of a simplex
if let Some(simplex) = mesh.simplices().first() {
    if let Some(center) = circumcenter(simplex, mesh.points()) {
        println!("Circumcenter: {:?}", center);
    }
}
```

**Note**: The Delaunay implementation is currently a prototype and may need refinement for production use.

## Algorithm

The implementation uses the **Quickhull algorithm** for 3D convex hull computation:

1. **Initialization**: Find an initial simplex (tetrahedron) from extreme points
2. **Point Assignment**: Assign remaining points to visible faces
3. **Iteration**:
   - Select furthest point from a face
   - Find all faces visible from that point
   - Determine horizon edges
   - Create new faces connecting the point to the horizon
4. **Termination**: Continue until no outside points remain

### Key Features of the Implementation

- **Robust**: Handles degenerate cases and numerical precision issues
- **Efficient**: O(n log n) expected time complexity
- **Accurate**: Proper face orientation and normal computation
- **Well-tested**: Comprehensive test suite with diverse datasets

## Test Results

The library has been tested against the same test cases as the C++ convhull_3d library:

| Test Case | Input Vertices | Output Faces | Time |
|-----------|---------------|--------------|------|
| Tetrahedron | 4 | 4 | < 1ms |
| Cube | 8 | 12-14 | < 1ms |
| Octahedron | 6 | 8-12 | < 1ms |
| Icosahedron | 12 | 20-32 | < 1ms |
| Random Sphere (936 pts) | 936 | ~14,678 | ~170ms |
| T-Design 180 | 180 | ~1,092 | ~10ms |
| T-Design 840 | 840 | ~24,898 | ~2,500ms |

*Note: Face counts may vary slightly due to numerical precision and triangulation choices.*

## Running Tests

```bash
# Run all tests
cargo test --package convexhull3d

# Run specific test
cargo test --package convexhull3d test_tetrahedron

# Run with output
cargo test --package convexhull3d -- --nocapture
```

### Generated Visualizations

Tests automatically generate:
- **OBJ files**: Located in `target/convexhull_test_output/*.obj`
- **HTML visualizations**: Located in `target/convexhull_test_output/*.html`

Open the HTML files in a browser to see interactive 3D visualizations using Three.js.

## Comparison with C++ Implementation

This Rust implementation provides similar functionality to the [convhull_3d C library](https://github.com/leomccormack/convhull_3d):

| Feature | C Implementation | Rust Implementation |
|---------|-----------------|---------------------|
| Algorithm | Quickhull | Quickhull |
| 3D Convex Hull | ✓ | ✓ |
| N-D Convex Hull | ✓ (up to 5D) | ✓ (up to 10D) |
| Delaunay Triangulation | ✓ | ✓ (prototype) |
| OBJ Export | ✓ | ✓ |
| MATLAB Export | ✓ | - |
| HTML Visualization | - | ✓ |
| Memory Safety | Manual | Automatic |

### Key Advantages of Rust Implementation

1. **Memory Safety**: No manual memory management required
2. **Type Safety**: Compile-time error checking
3. **Modern Tooling**: Cargo, rustdoc, clippy, etc.
4. **Interactive Visualization**: Built-in Three.js HTML export
5. **Comprehensive Error Handling**: Result types instead of error codes

## API Reference

### 3D Types

- **`Vertex`**: 3D point with x, y, z coordinates
- **`Face`**: Triangle defined by 3 vertex indices
- **`ConvexHull3D`**: Result of 3D convex hull computation

### N-D Types

- **`PointND`**: N-dimensional point with arbitrary coordinates
- **`SimplexND`**: N-dimensional simplex (facet) defined by vertex indices
- **`ConvexHullND`**: Result of N-dimensional convex hull computation
- **`DelaunayMesh`**: Result of Delaunay triangulation

### Main Functions

**3D Functions:**
- **`ConvexHull3D::build(&[Vertex])`**: Build 3D convex hull from vertices
- **`export_obj(&ConvexHull3D, path)`**: Export to OBJ format
- **`export_html(&ConvexHull3D, path, title)`**: Export to interactive HTML

**N-D Functions:**
- **`ConvexHullND::build(&[PointND])`**: Build N-D convex hull from points
- **`DelaunayMesh::build(&[PointND])`**: Build Delaunay triangulation
- **`delaunay_nd(&[PointND])`**: Standalone Delaunay triangulation function
- **`circumcenter(&SimplexND, &[PointND])`**: Compute circumcenter of a simplex

### Test Data Generation

- **`testdata::random_sphere_points(n, radius)`**: Random sphere distribution
- **`testdata::fibonacci_sphere_points(n, radius)`**: Uniform sphere distribution
- **`testdata::tdesign_*_sphere()`**: T-Design approximations
- **`testdata::*_vertices()`**: Platonic solids and other shapes

## References

1. Barber, C.B., Dobkin, D.P., and Huhdanpaa, H.T., "The Quickhull algorithm for convex hulls," *ACM Trans. on Mathematical Software*, 22(4):469-483, 1996.
2. [convhull_3d C Library](https://github.com/leomccormack/convhull_3d) by Leo McCormack
3. [MATLAB Computational Geometry Toolbox](https://www.mathworks.com/matlabcentral/fileexchange/48509-computational-geometry-toolbox)

## License

GPL-3.0-or-later (matching the parent project)

## Contributing

This crate is part of the [SotF](https://github.com/pierreaubert/sotf) project.
