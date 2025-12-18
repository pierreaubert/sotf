//! Geometric utility functions

use crate::types::Vertex;

// Compute the determinant of a 3x3 matrix
// pub fn det3(a: &[f64; 3], b: &[f64; 3], c: &[f64; 3]) -> f64 {
//     a[0] * (b[1] * c[2] - b[2] * c[1]) - a[1] * (b[0] * c[2] - b[2] * c[0])
//         + a[2] * (b[0] * c[1] - b[1] * c[0])
// }

/// Compute the volume of a tetrahedron formed by 4 points
pub fn tetrahedron_volume(p0: &Vertex, p1: &Vertex, p2: &Vertex, p3: &Vertex) -> f64 {
    let v1 = p1.sub(p0);
    let v2 = p2.sub(p0);
    let v3 = p3.sub(p0);

    v1.dot(&v2.cross(&v3)).abs() / 6.0
}

/// Check if 4 points are coplanar
pub fn are_coplanar(p0: &Vertex, p1: &Vertex, p2: &Vertex, p3: &Vertex, epsilon: f64) -> bool {
    tetrahedron_volume(p0, p1, p2, p3) < epsilon
}

// Find the point furthest from a plane defined by a point and normal
// pub fn furthest_point_from_plane(
//     points: &[Vertex],
//     plane_point: &Vertex,
//     plane_normal: &Vertex,
// ) -> Option<(usize, f64)> {
//     let mut max_distance = 0.0;
//     let mut max_index = None;
//
//     for (i, point) in points.iter().enumerate() {
//         let to_point = point.sub(plane_point);
//         let distance = plane_normal.dot(&to_point);
//
//         if distance > max_distance {
//             max_distance = distance;
//             max_index = Some(i);
//         }
//     }
//
//     max_index.map(|i| (i, max_distance))
// }

/// Find the extreme points (min/max in each dimension)
pub fn find_extreme_points(vertices: &[Vertex]) -> [usize; 6] {
    let mut min_x_idx = 0;
    let mut max_x_idx = 0;
    let mut min_y_idx = 0;
    let mut max_y_idx = 0;
    let mut min_z_idx = 0;
    let mut max_z_idx = 0;

    for (i, v) in vertices.iter().enumerate() {
        if v.x < vertices[min_x_idx].x {
            min_x_idx = i;
        }
        if v.x > vertices[max_x_idx].x {
            max_x_idx = i;
        }
        if v.y < vertices[min_y_idx].y {
            min_y_idx = i;
        }
        if v.y > vertices[max_y_idx].y {
            max_y_idx = i;
        }
        if v.z < vertices[min_z_idx].z {
            min_z_idx = i;
        }
        if v.z > vertices[max_z_idx].z {
            max_z_idx = i;
        }
    }

    [
        min_x_idx, max_x_idx, min_y_idx, max_y_idx, min_z_idx, max_z_idx,
    ]
}

// Compute the centroid of a set of vertices
// pub fn centroid(vertices: &[Vertex]) -> Vertex {
//     let n = vertices.len() as f64;
//     let sum = vertices
//         .iter()
//         .fold(Vertex::new(0.0, 0.0, 0.0), |acc, v| acc.add(v));
//     sum.scale(1.0 / n)
// }

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tetrahedron_volume() {
        let p0 = Vertex::new(0.0, 0.0, 0.0);
        let p1 = Vertex::new(1.0, 0.0, 0.0);
        let p2 = Vertex::new(0.0, 1.0, 0.0);
        let p3 = Vertex::new(0.0, 0.0, 1.0);

        let vol = tetrahedron_volume(&p0, &p1, &p2, &p3);
        assert!((vol - 1.0 / 6.0).abs() < 1e-10);
    }

    #[test]
    fn test_coplanarity() {
        let p0 = Vertex::new(0.0, 0.0, 0.0);
        let p1 = Vertex::new(1.0, 0.0, 0.0);
        let p2 = Vertex::new(0.0, 1.0, 0.0);
        let p3 = Vertex::new(0.5, 0.5, 0.0);

        assert!(are_coplanar(&p0, &p1, &p2, &p3, 1e-8));

        let p4 = Vertex::new(0.0, 0.0, 1.0);
        assert!(!are_coplanar(&p0, &p1, &p2, &p4, 1e-8));
    }
}
