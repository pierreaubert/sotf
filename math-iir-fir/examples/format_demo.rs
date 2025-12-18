// Example demonstrating the new peq_format_rme and peq_format_aupreset functions
//
// Copyright (C) 2025 Pierre Aubert pierre(at)spinorama(dot)org
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

use autoeq_iir::{Biquad, BiquadFilterType, Peq, peq_format_aupreset, peq_format_rme_channel};

fn main() {
    // Create a sample PEQ with a few filters
    let mut peq: Peq = Vec::new();

    // Add a low shelf at 100Hz
    let lowshelf = Biquad::new(BiquadFilterType::Lowshelf, 100.0, 48000.0, 0.7, -2.0);
    peq.push((1.0, lowshelf));

    // Add a peak filter at 1kHz
    let peak = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 3.0);
    peq.push((1.0, peak));

    // Add a high shelf at 8kHz
    let highshelf = Biquad::new(BiquadFilterType::Highshelf, 8000.0, 48000.0, 0.7, 2.0);
    peq.push((1.0, highshelf));

    println!("=== RME TotalMix Channel Format ===\n");
    let rme_output = peq_format_rme_channel(&peq);
    println!("{}\n", rme_output);

    println!("=== Apple AUNBandEQ Format ===\n");
    let apple_output = peq_format_aupreset(&peq, "Example EQ");
    println!("{}", apple_output);
}
