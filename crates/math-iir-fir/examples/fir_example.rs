//! Example demonstrating FIR filter usage

use math_audio_iir_fir::*;
use ndarray::{Array1, array};

fn main() {
    println!("AutoEQ IIR - FIR Filter Example");
    println!("================================\n");

    // Example 1: Basic FIR lowpass filter
    println!("1. Lowpass FIR Filter (1kHz cutoff, Hamming window):");
    let fir_lp = Fir::lowpass(51, 1000.0, 48000.0, WindowType::Hamming, 0.0);
    println!("   {}", fir_lp);
    println!("   Number of taps: {}", fir_lp.n_taps());

    // Calculate response at key frequencies
    let response_dc = fir_lp.log_result(0.0);
    let response_500 = fir_lp.log_result(500.0);
    let response_1k = fir_lp.log_result(1000.0);
    let response_2k = fir_lp.log_result(2000.0);
    let response_5k = fir_lp.log_result(5000.0);

    println!("   Response at DC: {:.2} dB", response_dc);
    println!("   Response at 500 Hz: {:.2} dB", response_500);
    println!("   Response at 1 kHz: {:.2} dB", response_1k);
    println!("   Response at 2 kHz: {:.2} dB", response_2k);
    println!("   Response at 5 kHz: {:.2} dB", response_5k);

    // Example 2: Highpass filter with Blackman window
    println!("\n2. Highpass FIR Filter (500Hz cutoff, Blackman window):");
    let fir_hp = Fir::highpass(101, 500.0, 48000.0, WindowType::Blackman, 0.0);
    println!("   {}", fir_hp);

    let response_100 = fir_hp.log_result(100.0);
    let response_500 = fir_hp.log_result(500.0);
    let response_1k = fir_hp.log_result(1000.0);

    println!("   Response at 100 Hz: {:.2} dB", response_100);
    println!("   Response at 500 Hz: {:.2} dB", response_500);
    println!("   Response at 1 kHz: {:.2} dB", response_1k);

    // Example 3: Bandpass filter
    println!("\n3. Bandpass FIR Filter (500-2000Hz, Hann window):");
    let fir_bp = Fir::bandpass(101, 500.0, 2000.0, 48000.0, WindowType::Hann, 0.0);
    println!("   {}", fir_bp);

    let response_100 = fir_bp.log_result(100.0);
    let response_1k = fir_bp.log_result(1000.0);
    let response_5k = fir_bp.log_result(5000.0);

    println!("   Response at 100 Hz: {:.2} dB", response_100);
    println!("   Response at 1 kHz: {:.2} dB", response_1k);
    println!("   Response at 5 kHz: {:.2} dB", response_5k);

    // Example 4: Bandstop (notch) filter with Kaiser window
    println!("\n4. Bandstop FIR Filter (800-1200Hz, Kaiser window, beta=5.0):");
    let fir_bs = Fir::bandstop(101, 800.0, 1200.0, 48000.0, WindowType::Kaiser, 5.0);
    println!("   {}", fir_bs);

    let response_500 = fir_bs.log_result(500.0);
    let response_1k = fir_bs.log_result(1000.0);
    let response_2k = fir_bs.log_result(2000.0);

    println!("   Response at 500 Hz: {:.2} dB", response_500);
    println!("   Response at 1 kHz: {:.2} dB", response_1k);
    println!("   Response at 2 kHz: {:.2} dB", response_2k);

    // Example 5: Custom FIR filter from coefficients
    println!("\n5. Custom FIR Filter (simple 5-tap moving average):");
    let coeffs = vec![0.2, 0.2, 0.2, 0.2, 0.2];
    let fir_custom = Fir::new_custom(coeffs, 48000.0);
    println!("   {}", fir_custom);
    println!("   Coefficients: {:?}", fir_custom.coeffs());

    // Example 6: Window functions comparison
    println!("\n6. Window Functions Comparison (5 taps):");
    let windows = [
        (WindowType::Rectangular, "Rectangular"),
        (WindowType::Hamming, "Hamming"),
        (WindowType::Hann, "Hann"),
        (WindowType::Blackman, "Blackman"),
        (WindowType::Kaiser, "Kaiser (beta=5.0)"),
    ];

    for (window_type, name) in windows.iter() {
        let beta = if *window_type == WindowType::Kaiser {
            5.0
        } else {
            0.0
        };
        let window = generate_window(5, *window_type, beta);
        println!(
            "   {}: {:?}",
            name,
            window
                .iter()
                .map(|&x| format!("{:.3}", x))
                .collect::<Vec<_>>()
        );
    }

    // Example 7: Vectorized frequency response
    println!("\n7. Vectorized Frequency Response:");
    let fir = Fir::lowpass(51, 1000.0, 48000.0, WindowType::Hamming, 0.0);
    let freqs = Array1::logspace(10.0, 20.0_f64.log10(), 20000.0_f64.log10(), 10);
    let response = fir.np_log_result(&freqs);

    println!("   Frequency (Hz) | Response (dB)");
    println!("   --------------------------------");
    for (freq, resp) in freqs.iter().zip(response.iter()) {
        println!("   {:>13.1} | {:>13.2}", freq, resp);
    }

    // Example 8: FIR Bank (multiple filters combined)
    println!("\n8. FIR Bank Example:");
    let fir1 = Fir::lowpass(51, 2000.0, 48000.0, WindowType::Hamming, 0.0);
    let fir2 = Fir::highpass(51, 100.0, 48000.0, WindowType::Hamming, 0.0);
    let bank = vec![(1.0, fir1), (1.0, fir2)];

    let freqs = array![50.0, 100.0, 500.0, 1000.0, 2000.0, 5000.0];
    let response = fir_bank_spl(&freqs, &bank);

    println!("   Bandpass effect (highpass + lowpass):");
    for (freq, resp) in freqs.iter().zip(response.iter()) {
        println!("   {:>6.0} Hz: {:>6.2} dB", freq, resp);
    }

    let preamp = fir_bank_preamp_gain(&bank);
    println!("   Recommended preamp gain: {:.2} dB", preamp);

    // Example 9: Processing audio samples
    println!("\n9. Processing Audio Samples:");
    let mut fir = Fir::lowpass(51, 1000.0, 48000.0, WindowType::Hamming, 0.0);

    // Process a DC signal
    println!("   Processing DC signal (1.0):");
    let mut outputs = Vec::new();
    for _ in 0..10 {
        let output = fir.process(1.0);
        outputs.push(output);
    }
    println!(
        "   First 10 outputs: {:?}",
        outputs
            .iter()
            .map(|&x| format!("{:.3}", x))
            .collect::<Vec<_>>()
    );

    // Reset and process a step response
    fir.reset();
    println!("\n   Processing step response:");
    let mut outputs = Vec::new();
    for i in 0..10 {
        let input = if i >= 5 { 1.0 } else { 0.0 };
        let output = fir.process(input);
        outputs.push((input, output));
    }
    for (i, (input, output)) in outputs.iter().enumerate() {
        println!("   Sample {}: input={:.1}, output={:.3}", i, input, output);
    }

    println!("\n10. Filter Comparison:");
    println!("   Comparing different window functions for same lowpass filter:");
    let windows = [
        (WindowType::Rectangular, "Rectangular"),
        (WindowType::Hamming, "Hamming"),
        (WindowType::Hann, "Hann"),
        (WindowType::Blackman, "Blackman"),
    ];

    let test_freqs = array![100.0, 1000.0, 2000.0, 5000.0];
    println!("\n   Freq (Hz) | Rect  | Hamm  | Hann  | Blkmn");
    println!("   -----------------------------------------------");

    for &freq in test_freqs.iter() {
        print!("   {:>8.0} |", freq);
        for (window_type, _) in windows.iter() {
            let fir = Fir::lowpass(51, 1000.0, 48000.0, *window_type, 0.0);
            let response = fir.log_result(freq);
            print!(" {:>5.1} |", response);
        }
        println!();
    }
}
