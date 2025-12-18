//! Test Gramacy & Lee function to find the correct implementation

use autoeq_testfunctions::*;
use ndarray::Array1;

fn main() {
    println!("Testing Gramacy & Lee function at various points:");

    // Test at the supposed global minimum
    let x_min = Array1::from_vec(vec![0.548563444114526]);
    let f_min = gramacy_lee_2012(&x_min);
    println!(
        "At supposed minimum x = {:.6}: f(x) = {:.6}",
        x_min[0], f_min
    );

    // Test at various points to understand the function behavior
    let test_points = vec![0.5, 0.6, 0.7, 0.8, 0.9, 1.0, 1.5, 2.0, 2.5];

    println!("\nFunction evaluation at various points:");
    for &x_val in &test_points {
        let x = Array1::from_vec(vec![x_val]);
        let f = gramacy_lee_2012(&x);
        println!("x = {:.1}: f(x) = {:.6}", x_val, f);
    }

    // Find minimum in the range
    let mut min_val = f64::INFINITY;
    let mut min_x = 0.0;

    for i in 0..1000 {
        let x_val = 0.5 + (i as f64) * (2.0 / 1000.0); // from 0.5 to 2.5
        let x = Array1::from_vec(vec![x_val]);
        let f = gramacy_lee_2012(&x);

        if f < min_val {
            min_val = f;
            min_x = x_val;
        }
    }

    println!("\nGrid search minimum:");
    println!("x = {:.6}: f(x) = {:.6}", min_x, min_val);
}
