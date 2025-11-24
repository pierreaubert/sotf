//! Test Hartmann 4-D function to find the correct implementation and minimum

use autoeq_testfunctions::*;
use ndarray::Array1;

fn main() {
    println!("Testing Hartmann 4-D function:");
    println!("{}", "=".repeat(50));

    // Test at the supposed global minimum
    let x_min = Array1::from_vec(vec![0.1873, 0.1906, 0.5566, 0.2647]);
    let f_min = hartman_4d(&x_min);
    println!(
        "At supposed minimum x = [{:.4}, {:.4}, {:.4}, {:.4}]",
        x_min[0], x_min[1], x_min[2], x_min[3]
    );
    println!("f(x) = {:.6} (expected ≈ -3.135)", f_min);
    println!();

    // Test at some other known points for Hartmann functions
    let test_points = vec![
        vec![0.20169, 0.150011, 0.476874, 0.275332], // From 6D version adapted
        vec![0.1, 0.2, 0.3, 0.4],
        vec![0.5, 0.5, 0.5, 0.5],
        vec![0.0, 0.0, 0.0, 0.0],
        vec![1.0, 1.0, 1.0, 1.0],
    ];

    println!("Function evaluation at various test points:");
    for (i, point) in test_points.iter().enumerate() {
        let x = Array1::from_vec(point.clone());
        let f = hartman_4d(&x);
        println!(
            "Point {}: x = [{:.4}, {:.4}, {:.4}, {:.4}] → f(x) = {:.6}",
            i + 1,
            x[0],
            x[1],
            x[2],
            x[3],
            f
        );
    }

    // Grid search to find minimum
    println!("\nGrid search for minimum:");
    let mut min_val = f64::INFINITY;
    let mut min_x = vec![0.0; 4];

    let steps = 20; // 20^4 = 160k evaluations
    for i in 0..steps {
        for j in 0..steps {
            for k in 0..steps {
                for l in 0..steps {
                    let x_vals = vec![
                        i as f64 / (steps - 1) as f64,
                        j as f64 / (steps - 1) as f64,
                        k as f64 / (steps - 1) as f64,
                        l as f64 / (steps - 1) as f64,
                    ];
                    let x = Array1::from_vec(x_vals.clone());
                    let f = hartman_4d(&x);

                    if f < min_val {
                        min_val = f;
                        min_x = x_vals;
                    }
                }
            }
        }
    }

    println!("Grid search minimum found:");
    println!(
        "x = [{:.4}, {:.4}, {:.4}, {:.4}]",
        min_x[0], min_x[1], min_x[2], min_x[3]
    );
    println!("f(x) = {:.6}", min_val);
}
