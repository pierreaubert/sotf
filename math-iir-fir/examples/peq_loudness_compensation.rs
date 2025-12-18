//! Example: Fast PEQ Loudness Compensation
//!
//! This example demonstrates how to use `peq_loudness_gain()` to maintain
//! spectral balance when applying parametric EQ filters.
//!
//! This approach is much faster than full Replay Gain analysis because it
//! analyzes the PEQ frequency response analytically rather than processing audio.
//!
//! Run with: cargo run --example peq_loudness_compensation

use autoeq_iir::{Biquad, BiquadFilterType, Peq, peq_loudness_gain, peq_preamp_gain};

fn main() {
    println!("=== PEQ Loudness Compensation Example ===\n");

    // Example 1: Single peak boost at 1kHz
    println!("Example 1: +6 dB peak at 1 kHz");
    let bq1 = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, 6.0);
    let peq1: Peq = vec![(1.0, bq1)];

    let clip_gain = peq_preamp_gain(&peq1);
    let loudness_k = peq_loudness_gain(&peq1, "k");
    let loudness_a = peq_loudness_gain(&peq1, "a");

    println!(
        "  Anti-clipping gain:     {:.2} dB (prevents clipping only)",
        clip_gain
    );
    println!(
        "  K-weighted compensation: {:.2} dB (maintains EBU R128-like loudness)",
        loudness_k
    );
    println!(
        "  A-weighted compensation: {:.2} dB (maintains A-weighted loudness)",
        loudness_a
    );
    println!();

    // Example 2: Bass boost
    println!("Example 2: +6 dB bass boost at 100 Hz");
    let bq2 = Biquad::new(BiquadFilterType::Peak, 100.0, 48000.0, 1.0, 6.0);
    let peq2: Peq = vec![(1.0, bq2)];

    let clip_gain = peq_preamp_gain(&peq2);
    let loudness_k = peq_loudness_gain(&peq2, "k");
    let loudness_a = peq_loudness_gain(&peq2, "a");

    println!("  Anti-clipping gain:     {:.2} dB", clip_gain);
    println!("  K-weighted compensation: {:.2} dB", loudness_k);
    println!("  A-weighted compensation: {:.2} dB", loudness_a);
    println!("  Note: A-weighting is less negative because bass is less perceptually important");
    println!();

    // Example 3: Complex multi-band EQ (V-shape)
    println!("Example 3: V-shaped EQ (bass & treble boost, midrange cut)");
    let bass_shelf = Biquad::new(BiquadFilterType::Lowshelf, 150.0, 48000.0, 0.7, 4.0);
    let mid_cut = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.0, -3.0);
    let treble_shelf = Biquad::new(BiquadFilterType::Highshelf, 8000.0, 48000.0, 0.7, 3.0);
    let peq3: Peq = vec![(1.0, bass_shelf), (1.0, mid_cut), (1.0, treble_shelf)];

    let clip_gain = peq_preamp_gain(&peq3);
    let loudness_k = peq_loudness_gain(&peq3, "k");
    let loudness_a = peq_loudness_gain(&peq3, "a");

    println!("  Anti-clipping gain:     {:.2} dB", clip_gain);
    println!("  K-weighted compensation: {:.2} dB", loudness_k);
    println!("  A-weighted compensation: {:.2} dB", loudness_a);
    println!();

    // Example 4: Flat response (no EQ)
    println!("Example 4: Flat response (no EQ)");
    let peq4: Peq = vec![];

    let clip_gain = peq_preamp_gain(&peq4);
    let loudness_k = peq_loudness_gain(&peq4, "k");
    let loudness_a = peq_loudness_gain(&peq4, "a");

    println!("  Anti-clipping gain:     {:.2} dB", clip_gain);
    println!("  K-weighted compensation: {:.2} dB", loudness_k);
    println!("  A-weighted compensation: {:.2} dB", loudness_a);
    println!();

    println!("=== Usage in Your Audio Pipeline ===");
    println!(
        "
When applying PEQ to audio, use both gains:
1. peq_preamp_gain()    -> Prevents clipping
2. peq_loudness_gain()  -> Maintains perceived loudness

Combined example:
  let total_gain = peq_preamp_gain(&peq) + peq_loudness_gain(&peq, \"k\");
  // Apply PEQ filters + total_gain to your audio

This is orders of magnitude faster than Replay Gain because:
- No audio file decoding required
- No sample-by-sample processing
- Just evaluates transfer functions (microseconds vs seconds)
"
    );
}
