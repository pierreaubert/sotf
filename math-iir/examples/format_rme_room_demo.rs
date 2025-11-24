// Example demonstrating the new peq_format_rme_room function
//
// Copyright (C) 2025 Pierre Aubert pierre(at)spinorama(dot)org
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use autoeq_iir::{Biquad, BiquadFilterType, Peq, peq_format_rme_room};

fn main() {
    // Create a sample PEQ for the left channel
    let mut left: Peq = Vec::new();

    // Add a low shelf at 100Hz
    let lowshelf = Biquad::new(BiquadFilterType::Lowshelf, 100.0, 48000.0, 0.7, -2.0);
    left.push((1.0, lowshelf));

    // Add some peak filters
    let peak1 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 3.0);
    left.push((1.0, peak1));

    let peak2 = Biquad::new(BiquadFilterType::Peak, 2000.0, 48000.0, 1.5, -1.5);
    left.push((1.0, peak2));

    // Add a high shelf at 8kHz
    let highshelf = Biquad::new(BiquadFilterType::Highshelf, 8000.0, 48000.0, 0.7, 2.0);
    left.push((1.0, highshelf));

    // Create a sample PEQ for the right channel (slightly different)
    let mut right: Peq = Vec::new();

    let lowshelf_r = Biquad::new(BiquadFilterType::Lowshelf, 100.0, 48000.0, 0.7, -1.5);
    right.push((1.0, lowshelf_r));

    let peak_r = Biquad::new(BiquadFilterType::Peak, 1500.0, 48000.0, 1.2, 2.5);
    right.push((1.0, peak_r));

    let highshelf_r = Biquad::new(BiquadFilterType::Highshelf, 8000.0, 48000.0, 0.7, 1.5);
    right.push((1.0, highshelf_r));

    println!("=== RME TotalMix Room EQ Format (Dual Channel) ===\n");
    let rme_output = peq_format_rme_room(&left, &right);
    println!("{}\n", rme_output);

    // Example with single channel (right channel will copy left)
    println!("=== RME TotalMix Room EQ Format (Single Channel) ===\n");
    let empty_right: Peq = vec![];
    let rme_output_single = peq_format_rme_room(&left, &empty_right);
    println!("{}", rme_output_single);
}
