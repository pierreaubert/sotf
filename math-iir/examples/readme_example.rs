//! Example demonstrating the usage shown in README.md

use autoeq_iir::*;
use ndarray::Array1;

fn create_studio_eq() -> Peq {
    let mut peq = Vec::new();

    // High-pass filter to remove subsonic content
    let hp = peq_butterworth_highpass(2, 20.0, 48000.0);
    peq.extend(hp);

    // Presence boost
    let presence = Biquad::new(BiquadFilterType::Peak, 3000.0, 48000.0, 1.2, 2.5);
    peq.push((1.0, presence));

    // Air band enhancement
    let air = Biquad::new(BiquadFilterType::Highshelf, 10000.0, 48000.0, 0.9, 1.5);
    peq.push((1.0, air));

    peq
}

fn analyze_eq(peq: &Peq) {
    // Generate frequency sweep
    let freqs = Array1::logspace(10.0, 20.0_f64.log10(), 20000.0_f64.log10(), 200);

    // Calculate response
    let response = peq_spl(&freqs, peq);

    // Find peak response
    let max_gain = response.iter().fold(f64::NEG_INFINITY, |a, &b| a.max(b));
    let min_gain = response.iter().fold(f64::INFINITY, |a, &b| a.min(b));

    println!("EQ Analysis:");
    println!("  Peak gain: {:.2} dB", max_gain);
    println!("  Min gain: {:.2} dB", min_gain);
    println!("  Dynamic range: {:.2} dB", max_gain - min_gain);
    println!("  Recommended preamp: {:.2} dB", peq_preamp_gain(peq));
}

fn main() {
    println!("AutoEQ IIR - README Example");
    println!("===========================");

    // Basic biquad filter example
    println!("\n1. Basic Biquad Filter:");
    let filter = Biquad::new(
        BiquadFilterType::Peak,
        1000.0,  // frequency
        48000.0, // sample rate
        1.0,     // Q factor
        3.0,     // gain in dB
    );

    // Calculate frequency response at 1kHz
    let response_db = filter.log_result(1000.0);
    println!("   Response at 1kHz: {:.2} dB", response_db);

    // PEQ example
    println!("\n2. Parametric EQ Example:");
    let mut peq: Peq = Vec::new();

    // Add filters
    let hp = Biquad::new(BiquadFilterType::Highpass, 80.0, 48000.0, 0.707, 0.0);
    peq.push((1.0, hp));

    let peak = Biquad::new(BiquadFilterType::Peak, 1000.0, 48000.0, 1.5, 4.0);
    peq.push((1.0, peak));

    let hs = Biquad::new(BiquadFilterType::Highshelf, 8000.0, 48000.0, 0.8, -2.0);
    peq.push((1.0, hs));

    let preamp = peq_preamp_gain(&peq);
    println!("   Recommended preamp: {:.1} dB", preamp);

    // Filter design example
    println!("\n3. Filter Design:");
    let lp_filter = peq_butterworth_lowpass(4, 2000.0, 48000.0);
    println!("   Butterworth LP has {} sections", lp_filter.len());

    let hp_filter = peq_linkwitzriley_highpass(4, 2000.0, 48000.0);
    println!("   LR HP has {} sections", hp_filter.len());

    // Advanced studio EQ example
    println!("\n4. Studio EQ Analysis:");
    let studio_eq = create_studio_eq();
    analyze_eq(&studio_eq);

    // Export APO config
    println!("\n5. EqualizerAPO Configuration:");
    let config = peq_format_apo("Studio EQ v1.0", &studio_eq);
    println!("{}", config);
}
