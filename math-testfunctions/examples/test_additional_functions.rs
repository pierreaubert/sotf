//! Test script for additional SFU optimization functions
//!
//! This example tests all the newly added functions beyond the core SFU set

use autoeq_testfunctions::*;
use ndarray::Array1;

fn main() {
    println!("Testing additional optimization functions:");
    println!("{}", "=".repeat(60));

    // Test Xin-She Yang N.1 Function - 2D
    let x_xsy1 = Array1::from_vec(vec![0.0, 0.0]);
    let f_xsy1 = xin_she_yang_n1(&x_xsy1);
    println!("Xin-She Yang N.1 Function:");
    println!("  x = [{:.1}, {:.1}]", x_xsy1[0], x_xsy1[1]);
    println!("  f(x) = {:.6} (expected = 0.0)", f_xsy1);
    println!();

    // Test Discus Function - 2D
    let x_discus = Array1::from_vec(vec![0.0, 0.0]);
    let f_discus = discus(&x_discus);
    println!("Discus Function:");
    println!("  x = [{:.1}, {:.1}]", x_discus[0], x_discus[1]);
    println!("  f(x) = {:.6} (expected = 0.0)", f_discus);

    // Test with non-zero to see ill-conditioning
    let x_discus2 = Array1::from_vec(vec![0.1, 0.1]);
    let f_discus2 = discus(&x_discus2);
    println!("  x = [{:.1}, {:.1}]", x_discus2[0], x_discus2[1]);
    println!("  f(x) = {:.6} (1e6*0.1² + 0.1² = 10000.01)", f_discus2);
    println!();

    // Test Elliptic Function - 2D
    let x_elliptic = Array1::from_vec(vec![0.0, 0.0]);
    let f_elliptic = elliptic(&x_elliptic);
    println!("Elliptic Function:");
    println!("  x = [{:.1}, {:.1}]", x_elliptic[0], x_elliptic[1]);
    println!("  f(x) = {:.6} (expected = 0.0)", f_elliptic);
    println!();

    // Test Cigar Function - 2D
    let x_cigar = Array1::from_vec(vec![0.0, 0.0]);
    let f_cigar = cigar(&x_cigar);
    println!("Cigar Function:");
    println!("  x = [{:.1}, {:.1}]", x_cigar[0], x_cigar[1]);
    println!("  f(x) = {:.6} (expected = 0.0)", f_cigar);
    println!();

    // Test Tablet Function - 2D
    let x_tablet = Array1::from_vec(vec![0.0, 0.0]);
    let f_tablet = tablet(&x_tablet);
    println!("Tablet Function:");
    println!("  x = [{:.1}, {:.1}]", x_tablet[0], x_tablet[1]);
    println!("  f(x) = {:.6} (expected = 0.0)", f_tablet);
    println!();

    // Test Different Powers Function - 2D
    let x_diffpow = Array1::from_vec(vec![0.0, 0.0]);
    let f_diffpow = different_powers(&x_diffpow);
    println!("Different Powers Function:");
    println!("  x = [{:.1}, {:.1}]", x_diffpow[0], x_diffpow[1]);
    println!("  f(x) = {:.6} (expected = 0.0)", f_diffpow);
    println!();

    // Test Ridge Function - 2D
    let x_ridge = Array1::from_vec(vec![0.0, 0.0]);
    let f_ridge = ridge(&x_ridge);
    println!("Ridge Function:");
    println!("  x = [{:.1}, {:.1}]", x_ridge[0], x_ridge[1]);
    println!("  f(x) = {:.6} (expected = 0.0)", f_ridge);
    println!();

    // Test Sharp Ridge Function - 2D
    let x_sharpridge = Array1::from_vec(vec![0.0, 0.0]);
    let f_sharpridge = sharp_ridge(&x_sharpridge);
    println!("Sharp Ridge Function:");
    println!("  x = [{:.1}, {:.1}]", x_sharpridge[0], x_sharpridge[1]);
    println!("  f(x) = {:.6} (expected = 0.0)", f_sharpridge);
    println!();

    // Test Katsuura Function - 2D
    let x_katsuura = Array1::from_vec(vec![0.0, 0.0]);
    let f_katsuura = katsuura(&x_katsuura);
    println!("Katsuura Function:");
    println!("  x = [{:.1}, {:.1}]", x_katsuura[0], x_katsuura[1]);
    println!("  f(x) = {:.6} (expected ≈ 1.0)", f_katsuura);
    println!();

    // Test HappyCat Function - 2D
    let x_happycat = Array1::from_vec(vec![-1.0, -1.0]);
    let f_happycat = happycat(&x_happycat);
    println!("HappyCat Function:");
    println!("  x = [{:.1}, {:.1}]", x_happycat[0], x_happycat[1]);
    println!("  f(x) = {:.6} (expected = 0.0)", f_happycat);
    println!();

    // Test Expanded Griewank-Rosenbrock Function - 2D
    let x_egr = Array1::from_vec(vec![1.0, 1.0]);
    let f_egr = expanded_griewank_rosenbrock(&x_egr);
    println!("Expanded Griewank-Rosenbrock Function:");
    println!("  x = [{:.1}, {:.1}]", x_egr[0], x_egr[1]);
    println!("  f(x) = {:.6} (expected ≈ 0.0)", f_egr);
    println!();

    // Test Alternative Gramacy & Lee Function - 1D
    let x_gramacy_alt = Array1::from_vec(vec![0.3]);
    let f_gramacy_alt = gramacy_lee_function(&x_gramacy_alt);
    println!("Alternative Gramacy & Lee Function:");
    println!("  x = [{:.1}]", x_gramacy_alt[0]);
    println!("  f(x) = {:.6}", f_gramacy_alt);
    println!();

    // Test function metadata retrieval for new functions
    println!("Testing function metadata for additional functions:");
    let metadata = get_function_metadata();

    let new_functions = [
        "xin_she_yang_n1",
        "discus",
        "elliptic",
        "cigar",
        "tablet",
        "different_powers",
        "ridge",
        "sharp_ridge",
        "katsuura",
        "happycat",
        "expanded_griewank_rosenbrock",
    ];

    for func_name in &new_functions {
        if let Some(meta) = metadata.get(*func_name) {
            println!(
                "  {}: {} ({} dimensions available)",
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

    println!("\nTotal functions in metadata: {}", metadata.len());
}
