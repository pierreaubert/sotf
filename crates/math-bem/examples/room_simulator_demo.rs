//! Room Acoustics Simulator Demo
//!
//! Demonstrates the room acoustics BEM simulator with:
//! - Rectangular room with dimensions
//! - Sound sources with directivity patterns
//! - Listening positions
//! - Frequency response calculation
//! - Horizontal and vertical contour plots

use math_audio_bem::room_acoustics::*;
use ndarray::Array2;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Room Acoustics Simulator Demo ===\n");

    // Define a typical listening room (5m x 4m x 2.5m)
    let room = RectangularRoom::new(5.0, 4.0, 2.5);
    let room_geom = RoomGeometry::Rectangular(room.clone());
    println!(
        "Room dimensions: {:.1}m x {:.1}m x {:.1}m",
        room.width, room.depth, room.height
    );

    // Create an omnidirectional source at front center
    let source_position = Point3D::new(2.5, 0.5, 1.2); // Center front, ear height
    let directivity = DirectivityPattern::omnidirectional();
    let source = Source::new(source_position, directivity, 1.0);
    println!(
        "Source position: ({:.1}, {:.1}, {:.1})",
        source.position.x, source.position.y, source.position.z
    );

    // Define listening position at center of room
    let lp = Point3D::new(2.5, 2.0, 1.2); // Center, ear height
    println!(
        "Listening position: ({:.1}, {:.1}, {:.1})",
        lp.x, lp.y, lp.z
    );

    // Create simulation
    let simulation = RoomSimulation::new(room_geom, vec![source], vec![lp]);

    println!(
        "\nFrequency range: {:.0} Hz to {:.0} kHz ({} points)",
        simulation.frequencies[0],
        simulation.frequencies.last().unwrap() / 1000.0,
        simulation.frequencies.len()
    );

    // Generate room mesh
    println!("\nGenerating room mesh...");
    let mesh = room.generate_mesh(2); // 2 elements per meter
    println!(
        "Mesh: {} nodes, {} elements",
        mesh.nodes.len(),
        mesh.elements.len()
    );

    // Simulate at a few frequencies for demonstration
    let demo_frequencies = vec![100.0, 500.0, 1000.0, 5000.0];
    println!(
        "\nSimulating at demo frequencies: {:?} Hz",
        demo_frequencies
    );

    for freq in &demo_frequencies {
        let k = simulation.wavenumber(*freq);
        println!("\nFrequency: {:.0} Hz (k = {:.3})", freq, k);

        // Build BEM matrix
        let matrix = build_bem_matrix(&mesh, k);
        println!("  Matrix size: {} x {}", matrix.nrows(), matrix.ncols());

        // Calculate incident field
        let incident = calculate_incident_field(&mesh, &simulation.sources, k, *freq);
        println!("  Incident field calculated at {} elements", incident.len());

        // For now, use a simplified approach: assume surface pressure equals incident
        // In full implementation, this would be solved via BEM system
        let surface_pressure = incident.clone();

        // Calculate pressure at listening position
        let field_pressure = calculate_field_pressure(
            &mesh,
            &surface_pressure,
            &simulation.sources,
            &simulation.listening_positions,
            k,
            *freq,
        );

        let spl = pressure_to_spl(field_pressure[0]);
        println!("  SPL at LP: {:.1} dB", spl);
    }

    // Compute frequency response at all 200 frequencies
    println!(
        "\nComputing frequency response at {} frequencies...",
        simulation.frequencies.len()
    );
    let mut lp_spl_values = Vec::new();

    for (idx, &freq) in simulation.frequencies.iter().enumerate() {
        if idx % 20 == 0 {
            println!(
                "  Progress: {}/{} ({:.0} Hz)",
                idx + 1,
                simulation.frequencies.len(),
                freq
            );
        }
        let k = simulation.wavenumber(freq);
        let incident = calculate_incident_field(&mesh, &simulation.sources, k, freq);

        // Calculate SPL at listening position
        let lp_pressure =
            calculate_field_pressure(&mesh, &incident, &simulation.sources, &[lp], k, freq);
        let lp_spl = pressure_to_spl(lp_pressure[0]);
        lp_spl_values.push(lp_spl);
    }

    // Generate spatial slices at all frequencies for visualization
    println!(
        "\nGenerating spatial slices at all {} frequencies...",
        simulation.frequencies.len()
    );
    let slice_frequencies = simulation.frequencies.clone();
    let slice_resolution = 50;

    let x_points: Vec<f64> = (0..slice_resolution)
        .map(|i| i as f64 * room.width / (slice_resolution - 1) as f64)
        .collect();
    let y_points: Vec<f64> = (0..slice_resolution)
        .map(|i| i as f64 * room.depth / (slice_resolution - 1) as f64)
        .collect();
    let z_points: Vec<f64> = (0..slice_resolution)
        .map(|i| i as f64 * room.height / (slice_resolution - 1) as f64)
        .collect();

    let mut horizontal_slices = Vec::new();
    let mut vertical_slices = Vec::new();

    for (idx, &freq) in slice_frequencies.iter().enumerate() {
        if idx % 20 == 0 {
            println!(
                "  Slice progress: {}/{} ({:.0} Hz)",
                idx + 1,
                slice_frequencies.len(),
                freq
            );
        }
        let k = simulation.wavenumber(freq);
        let incident = calculate_incident_field(&mesh, &simulation.sources, k, freq);

        // Horizontal slice (XY at LP.z)
        let mut h_field_points = Vec::new();
        for &y in &y_points {
            for &x in &x_points {
                h_field_points.push(Point3D::new(x, y, lp.z));
            }
        }

        let h_pressures = calculate_field_pressure(
            &mesh,
            &incident,
            &simulation.sources,
            &h_field_points,
            k,
            freq,
        );

        let mut h_spl_grid = Array2::zeros((slice_resolution, slice_resolution));
        for (idx, p) in h_pressures.iter().enumerate() {
            let i = idx % slice_resolution;
            let j = idx / slice_resolution;
            h_spl_grid[[j, i]] = pressure_to_spl(*p);
        }

        horizontal_slices.push(serde_json::json!({
            "x": x_points,
            "y": y_points,
            "spl": h_spl_grid.iter().cloned().collect::<Vec<f64>>(),
            "shape": [slice_resolution, slice_resolution],
            "frequency": freq,
        }));

        // Vertical slice (XZ at LP.y)
        let mut v_field_points = Vec::new();
        for &z in &z_points {
            for &x in &x_points {
                v_field_points.push(Point3D::new(x, lp.y, z));
            }
        }

        let v_pressures = calculate_field_pressure(
            &mesh,
            &incident,
            &simulation.sources,
            &v_field_points,
            k,
            freq,
        );

        let mut v_spl_grid = Array2::zeros((slice_resolution, slice_resolution));
        for (idx, p) in v_pressures.iter().enumerate() {
            let i = idx % slice_resolution;
            let j = idx / slice_resolution;
            v_spl_grid[[j, i]] = pressure_to_spl(*p);
        }

        vertical_slices.push(serde_json::json!({
            "x": x_points,
            "z": z_points,
            "spl": v_spl_grid.iter().cloned().collect::<Vec<f64>>(),
            "shape": [slice_resolution, slice_resolution],
            "frequency": freq,
        }));
    }

    println!(
        "Generated {} slices for {} frequencies",
        slice_resolution * slice_resolution,
        slice_frequencies.len()
    );

    // Save results to JSON
    let output = serde_json::json!({
        "room": {
            "width": room.width,
            "depth": room.depth,
            "height": room.height,
        },
        "source": {
            "position": [source_position.x, source_position.y, source_position.z],
        },
        "listening_position": [lp.x, lp.y, lp.z],
        "frequencies": slice_frequencies,  // For slice visualization
        "lp_frequency_response": {
            "frequencies": simulation.frequencies.clone(),  // All 200 frequencies
            "spl": lp_spl_values,
        },
        "horizontal_slices": horizontal_slices,
        "vertical_slices": vertical_slices,
    });

    fs::write(
        "room_sim_results.json",
        serde_json::to_string_pretty(&output)?,
    )?;
    println!("\nResults saved to: room_sim_results.json");

    println!("\n=== Simulation Complete ===");

    Ok(())
}
