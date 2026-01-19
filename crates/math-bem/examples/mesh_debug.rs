//! Mesh Debug - Check mesh quality and normals
//!
//! Verifies that mesh normals point outward and elements are properly formed.

#[cfg(feature = "pure-rust")]
fn main() {
    use math_audio_bem::core::mesh::generators::{generate_icosphere_mesh, generate_sphere_mesh};
    use std::f64::consts::PI;

    let radius = 0.1;

    println!("=== Mesh Quality Diagnostics ===\n");

    // UV-sphere
    println!("--- UV-sphere (8, 16) ---");
    let mesh_uv = generate_sphere_mesh(radius, 8, 16);
    check_mesh(&mesh_uv, radius, "UV-sphere");

    // Icosphere
    println!("\n--- Icosphere (subdivisions=2) ---");
    let mesh_ico = generate_icosphere_mesh(radius, 2);
    check_mesh(&mesh_ico, radius, "Icosphere");
}

#[cfg(feature = "pure-rust")]
fn check_mesh(mesh: &math_audio_bem::core::types::Mesh, radius: f64, name: &str) {
    let n = mesh.elements.len();
    println!("  {} elements, {} nodes", n, mesh.nodes.nrows());

    // Check normals
    let mut normal_errors = 0;
    let mut area_min = f64::INFINITY;
    let mut area_max = 0.0f64;
    let mut total_area = 0.0;

    for elem in &mesh.elements {
        // Normal should point outward (same direction as center for sphere)
        let n_dot_c = elem.normal[0] * elem.center[0]
            + elem.normal[1] * elem.center[1]
            + elem.normal[2] * elem.center[2];

        if n_dot_c < 0.0 {
            normal_errors += 1;
        }

        // Normal should be unit length
        let n_len =
            (elem.normal[0].powi(2) + elem.normal[1].powi(2) + elem.normal[2].powi(2)).sqrt();
        if (n_len - 1.0).abs() > 1e-6 {
            println!("  WARNING: Non-unit normal: len = {}", n_len);
        }

        area_min = area_min.min(elem.area);
        area_max = area_max.max(elem.area);
        total_area += elem.area;
    }

    let expected_area = 4.0 * std::f64::consts::PI * radius * radius;
    let area_error = (total_area - expected_area).abs() / expected_area * 100.0;

    println!("  Normal direction errors: {}", normal_errors);
    println!(
        "  Area range: {:.6e} to {:.6e} (ratio = {:.1}x)",
        area_min,
        area_max,
        area_max / area_min
    );
    println!(
        "  Total area: {:.6e} (expected {:.6e}, error = {:.1}%)",
        total_area, expected_area, area_error
    );

    // Check element aspect ratios and show some examples
    println!("\n  First 5 elements:");
    for (i, elem) in mesh.elements.iter().take(5).enumerate() {
        let n_dot_c = elem.normal[0] * elem.center[0]
            + elem.normal[1] * elem.center[1]
            + elem.normal[2] * elem.center[2];
        let center_r =
            (elem.center[0].powi(2) + elem.center[1].powi(2) + elem.center[2].powi(2)).sqrt();

        println!(
            "    [{}] center=({:.4},{:.4},{:.4}) |center|={:.4} area={:.6e} n·c={:.4}",
            i, elem.center[0], elem.center[1], elem.center[2], center_r, elem.area, n_dot_c
        );
        println!(
            "        normal=({:.4},{:.4},{:.4})",
            elem.normal[0], elem.normal[1], elem.normal[2]
        );
    }

    // Check edge lengths for first element
    if !mesh.elements.is_empty() {
        let elem = &mesh.elements[0];
        println!("\n  First element edge lengths:");
        for i in 0..elem.connectivity.len() {
            let j = (i + 1) % elem.connectivity.len();
            let n0 = elem.connectivity[i];
            let n1 = elem.connectivity[j];
            let dx = mesh.nodes[[n1, 0]] - mesh.nodes[[n0, 0]];
            let dy = mesh.nodes[[n1, 1]] - mesh.nodes[[n0, 1]];
            let dz = mesh.nodes[[n1, 2]] - mesh.nodes[[n0, 2]];
            let edge_len = (dx * dx + dy * dy + dz * dz).sqrt();
            println!("    Edge {}-{}: {:.6e}", i, j, edge_len);
        }
    }
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
