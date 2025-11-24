//! Validation tool for the headphone_loss function implementation
//!
//! This tool helps verify that the headphone_loss function properly implements
//! the Olive/Harman preference prediction model for in-ear headphones.

use autoeq::Curve;
use autoeq::loss::headphone_loss;
use ndarray::Array1;

fn main() {
    println!("=== Headphone Loss Function Validation ===\n");

    // Test 1: Validate slope calculation with known linear response
    println!("Test 1: Slope Calculation Validation");
    validate_slope_calculation();

    // Test 2: Validate frequency band RMS calculations
    println!("\nTest 2: Frequency Band RMS Validation");
    validate_band_rms_calculation();

    // Test 3: Test edge cases
    println!("\nTest 3: Edge Case Validation");
    validate_edge_cases();

    // Test 4: Compare against expected behavior from paper
    println!("\nTest 4: Reference Implementation Validation");
    validate_reference_behavior();

    // Test 5: Validate weighting factors
    println!("\nTest 5: Weighting Factor Analysis");
    analyze_weighting_factors();
}

fn validate_slope_calculation() {
    // Test with perfectly linear responses to verify slope calculation
    let freq = Array1::logspace(10.0, 1.301, 4.301, 50); // 20Hz to 20kHz, 50 points

    // Test 1: Flat response (0 dB/octave)
    let flat_response = Array1::zeros(50);
    let flat_curve = Curve {
        freq: freq.clone(),
        spl: flat_response,
    };
    let flat_score = headphone_loss(&flat_curve);
    println!("  Flat response (0 dB/oct) score: {:.3}", flat_score);

    // Test 2: Ideal -1 dB/octave response
    let ideal_response = freq.mapv(|f: f64| -1.0 * f.log2() + 10.0);
    let ideal_curve = Curve {
        freq: freq.clone(),
        spl: ideal_response,
    };
    let ideal_score = headphone_loss(&ideal_curve);
    println!("  Ideal -1 dB/oct response score: {:.3}", ideal_score);

    // Test 3: -2 dB/octave response (too steep)
    let steep_response = freq.mapv(|f: f64| -2.0 * f.log2() + 10.0);
    let steep_curve = Curve {
        freq: freq.clone(),
        spl: steep_response,
    };
    let steep_score = headphone_loss(&steep_curve);
    println!("  Steep -2 dB/oct response score: {:.3}", steep_score);

    // Verify that ideal slope gets the best score for the slope component
    if ideal_score < flat_score {
        println!("  ✓ Ideal slope scores better than flat");
    } else {
        println!("  ⚠ Ideal slope should score better than flat");
    }

    if steep_score > ideal_score {
        println!("  ✓ Too steep slope scores worse than ideal");
    } else {
        println!("  ⚠ Too steep slope should score worse than ideal");
    }
}

fn validate_band_rms_calculation() {
    let freq = Array1::logspace(10.0, 1.301, 4.301, 200); // High resolution

    // Create a response with a single 3dB peak in the midrange (500-1000 Hz)
    let mut peak_response = Array1::zeros(200);
    for (i, &f) in freq.iter().enumerate() {
        if f >= 500.0 && f <= 1000.0 {
            peak_response[i] = 3.0; // 3dB peak
        }
    }

    let peak_curve = Curve {
        freq: freq.clone(),
        spl: peak_response,
    };
    let peak_score = headphone_loss(&peak_curve);

    // Compare with flat response
    let flat_response = Array1::zeros(200);
    let flat_curve = Curve {
        freq: freq.clone(),
        spl: flat_response,
    };
    let flat_score = headphone_loss(&flat_curve);

    println!("  Flat response score: {:.3}", flat_score);
    println!("  Midrange peak (3dB, 500-1000Hz) score: {:.3}", peak_score);

    if peak_score > flat_score {
        println!("  ✓ Peak correctly penalized");
    } else {
        println!("  ⚠ Peak should be penalized more than flat response");
    }

    // Test different band penalties
    test_band_specific_penalties();
}

fn test_band_specific_penalties() {
    let freq = Array1::logspace(10.0, 1.301, 4.301, 200);
    let flat_response = Array1::zeros(200);
    let flat_curve = Curve {
        freq: freq.clone(),
        spl: flat_response,
    };
    let baseline_score = headphone_loss(&flat_curve);

    // Test bands defined in the implementation
    let test_bands = [
        (20.0, 60.0, "Sub-bass"),
        (60.0, 200.0, "Bass"),
        (200.0, 500.0, "Lower mid"),
        (500.0, 1000.0, "Midrange"),
        (1000.0, 2000.0, "Upper mid"),
        (2000.0, 4000.0, "Presence"),
        (4000.0, 8000.0, "Brilliance"),
        (8000.0, 10000.0, "Upper treble"),
    ];

    for (f_low, f_high, name) in test_bands.iter() {
        // Create a 2dB deviation in this band only
        let mut test_response = Array1::zeros(200);
        for (i, &f) in freq.iter().enumerate() {
            if f >= *f_low && f <= *f_high {
                test_response[i] = 2.0;
            }
        }

        let test_curve = Curve {
            freq: freq.clone(),
            spl: test_response,
        };
        let test_score = headphone_loss(&test_curve);
        let penalty = test_score - baseline_score;

        println!(
            "    {} ({:.0}-{:.0} Hz): penalty = {:.3}",
            name, f_low, f_high, penalty
        );
    }
}

fn validate_edge_cases() {
    let freq = Array1::logspace(10.0, 1.301, 4.301, 100);

    // Test 1: Empty frequency range
    let empty_freq = Array1::from(vec![]);
    let empty_response = Array1::from(vec![]);
    let empty_curve = Curve {
        freq: empty_freq,
        spl: empty_response,
    };
    let empty_score = headphone_loss(&empty_curve);
    println!("  Empty curve score: {:.3}", empty_score);

    // Test 2: Very high deviation
    let extreme_response = Array1::from_elem(100, 20.0); // 20dB everywhere
    let extreme_curve = Curve {
        freq: freq.clone(),
        spl: extreme_response,
    };
    let extreme_score = headphone_loss(&extreme_curve);
    println!("  Extreme +20dB response score: {:.3}", extreme_score);

    // Test 3: Very low deviation
    let negative_response = Array1::from_elem(100, -20.0); // -20dB everywhere
    let negative_curve = Curve {
        freq: freq.clone(),
        spl: negative_response,
    };
    let negative_score = headphone_loss(&negative_curve);
    println!("  Extreme -20dB response score: {:.3}", negative_score);
}

fn validate_reference_behavior() {
    println!("  Analyzing implementation against reference paper expectations...");

    // According to the Olive/Harman paper, the preference model should:
    // 1. Prefer gentle downward slope (around -1 dB/octave)
    // 2. Penalize bass deviations more than treble
    // 3. Have reasonable penalty for excessive peak-to-peak variation

    let freq = Array1::logspace(10.0, 1.301, 4.301, 200);

    // Test bass vs treble sensitivity
    let mut bass_deviation = Array1::zeros(200);
    let mut treble_deviation = Array1::zeros(200);

    // 3dB deviation in bass (60-200 Hz)
    for (i, &f) in freq.iter().enumerate() {
        if f >= 60.0 && f <= 200.0 {
            bass_deviation[i] = 3.0;
        }
        if f >= 8000.0 && f <= 10000.0 {
            treble_deviation[i] = 3.0;
        }
    }

    let bass_curve = Curve {
        freq: freq.clone(),
        spl: bass_deviation,
    };
    let treble_curve = Curve {
        freq: freq.clone(),
        spl: treble_deviation,
    };
    let flat_curve = Curve {
        freq: freq.clone(),
        spl: Array1::zeros(200),
    };

    let bass_score = headphone_loss(&bass_curve);
    let treble_score = headphone_loss(&treble_curve);
    let flat_score = headphone_loss(&flat_curve);

    let bass_penalty = bass_score - flat_score;
    let treble_penalty = treble_score - flat_score;

    println!("  Bass deviation penalty: {:.3}", bass_penalty);
    println!("  Treble deviation penalty: {:.3}", treble_penalty);

    if bass_penalty > treble_penalty {
        println!("  ✓ Bass deviations penalized more than treble (expected)");
    } else {
        println!("  ⚠ Bass should be penalized more than treble according to the paper");
    }
}

fn analyze_weighting_factors() {
    println!("  Current weighting factors in implementation:");
    println!("    Slope deviation: 10.0x");
    println!("    Sub-bass (20-60 Hz): 3.0x");
    println!("    Bass (60-200 Hz): 4.0x");
    println!("    Lower mid (200-500 Hz): 5.0x");
    println!("    Midrange (500-1000 Hz): 5.0x");
    println!("    Upper mid (1000-2000 Hz): 3.0x");
    println!("    Presence (2000-4000 Hz): 2.0x");
    println!("    High frequencies: 1.5x each");
    println!("    Peak-to-peak penalty threshold: 6.0 dB");
    println!("    Peak-to-peak penalty factor: 0.5x");

    println!(
        "\n  Note: These weights are marked as 'approximations based on the paper's findings'"
    );
    println!(
        "  in the code comments. Verification against the actual paper coefficients is needed."
    );

    // Test the relative impact of different components
    test_component_impacts();
}

fn test_component_impacts() {
    let freq = Array1::logspace(10.0, 1.301, 4.301, 200);
    let flat_response = Array1::zeros(200);
    let flat_curve = Curve {
        freq: freq.clone(),
        spl: flat_response,
    };
    let baseline = headphone_loss(&flat_curve);

    println!("\n  Component impact analysis:");

    // Slope impact: Add 0.5 dB/octave deviation from ideal
    let slope_test = freq.mapv(|f: f64| -0.5 * f.log2() + 10.0); // -0.5 instead of -1.0
    let slope_curve = Curve {
        freq: freq.clone(),
        spl: slope_test,
    };
    let slope_impact = headphone_loss(&slope_curve) - baseline;
    println!("    0.5 dB/oct slope deviation impact: {:.3}", slope_impact);

    // Band RMS impact: 1 dB RMS in midrange
    let mut rms_test = Array1::zeros(200);
    for (i, &f) in freq.iter().enumerate() {
        if f >= 500.0 && f <= 1000.0 {
            rms_test[i] = 1.0;
        }
    }
    let rms_curve = Curve {
        freq: freq.clone(),
        spl: rms_test,
    };
    let rms_impact = headphone_loss(&rms_curve) - baseline;
    println!("    1 dB RMS midrange deviation impact: {:.3}", rms_impact);

    // Peak-to-peak impact: 8 dB peak-to-peak in one band
    let mut pp_test = Array1::zeros(200);
    for (i, &f) in freq.iter().enumerate() {
        if f >= 2000.0 && f <= 4000.0 {
            if i % 4 == 0 {
                pp_test[i] = 4.0;
            }
            if i % 4 == 2 {
                pp_test[i] = -4.0;
            }
        }
    }
    let pp_curve = Curve {
        freq: freq.clone(),
        spl: pp_test,
    };
    let pp_impact = headphone_loss(&pp_curve) - baseline;
    println!("    8 dB peak-to-peak variation impact: {:.3}", pp_impact);
}
