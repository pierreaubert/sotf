//! Test script for newly added SFU optimization functions
//!
//! This example demonstrates the newly implemented functions from the SFU Virtual Library

use autoeq_testfunctions::*;
use ndarray::Array1;

fn main() {
    println!("Testing newly implemented SFU optimization functions:");
    println!("{}", "=".repeat(60));

    // Test Gramacy & Lee (2012) Function - 1D
    let x_gramacy = Array1::from_vec(vec![0.548563444114526]);
    let f_gramacy = gramacy_lee_2012(&x_gramacy);
    println!("Gramacy & Lee (2012) Function:");
    println!("  x = [{:.6}]", x_gramacy[0]);
    println!("  f(x) = {:.6} (expected ≈ -0.869011)", f_gramacy);
    println!();

    // Test Perm Function 0, d, β - 2D
    let x_perm0 = Array1::from_vec(vec![1.0, 0.5]); // (1, 1/2) for 2D
    let f_perm0 = perm_0_d_beta(&x_perm0);
    println!("Perm Function (0, d, β):");
    println!("  x = [{:.3}, {:.3}]", x_perm0[0], x_perm0[1]);
    println!("  f(x) = {:.6} (expected ≈ 0.0)", f_perm0);
    println!();

    // Test Sum Squares Function - 2D
    let x_sum_sq = Array1::from_vec(vec![0.0, 0.0]);
    let f_sum_sq = sum_squares(&x_sum_sq);
    println!("Sum Squares Function:");
    println!("  x = [{:.1}, {:.1}]", x_sum_sq[0], x_sum_sq[1]);
    println!("  f(x) = {:.6} (expected = 0.0)", f_sum_sq);

    // Test with non-zero values to see the weighted effect
    let x_sum_sq2 = Array1::from_vec(vec![1.0, 1.0]);
    let f_sum_sq2 = sum_squares(&x_sum_sq2);
    println!("  x = [{:.1}, {:.1}]", x_sum_sq2[0], x_sum_sq2[1]);
    println!("  f(x) = {:.6} (1*1² + 2*1² = 3.0)", f_sum_sq2);
    println!();

    // Test Power Sum Function - 2D
    let x_power = Array1::from_vec(vec![2.0, 1.3199]);
    let f_power = power_sum(&x_power);
    println!("Power Sum Function:");
    println!("  x = [{:.4}, {:.4}]", x_power[0], x_power[1]);
    println!("  f(x) = {:.6}", f_power);
    println!();

    // Test Forrester (2008) Function - 1D
    let x_forrester = Array1::from_vec(vec![0.757249]);
    let f_forrester = forrester_2008(&x_forrester);
    println!("Forrester et al. (2008) Function:");
    println!("  x = [{:.6}]", x_forrester[0]);
    println!("  f(x) = {:.6} (expected ≈ -6.02074)", f_forrester);
    println!();

    // Test Hartmann 4-D Function
    let x_hart4d = Array1::from_vec(vec![0.1873, 0.1936, 0.5576, 0.2647]);
    let f_hart4d = hartman_4d(&x_hart4d);
    println!("Hartmann 4-D Function:");
    println!(
        "  x = [{:.4}, {:.4}, {:.4}, {:.4}]",
        x_hart4d[0], x_hart4d[1], x_hart4d[2], x_hart4d[3]
    );
    println!("  f(x) = {:.6} (expected ≈ -3.72983)", f_hart4d);
    println!();

    // Test Perm Function d, β - 2D
    let x_permd = Array1::from_vec(vec![1.0, 0.5]);
    let f_permd = perm_d_beta(&x_permd);
    println!("Perm Function (d, β):");
    println!("  x = [{:.3}, {:.3}]", x_permd[0], x_permd[1]);
    println!("  f(x) = {:.6} (expected ≈ 0.0)", f_permd);
    println!();

    // Test Shekel Function - 4D
    let x_shekel = Array1::from_vec(vec![4.0, 4.0, 4.0, 4.0]);
    let f_shekel = shekel(&x_shekel);
    println!("Shekel Function:");
    println!(
        "  x = [{:.1}, {:.1}, {:.1}, {:.1}]",
        x_shekel[0], x_shekel[1], x_shekel[2], x_shekel[3]
    );
    println!("  f(x) = {:.6} (expected ≈ -10.5364)", f_shekel);
    println!();

    // Test function metadata retrieval
    println!("Testing function metadata:");
    let metadata = get_function_metadata();
    for func_name in ["gramacy_lee_2012", "sum_squares", "power_sum", "shekel"] {
        if let Some(meta) = metadata.get(func_name) {
            println!(
                "  {}: {} ({} dimensions)",
                meta.name,
                if meta.multimodal {
                    "multimodal"
                } else {
                    "unimodal"
                },
                meta.dimensions.len()
            );
        }
    }
}
