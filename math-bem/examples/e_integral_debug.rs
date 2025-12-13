//! E Integral Debug - Compare singular vs regular E contributions
//!
//! Check if the self-element E integral is consistent with off-diagonal E integrals

#[cfg(feature = "pure-rust")]
fn main() {
    use bem::core::integration::{regular_integration, singular_integration};
    use bem::core::mesh::generators::generate_icosphere_mesh;
    use bem::core::types::PhysicsParams;
    use num_complex::Complex64;
    use std::f64::consts::PI;

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;
    let ka_target = 0.2;
    let frequency = ka_target * speed_of_sound / (2.0 * PI * radius);
    let k = 2.0 * PI * frequency / speed_of_sound;

    println!("=== E Integral Debug ===\n");
    println!("ka = {:.2}, k = {:.4}", ka_target, k);

    // Test with icosphere (more uniform elements)
    let mesh = generate_icosphere_mesh(radius, 2);
    println!("\nIcosphere with {} elements", mesh.elements.len());

    let physics = PhysicsParams::new(frequency, speed_of_sound, density, false);
    let beta = physics.burton_miller_beta();

    // Pick a source element and analyze its row
    let source_idx = 0;
    let source_elem = &mesh.elements[source_idx];
    let source_point = &source_elem.center;
    let source_normal = &source_elem.normal;

    println!(
        "\nSource element {}: center=({:.4},{:.4},{:.4}), normal=({:.4},{:.4},{:.4})",
        source_idx,
        source_point[0],
        source_point[1],
        source_point[2],
        source_normal[0],
        source_normal[1],
        source_normal[2]
    );

    // Compute all E integrals for this row
    let mut total_e = Complex64::new(0.0, 0.0);
    let mut total_k = Complex64::new(0.0, 0.0); // Double-layer K
    let mut off_diag_e = Complex64::new(0.0, 0.0);

    for (j, field_elem) in mesh.elements.iter().enumerate() {
        let element_coords = mesh.element_nodes(field_elem);

        let result = if j == source_idx {
            // Self-element (singular)
            singular_integration(
                source_point,
                source_normal,
                &element_coords,
                field_elem.element_type,
                &physics,
                None,
                0,
                false,
            )
        } else {
            // Regular element
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

        total_e += result.d2g_dnxdny_integral;
        total_k += result.dg_dn_integral;

        if j == source_idx {
            let self_e = result.d2g_dnxdny_integral;
            println!(
                "\nSelf-element E: {:.4}+{:.4}i (|.|={:.4})",
                self_e.re,
                self_e.im,
                self_e.norm()
            );
            println!(
                "Self-element K: {:.4}+{:.4}i (|.|={:.4})",
                result.dg_dn_integral.re,
                result.dg_dn_integral.im,
                result.dg_dn_integral.norm()
            );
        } else {
            off_diag_e += result.d2g_dnxdny_integral;
        }
    }

    println!(
        "\nTotal E (sum of all j): {:.4}+{:.4}i (|.|={:.4})",
        total_e.re,
        total_e.im,
        total_e.norm()
    );
    println!(
        "Off-diagonal E sum: {:.4}+{:.4}i (|.|={:.4})",
        off_diag_e.re,
        off_diag_e.im,
        off_diag_e.norm()
    );
    println!(
        "Total K (should be ~-0.5): {:.4}+{:.4}i",
        total_k.re, total_k.im
    );

    // β*E[1] contribution to row sum
    let beta_e_one = beta * total_e;
    println!(
        "\nβ*E[1] = {:.4}+{:.4}i (should be ~0)",
        beta_e_one.re, beta_e_one.im
    );

    // Check symmetry: compare E_ij for i=0 with E_ji
    println!("\n--- Symmetry check E_ij vs E_ji ---");
    for j in [1, 10, 50, 100, 200] {
        if j >= mesh.elements.len() {
            continue;
        }

        let field_elem_j = &mesh.elements[j];
        let coords_j = mesh.element_nodes(field_elem_j);

        // E_ij: source=i, field=j
        let result_ij = regular_integration(
            source_point,
            source_normal,
            &coords_j,
            field_elem_j.element_type,
            field_elem_j.area,
            &physics,
            None,
            0,
            false,
        );

        // E_ji: source=j, field=i
        let field_elem_i = &mesh.elements[source_idx];
        let coords_i = mesh.element_nodes(field_elem_i);
        let result_ji = regular_integration(
            &field_elem_j.center,
            &field_elem_j.normal,
            &coords_i,
            field_elem_i.element_type,
            field_elem_i.area,
            &physics,
            None,
            0,
            false,
        );

        let e_ij = result_ij.d2g_dnxdny_integral;
        let e_ji = result_ji.d2g_dnxdny_integral;
        let diff = (e_ij - e_ji).norm();

        println!(
            "E[0,{}]: {:.4}+{:.4}i, E[{},0]: {:.4}+{:.4}i, |diff|={:.4}",
            j, e_ij.re, e_ij.im, j, e_ji.re, e_ji.im, diff
        );
    }

    // Check: compute (n_i · n_j) distribution
    println!(
        "\n--- Normal dot products (n_i · n_j) for source {} ---",
        source_idx
    );
    let mut nx_ny_sum = 0.0;
    for j in 0..mesh.elements.len() {
        let nj = &mesh.elements[j].normal;
        let dot = source_normal[0] * nj[0] + source_normal[1] * nj[1] + source_normal[2] * nj[2];
        nx_ny_sum += dot;
    }
    println!("Sum of (n_0 · n_j) over all j: {:.4}", nx_ny_sum);
    println!("(This should be ~0 for a closed sphere due to symmetry)");
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
