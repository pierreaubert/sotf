//! E Nearby Element Analysis
//!
//! Analyze E integral accuracy for elements at different distances from source.
//! Hypothesis: Nearly-singular elements (neighbors) may have integration errors.

#[cfg(feature = "pure-rust")]
fn main() {
    use math_audio_bem::core::integration::{regular_integration, singular_integration};
    use math_audio_bem::core::mesh::generators::generate_icosphere_mesh;
    use math_audio_bem::core::types::PhysicsParams;
    use num_complex::Complex64;
    use std::f64::consts::PI;

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;
    let ka_target = 0.2;
    let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);
    let k = 2.0 * PI * frequency / speed_of_sound;

    println!("=== E Nearby Element Analysis ===\n");
    println!("ka = {:.2}, k = {:.4}", ka_target, k);

    // Test with different mesh resolutions
    for subdivisions in [1, 2, 3] {
        let mesh = generate_icosphere_mesh(radius, subdivisions);
        let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);

        println!(
            "\n--- Icosphere subdivisions={} ({} elements) ---",
            subdivisions,
            mesh.elements.len()
        );

        // Pick a source element
        let source_idx = 0;
        let source_elem = &mesh.elements[source_idx];
        let source_point = &source_elem.center;
        let source_normal = &source_elem.normal;

        // Compute distances from source to all elements
        let mut distances: Vec<(usize, f64, Complex64)> = Vec::new();

        for (j, field_elem) in mesh.elements.iter().enumerate() {
            if j == source_idx {
                continue;
            }

            let element_coords = mesh.element_nodes(field_elem);

            // Distance from source to field element center
            let dx = field_elem.center[0] - source_point[0];
            let dy = field_elem.center[1] - source_point[1];
            let dz = field_elem.center[2] - source_point[2];
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();

            let result = regular_integration(
                source_point,
                source_normal,
                &element_coords,
                field_elem.element_type,
                field_elem.area,
                &physics,
                None,
                0,
                false,
            );

            distances.push((j, dist, result.d2g_dnxdny_integral));
        }

        // Sort by distance
        distances.sort_by(|a, b| a.1.partial_cmp(&b.1).unwrap());

        // Get self E
        let source_coords = mesh.element_nodes(source_elem);
        let self_result = singular_integration(
            source_point,
            source_normal,
            &source_coords,
            source_elem.element_type,
            &physics,
            None,
            0,
            false,
        );
        let self_e = self_result.d2g_dnxdny_integral;

        println!("  Self E: {:.4}+{:.4}i", self_e.re, self_e.im);

        // Characteristic element size
        let elem_size = (source_elem.area * 2.0).sqrt(); // Approximate edge length
        println!("  Element size: {:.4}", elem_size);

        // Analyze E contributions by distance bands
        let bands = [
            (0.0, 1.0, "0-1 elem"),
            (1.0, 2.0, "1-2 elem"),
            (2.0, 4.0, "2-4 elem"),
            (4.0, 8.0, "4-8 elem"),
            (8.0, f64::MAX, "8+ elem"),
        ];

        println!("\n  E contributions by distance from source:");
        let mut total_offdiag = Complex64::new(0.0, 0.0);

        for (min_d, max_d, label) in bands {
            let min_dist = min_d * elem_size;
            let max_dist = max_d * elem_size;

            let band_elements: Vec<_> = distances
                .iter()
                .filter(|(_, d, _)| *d >= min_dist && *d < max_dist)
                .collect();

            let band_sum: Complex64 = band_elements.iter().map(|(_, _, e)| *e).sum();
            total_offdiag += band_sum;

            if !band_elements.is_empty() {
                println!(
                    "    {} (d={:.3}-{:.3}): {} elems, E_sum={:.4}+{:.4}i (|.|={:.4})",
                    label,
                    min_dist,
                    max_dist.min(10.0),
                    band_elements.len(),
                    band_sum.re,
                    band_sum.im,
                    band_sum.norm()
                );
            }
        }

        println!(
            "\n  Total off-diag E: {:.4}+{:.4}i",
            total_offdiag.re, total_offdiag.im
        );
        println!(
            "  E[1] = self + offdiag: {:.4}+{:.4}i",
            self_e.re + total_offdiag.re,
            self_e.im + total_offdiag.im
        );

        // Show nearest neighbors contribution
        println!("\n  Nearest {} elements:", 5.min(distances.len()));
        for (j, dist, e) in distances.iter().take(5) {
            let dist_ratio = dist / elem_size;
            println!(
                "    elem {}: d/h={:.2}, E={:.4}+{:.4}i (|.|={:.4})",
                j,
                dist_ratio,
                e.re,
                e.im,
                e.norm()
            );
        }
    }

    println!("\n=== Analysis ===");
    println!("If the E[1] error is concentrated in nearby elements (0-2 elem distance),");
    println!("then the nearly-singular integration needs improvement.");
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
