//! Neighbor Integration Test
//!
//! Test if using higher integration accuracy for neighbor elements improves E[1].
//! Hypothesis: Edge-adjacent elements need special treatment.

#[cfg(feature = "pure-rust")]
fn main() {
    use bem::core::integration::{
        regular_integration, regular_integration_fixed_order, singular_integration,
    };
    use bem::core::mesh::generators::generate_icosphere_mesh;
    use bem::core::types::PhysicsParams;
    use num_complex::Complex64;
    use std::collections::HashSet;
    use std::f64::consts::PI;

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;
    let ka_target = 0.2;
    let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);

    println!("=== Neighbor Integration Test ===\n");
    println!("Testing if higher quadrature for neighbor elements improves E[1]\n");

    for subdivisions in [2, 3] {
        let mesh = generate_icosphere_mesh(radius, subdivisions);
        let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);

        println!(
            "--- Icosphere subdivisions={} ({} elements) ---",
            subdivisions,
            mesh.elements.len()
        );

        // Find neighbor elements for source element 0
        let source_idx = 0;
        let source_elem = &mesh.elements[source_idx];
        let source_point = &source_elem.center;
        let source_normal = &source_elem.normal;
        let source_nodes: HashSet<usize> = source_elem.connectivity.iter().copied().collect();

        // Find elements sharing at least one node
        let mut neighbors: HashSet<usize> = HashSet::new();
        for (j, elem) in mesh.elements.iter().enumerate() {
            if j == source_idx {
                continue;
            }
            for &node in &elem.connectivity {
                if source_nodes.contains(&node) {
                    neighbors.insert(j);
                    break;
                }
            }
        }

        println!(
            "  Source element {} has {} neighbor elements",
            source_idx,
            neighbors.len()
        );

        // Compute E integrals with STANDARD integration
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

        let mut offdiag_e_standard = Complex64::new(0.0, 0.0);
        let mut neighbor_e_standard = Complex64::new(0.0, 0.0);
        let mut far_e_standard = Complex64::new(0.0, 0.0);

        for (j, field_elem) in mesh.elements.iter().enumerate() {
            if j == source_idx {
                continue;
            }
            let element_coords = mesh.element_nodes(field_elem);
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
            offdiag_e_standard += result.d2g_dnxdny_integral;
            if neighbors.contains(&j) {
                neighbor_e_standard += result.d2g_dnxdny_integral;
            } else {
                far_e_standard += result.d2g_dnxdny_integral;
            }
        }

        let e1_standard = self_e + offdiag_e_standard;

        println!("  Standard integration:");
        println!("    Self E: {:.4}", self_e.re);
        println!(
            "    Neighbor E sum: {:.4} ({} elements)",
            neighbor_e_standard.re,
            neighbors.len()
        );
        println!("    Far E sum: {:.4}", far_e_standard.re);
        println!(
            "    E[1] = {:.4} (error = {:.2}%)",
            e1_standard.re,
            e1_standard.norm() / self_e.norm() * 100.0
        );

        // Compute E integrals with HIGH-ORDER integration for neighbors
        let mut offdiag_e_high = Complex64::new(0.0, 0.0);
        let mut neighbor_e_high = Complex64::new(0.0, 0.0);

        for (j, field_elem) in mesh.elements.iter().enumerate() {
            if j == source_idx {
                continue;
            }
            let element_coords = mesh.element_nodes(field_elem);

            let result = if neighbors.contains(&j) {
                // Use high-order fixed quadrature for neighbors
                regular_integration_fixed_order(
                    source_point,
                    source_normal,
                    &element_coords,
                    field_elem.element_type,
                    &physics,
                    12, // High order
                )
            } else {
                regular_integration(
                    source_point,
                    source_normal,
                    &element_coords,
                    field_elem.element_type,
                    field_elem.area,
                    &physics,
                    None,
                    0,
                    false,
                )
            };

            offdiag_e_high += result.d2g_dnxdny_integral;
            if neighbors.contains(&j) {
                neighbor_e_high += result.d2g_dnxdny_integral;
            }
        }

        let e1_high = self_e + offdiag_e_high;

        println!("\n  High-order neighbor integration (order=12):");
        println!("    Neighbor E sum: {:.4}", neighbor_e_high.re);
        println!(
            "    E[1] = {:.4} (error = {:.2}%)",
            e1_high.re,
            e1_high.norm() / self_e.norm() * 100.0
        );

        // Compare neighbor contributions
        let neighbor_diff = (neighbor_e_high - neighbor_e_standard).norm();
        println!(
            "\n  Neighbor E difference (high vs standard): {:.4}",
            neighbor_diff
        );
        println!(
            "  This accounts for {:.2}% of the E[1] error improvement",
            (e1_standard.norm() - e1_high.norm()) / e1_standard.norm() * 100.0
        );

        println!();
    }

    println!("=== Conclusion ===");
    println!("If high-order neighbor integration significantly reduces E[1],");
    println!("then the adaptive subelement method isn't adequately handling");
    println!("nearly-singular integrals for edge-adjacent elements.");
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
