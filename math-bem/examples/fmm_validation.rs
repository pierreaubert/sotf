//! FMM Validation: Compare SLFMM matvec vs direct TBEM
//!
//! This validates that the FMM implementation produces the same
//! results as the direct TBEM assembly for matrix-vector products.

#[cfg(feature = "pure-rust")]
fn main() {
    use bem::core::assembly::slfmm::build_slfmm_system;
    use bem::core::assembly::tbem::build_tbem_system_scaled;
    use bem::core::mesh::generators::generate_icosphere_mesh;
    use bem::core::types::{BoundaryCondition, Cluster, PhysicsParams};
    use ndarray::Array1;
    use num_complex::Complex64;
    use std::f64::consts::PI;
    use std::time::Instant;

    println!("=== FMM Validation: SLFMM vs TBEM ===\n");

    let radius = 0.1;
    let speed_of_sound = 343.0;
    let density = 1.21;

    // Test different mesh sizes
    let subdivisions = [2]; // Start with small mesh

    for &subdiv in &subdivisions {
        let mesh = generate_icosphere_mesh(radius, subdiv);
        let n = mesh.elements.len();

        println!("Mesh: {} elements (subdivision {})", n, subdiv);

        // Prepare elements
        let mut elements = mesh.elements.clone();
        for (i, elem) in elements.iter_mut().enumerate() {
            elem.boundary_condition = BoundaryCondition::Velocity(vec![Complex64::new(0.0, 0.0)]);
            elem.dof_addresses = vec![i];
        }

        // Test at ka = 1.0
        let ka = 1.0;
        let freq = ka * speed_of_sound / (2.0 * PI * radius);
        let physics = PhysicsParams::new(freq, speed_of_sound, density, false);
        let scale = 1.0;

        println!("  ka = {:.1}, freq = {:.0} Hz", ka, freq);

        // Build TBEM system
        let start = Instant::now();
        let tbem_system = build_tbem_system_scaled(&elements, &mesh.nodes, &physics, scale);
        let tbem_time = start.elapsed();
        println!("  TBEM assembly: {:.2}ms", tbem_time.as_secs_f64() * 1000.0);

        // Create a single cluster containing all elements (simplest case for validation)
        let mut cluster = Cluster::new(Array1::from_vec(vec![0.0, 0.0, 0.0]));
        cluster.element_indices = (0..n).collect();
        cluster.radius = radius * 2.0;
        cluster.near_clusters = vec![]; // Self-interaction is always near
        cluster.far_clusters = vec![];
        let clusters = vec![cluster];
        println!(
            "  Clusters: {} (all elements in one cluster for validation)",
            clusters.len()
        );

        // Build SLFMM system
        let start = Instant::now();
        let slfmm_system = build_slfmm_system(
            &elements,
            &mesh.nodes,
            &clusters,
            &physics,
            8,  // n_theta
            16, // n_phi
            10, // n_terms
        );
        let slfmm_time = start.elapsed();
        println!(
            "  SLFMM assembly: {:.2}ms",
            slfmm_time.as_secs_f64() * 1000.0
        );

        // Create test vectors
        let mut x_real = Array1::<Complex64>::zeros(n);
        let mut x_imag = Array1::<Complex64>::zeros(n);
        let mut x_rand = Array1::<Complex64>::zeros(n);

        for i in 0..n {
            x_real[i] = Complex64::new(1.0, 0.0);
            x_imag[i] = Complex64::new(0.0, 1.0);
            x_rand[i] = Complex64::new((i as f64 * 0.1).sin(), (i as f64 * 0.2).cos());
        }

        // Compare matvec results
        println!("\n  Matvec comparison:");

        for (name, x) in [("ones", &x_real), ("i*ones", &x_imag), ("random", &x_rand)] {
            // TBEM matvec (dense)
            let start = Instant::now();
            let y_tbem = tbem_system.matrix.dot(x);
            let tbem_mv_time = start.elapsed();

            // SLFMM matvec
            let start = Instant::now();
            let y_slfmm = slfmm_system.matvec(x);
            let slfmm_mv_time = start.elapsed();

            // Compare results
            let diff: Array1<Complex64> = &y_tbem - &y_slfmm;
            let diff_norm: f64 = diff.iter().map(|d| d.norm_sqr()).sum::<f64>().sqrt();
            let y_norm: f64 = y_tbem.iter().map(|y| y.norm_sqr()).sum::<f64>().sqrt();
            let rel_error = diff_norm / y_norm.max(1e-15);

            let status = if rel_error < 0.01 {
                "✓"
            } else if rel_error < 0.1 {
                "~"
            } else {
                "✗"
            };

            println!(
                "    {:<8}: rel_error={:.2e} {} (TBEM:{:.2}ms, SLFMM:{:.2}ms)",
                name,
                rel_error,
                status,
                tbem_mv_time.as_secs_f64() * 1000.0,
                slfmm_mv_time.as_secs_f64() * 1000.0,
            );
        }

        println!();
    }

    println!("=== Analysis ===");
    println!("If relative errors are <1%, SLFMM near-field is correct.");
    println!("A single cluster with all near-field tests the core SLFMM logic.");
    println!("Multi-cluster tests would validate far-field (D-matrix) translations.");
}

#[cfg(not(feature = "pure-rust"))]
fn main() {
    eprintln!("This example requires the 'pure-rust' feature.");
}
