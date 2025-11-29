//! Normal Check - Verify element normals point outward
//!
//! For a sphere centered at origin, outward normal at (x,y,z) should be (x,y,z)/r

#[cfg(feature = "pure-rust")]
fn main() {
    use bem::core::mesh::generators::{generate_icosphere_mesh, generate_sphere_mesh};
    use std::f64::consts::PI;

    let radius = 0.1;

    println!("=== Normal Direction Check ===\n");

    // Check UV-sphere
    println!("--- UV-sphere mesh ---");
    let mesh_uv = generate_sphere_mesh(radius, 4, 8);
    check_normals(&mesh_uv, radius, "UV-sphere");

    // Check Icosphere
    println!("\n--- Icosphere mesh ---");
    let mesh_ico = generate_icosphere_mesh(radius, 1);
    check_normals(&mesh_ico, radius, "Icosphere");
}

#[cfg(feature = "pure-rust")]
fn check_normals(mesh: &bem::core::types::Mesh, radius: f64, name: &str) {
    let mut inward_count = 0;
    let mut outward_count = 0;

    for elem in &mesh.elements {
        let cx = elem.center[0];
        let cy = elem.center[1];
        let cz = elem.center[2];

        // Expected outward normal: center / |center|
        let r = (cx * cx + cy * cy + cz * cz).sqrt();
        let expected_nx = cx / r;
        let expected_ny = cy / r;
        let expected_nz = cz / r;

        // Dot product to check alignment
        let dot = elem.normal[0] * expected_nx
            + elem.normal[1] * expected_ny
            + elem.normal[2] * expected_nz;

        if dot > 0.0 {
            outward_count += 1;
        } else {
            inward_count += 1;
        }
    }

    println!("{}: {} elements", name, mesh.elements.len());
    println!("  Outward normals: {}", outward_count);
    println!("  Inward normals: {}", inward_count);

    // Sample a few elements
    println!("\n  Sample elements:");
    for (i, elem) in mesh.elements.iter().enumerate().take(5) {
        let cx = elem.center[0];
        let cy = elem.center[1];
        let cz = elem.center[2];
        let r = (cx * cx + cy * cy + cz * cz).sqrt();
        let expected_nx = cx / r;
        let expected_ny = cy / r;
        let expected_nz = cz / r;
        let dot = elem.normal[0] * expected_nx
            + elem.normal[1] * expected_ny
            + elem.normal[2] * expected_nz;

        println!("  elem[{}]: center=({:.4},{:.4},{:.4})", i, cx, cy, cz);
        println!(
            "           normal=({:.4},{:.4},{:.4})",
            elem.normal[0], elem.normal[1], elem.normal[2]
        );
        println!(
            "           expected=({:.4},{:.4},{:.4})",
            expected_nx, expected_ny, expected_nz
        );
        println!(
            "           dot={:.4} ({})",
            dot,
            if dot > 0.0 { "OUTWARD" } else { "INWARD" }
        );
    }

    // Also check front (z>0) and back (z<0) normals
    println!("\n  Front elements (z > 0):");
    let front_elems: Vec<_> = mesh
        .elements
        .iter()
        .filter(|e| e.center[2] > 0.05)
        .take(3)
        .collect();
    for elem in &front_elems {
        println!(
            "    center z={:.4}, normal z={:.4}",
            elem.center[2], elem.normal[2]
        );
    }

    println!("  Back elements (z < 0):");
    let back_elems: Vec<_> = mesh
        .elements
        .iter()
        .filter(|e| e.center[2] < -0.05)
        .take(3)
        .collect();
    for elem in &back_elems {
        println!(
            "    center z={:.4}, normal z={:.4}",
            elem.center[2], elem.normal[2]
        );
    }
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
