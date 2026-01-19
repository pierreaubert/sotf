//! Room Acoustics Simulator with Dynamic Configuration
//!
//! Demonstrates configurable room simulator with:
//! - Rectangular or L-shaped rooms
//! - Multiple sources with crossovers
//! - Configurable frequency resolution
//!
//! Usage:
//!   cargo run --release --example room_simulator_config -- rectangular
//!   cargo run --release --example room_simulator_config -- lshaped
//!   cargo run --release --example room_simulator_config -- multi-source

use math_audio_bem::room_acoustics::*;
use ndarray::Array2;
use std::env;
use std::fs;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    let config_type = if args.len() > 1 {
        args[1].as_str()
    } else {
        "rectangular"
    };

    match config_type {
        "rectangular" => run_rectangular_room(),
        "lshaped" => run_lshaped_room(),
        "multi-source" => run_multi_source_system(),
        _ => {
            println!("Unknown configuration: {}", config_type);
            println!("Options: rectangular, lshaped, multi-source");
            Ok(())
        }
    }
}

fn run_rectangular_room() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Rectangular Room Simulation ===\n");

    // Define a typical listening room (5m x 4m x 2.5m)
    let room = RectangularRoom::new(5.0, 4.0, 2.5);
    let room_geom = RoomGeometry::Rectangular(room);

    // Single full-range source at front center
    let source = Source::new(
        Point3D::new(2.5, 0.5, 1.2),
        DirectivityPattern::omnidirectional(),
        1.0,
    )
    .with_name("Main Speaker".to_string());

    // Listening position
    let lp = Point3D::new(2.5, 2.0, 1.2);

    // Create simulation with custom frequency range: 50Hz to 10kHz, 100 points
    let simulation = RoomSimulation::with_frequencies(
        room_geom,
        vec![source],
        vec![lp],
        50.0,    // min frequency
        10000.0, // max frequency
        100,     // number of points
    );

    run_simulation(simulation, "rectangular_room")?;

    Ok(())
}

fn run_lshaped_room() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== L-Shaped Room Simulation ===\n");

    // L-shaped room: main section 6x4m, extension 3x3m, height 2.5m
    let room = LShapedRoom::new(
        6.0, // main width
        4.0, // main depth
        3.0, // extension width
        3.0, // extension depth
        2.5, // height
    );
    let room_geom = RoomGeometry::LShaped(room);

    println!("Room: L-shaped");
    println!("  Main section: 6.0m x 4.0m");
    println!("  Extension: 3.0m x 3.0m");
    println!("  Height: 2.5m");

    // Source in main section
    let source = Source::new(
        Point3D::new(3.0, 0.5, 1.2),
        DirectivityPattern::omnidirectional(),
        1.0,
    )
    .with_name("Main Speaker".to_string());

    // Listening position in main section
    let lp = Point3D::new(3.0, 2.5, 1.2);

    // Standard frequency range
    let simulation =
        RoomSimulation::with_frequencies(room_geom, vec![source], vec![lp], 20.0, 20000.0, 200);

    run_simulation(simulation, "lshaped_room")?;

    Ok(())
}

fn run_multi_source_system() -> Result<(), Box<dyn std::error::Error>> {
    println!("=== Multi-Source System (Stereo + Subwoofer) ===\n");

    // Rectangular room
    let room = RectangularRoom::new(5.0, 4.0, 2.5);
    let room_geom = RoomGeometry::Rectangular(room);

    // Left main speaker with highpass (80Hz, 4th order)
    let left_main = Source::new(
        Point3D::new(1.5, 0.5, 1.0),
        DirectivityPattern::omnidirectional(),
        0.7,
    )
    .with_name("Left Main".to_string())
    .with_crossover(CrossoverFilter::Highpass {
        cutoff_freq: 80.0,
        order: 4,
    });

    // Right main speaker with highpass (80Hz, 4th order)
    let right_main = Source::new(
        Point3D::new(3.5, 0.5, 1.0),
        DirectivityPattern::omnidirectional(),
        0.7,
    )
    .with_name("Right Main".to_string())
    .with_crossover(CrossoverFilter::Highpass {
        cutoff_freq: 80.0,
        order: 4,
    });

    // Subwoofer with lowpass (80Hz, 4th order)
    let subwoofer = Source::new(
        Point3D::new(2.5, 0.3, 0.3),
        DirectivityPattern::omnidirectional(),
        1.0,
    )
    .with_name("Subwoofer".to_string())
    .with_crossover(CrossoverFilter::Lowpass {
        cutoff_freq: 80.0,
        order: 4,
    });

    println!("Sources:");
    println!(
        "  Left Main: ({:.1}, {:.1}, {:.1}) - HPF 80Hz",
        left_main.position.x, left_main.position.y, left_main.position.z
    );
    println!(
        "  Right Main: ({:.1}, {:.1}, {:.1}) - HPF 80Hz",
        right_main.position.x, right_main.position.y, right_main.position.z
    );
    println!(
        "  Subwoofer: ({:.1}, {:.1}, {:.1}) - LPF 80Hz",
        subwoofer.position.x, subwoofer.position.y, subwoofer.position.z
    );

    // Listening position
    let lp = Point3D::new(2.5, 2.0, 1.2);

    // Focus on bass region: 20Hz to 500Hz, 150 points
    let simulation = RoomSimulation::with_frequencies(
        room_geom,
        vec![left_main, right_main, subwoofer],
        vec![lp],
        20.0,
        500.0,
        150,
    );

    run_simulation(simulation, "multi_source_system")?;

    Ok(())
}

fn run_simulation(
    simulation: RoomSimulation,
    output_name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    println!("\nSimulation Configuration:");
    println!("  Sources: {}", simulation.sources.len());
    for source in &simulation.sources {
        println!(
            "    - {}: ({:.1}, {:.1}, {:.1})",
            source.name, source.position.x, source.position.y, source.position.z
        );
    }
    println!(
        "  Listening positions: {}",
        simulation.listening_positions.len()
    );
    println!(
        "  Frequency range: {:.0} Hz to {:.0} Hz ({} points)",
        simulation.frequencies[0],
        simulation.frequencies.last().unwrap(),
        simulation.frequencies.len()
    );

    // Generate mesh
    println!("\nGenerating room mesh...");
    let mesh = simulation.room.generate_mesh(2); // 2 elements per meter
    println!(
        "Mesh: {} nodes, {} elements",
        mesh.nodes.len(),
        mesh.elements.len()
    );

    // Get room edges for visualization
    let edges = simulation.room.get_edges();

    let lp = simulation.listening_positions[0];

    // Compute frequency response
    println!("\nComputing frequency response...");
    let mut lp_spl_values = Vec::new();

    for (idx, &freq) in simulation.frequencies.iter().enumerate() {
        if idx % 20 == 0 || simulation.frequencies.len() < 50 {
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

    // Generate spatial slices (every 10th frequency for efficiency)
    println!("\nGenerating spatial slices...");
    let slice_indices: Vec<usize> = (0..simulation.frequencies.len())
        .step_by((simulation.frequencies.len() / 20).max(1))
        .collect();

    println!(
        "Computing {} slices out of {} frequencies",
        slice_indices.len(),
        simulation.frequencies.len()
    );

    let slice_resolution = 50;

    // Determine spatial bounds from room geometry
    let (room_width, room_depth, room_height) = match &simulation.room {
        RoomGeometry::Rectangular(r) => (r.width, r.depth, r.height),
        RoomGeometry::LShaped(r) => (r.width1.max(r.width2), r.depth1 + r.depth2, r.height),
    };

    let x_points: Vec<f64> = (0..slice_resolution)
        .map(|i| i as f64 * room_width / (slice_resolution - 1) as f64)
        .collect();
    let y_points: Vec<f64> = (0..slice_resolution)
        .map(|i| i as f64 * room_depth / (slice_resolution - 1) as f64)
        .collect();
    let z_points: Vec<f64> = (0..slice_resolution)
        .map(|i| i as f64 * room_height / (slice_resolution - 1) as f64)
        .collect();

    let mut horizontal_slices = Vec::new();
    let mut vertical_slices = Vec::new();

    for &idx in &slice_indices {
        let freq = simulation.frequencies[idx];
        println!("  Slice at {:.0} Hz", freq);

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
        for (pidx, p) in h_pressures.iter().enumerate() {
            let i = pidx % slice_resolution;
            let j = pidx / slice_resolution;
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
        for (pidx, p) in v_pressures.iter().enumerate() {
            let i = pidx % slice_resolution;
            let j = pidx / slice_resolution;
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

    // Save results to JSON
    let output = serde_json::json!({
        "room": {
            "type": match simulation.room {
                RoomGeometry::Rectangular(_) => "rectangular",
                RoomGeometry::LShaped(_) => "lshaped",
            },
            "width": room_width,
            "depth": room_depth,
            "height": room_height,
            "edges": edges.iter().map(|(p1, p2)| {
                vec![
                    vec![p1.x, p1.y, p1.z],
                    vec![p2.x, p2.y, p2.z],
                ]
            }).collect::<Vec<_>>(),
        },
        "sources": simulation.sources.iter().map(|s| {
            serde_json::json!({
                "name": s.name,
                "position": [s.position.x, s.position.y, s.position.z],
                "crossover": match &s.crossover {
                    CrossoverFilter::FullRange => "fullrange".to_string(),
                    CrossoverFilter::Lowpass { cutoff_freq, order } =>
                        format!("lowpass_{:.0}Hz_{}order", cutoff_freq, order),
                    CrossoverFilter::Highpass { cutoff_freq, order } =>
                        format!("highpass_{:.0}Hz_{}order", cutoff_freq, order),
                    CrossoverFilter::Bandpass { low_cutoff, high_cutoff, order } =>
                        format!("bandpass_{:.0}-{:.0}Hz_{}order", low_cutoff, high_cutoff, order),
                },
            })
        }).collect::<Vec<_>>(),
        "listening_position": [lp.x, lp.y, lp.z],
        "frequencies": simulation.frequencies,
        "frequency_response": lp_spl_values,
        "horizontal_slices": horizontal_slices,
        "vertical_slices": vertical_slices,
    });

    let output_path = format!("plotting/{}_sim.json", output_name);
    fs::write(&output_path, serde_json::to_string_pretty(&output)?)?;
    println!("\nResults saved to {}", output_path);
    println!("Open plotting/plot_room_sim.html in a browser to visualize");

    Ok(())
}
