//! Find the precise Hartmann 4-D minimum using refined search

use autoeq_testfunctions::*;
use ndarray::Array1;

fn main() {
    println!("Finding precise Hartmann 4-D minimum:");
    println!("{}", "=".repeat(50));

    // Start from our best known point and refine
    let mut best_x = vec![0.1873, 0.1906, 0.5566, 0.2647];
    let x = Array1::from_vec(best_x.clone());
    let mut best_f = hartman_4d(&x);

    println!(
        "Starting point: x = [{:.6}, {:.6}, {:.6}, {:.6}]",
        best_x[0], best_x[1], best_x[2], best_x[3]
    );
    println!("Starting value: f(x) = {:.8}", best_f);
    println!();

    // Refine search around the known minimum
    let step_size = 0.001;
    let mut improved = true;
    let mut iterations = 0;

    while improved && iterations < 1000 {
        improved = false;
        let current_x = best_x.clone();

        // Try small steps in each dimension
        for dim in 0..4 {
            for &delta in &[-step_size, step_size] {
                let mut test_x = current_x.clone();
                test_x[dim] += delta;

                // Keep in bounds [0, 1]
                if test_x[dim] >= 0.0 && test_x[dim] <= 1.0 {
                    let x = Array1::from_vec(test_x.clone());
                    let f = hartman_4d(&x);

                    if f < best_f {
                        best_x = test_x;
                        best_f = f;
                        improved = true;
                    }
                }
            }
        }

        iterations += 1;
        if iterations % 100 == 0 {
            println!("Iteration {}: f(x) = {:.8}", iterations, best_f);
        }
    }

    println!("\nRefined minimum after {} iterations:", iterations);
    println!(
        "x = [{:.6}, {:.6}, {:.6}, {:.6}]",
        best_x[0], best_x[1], best_x[2], best_x[3]
    );
    println!("f(x) = {:.8}", best_f);

    // Also try the grid search result
    let grid_x = vec![0.2105, 0.2105, 0.5789, 0.2632];
    let x = Array1::from_vec(grid_x.clone());
    let grid_f = hartman_4d(&x);

    println!("\nGrid search result:");
    println!(
        "x = [{:.6}, {:.6}, {:.6}, {:.6}]",
        grid_x[0], grid_x[1], grid_x[2], grid_x[3]
    );
    println!("f(x) = {:.8}", grid_f);

    // Compare with literature values for Hartmann 3-D to verify our implementation pattern
    println!("\nFor comparison, Hartmann 3-D:");
    let x3d = Array1::from_vec(vec![0.114614, 0.555649, 0.852547]);
    let f3d = hartman_3d(&x3d);
    println!("x = [{:.6}, {:.6}, {:.6}]", x3d[0], x3d[1], x3d[2]);
    println!("f(x) = {:.8} (expected ≈ -3.86278)", f3d);
}
