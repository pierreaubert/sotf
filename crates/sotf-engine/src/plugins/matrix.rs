//! Matrix preset detection and manipulation functions

/// Returns the list of presets valid for the given channel configuration.
/// Identity is always available. Other presets require specific channel counts.
pub fn available_matrix_presets(in_ch: usize, out_ch: usize) -> Vec<&'static str> {
    let mut presets = vec!["Identity"];
    if in_ch >= 2 && out_ch >= 2 {
        presets.push("Swap L/R");
    }
    // Mono Mix is only distinct from Identity when in_ch > 1
    if in_ch > 1 {
        presets.push("Mono Mix");
    }
    if in_ch >= 2 && out_ch >= 2 {
        presets.push("M/S Encode");
        presets.push("M/S Decode");
    }
    presets
}

/// Detect which preset a matrix matches, if any
pub fn detect_matrix_preset(in_ch: usize, out_ch: usize, matrix: &[f32]) -> &'static str {
    if is_identity_matrix(in_ch, out_ch, matrix) {
        "Identity"
    } else if is_swap_matrix(in_ch, out_ch, matrix) {
        "Swap L/R"
    } else if is_mono_mix_matrix(in_ch, out_ch, matrix) {
        "Mono Mix"
    } else if is_ms_encode_matrix(in_ch, out_ch, matrix) {
        "M/S Encode"
    } else if is_ms_decode_matrix(in_ch, out_ch, matrix) {
        "M/S Decode"
    } else {
        "Custom"
    }
}

/// Check if matrix is identity (diagonal = 1, rest = 0)
fn is_identity_matrix(in_ch: usize, out_ch: usize, matrix: &[f32]) -> bool {
    if matrix.len() != in_ch * out_ch {
        return false;
    }

    for out in 0..out_ch {
        for inp in 0..in_ch {
            let value = matrix[out * in_ch + inp];
            let expected = if inp == out { 1.0 } else { 0.0 };
            if (value - expected).abs() > 0.001 {
                return false;
            }
        }
    }
    true
}

/// Check if matrix swaps L/R (first two channels swapped, rest pass-through)
/// Requires at least 2 input and 2 output channels.
fn is_swap_matrix(in_ch: usize, out_ch: usize, matrix: &[f32]) -> bool {
    if in_ch < 2 || out_ch < 2 || matrix.len() != in_ch * out_ch {
        return false;
    }

    // Expected pattern: out0←in1, out1←in0, remaining diagonal pass-through
    for out in 0..out_ch {
        for inp in 0..in_ch {
            let value = matrix[out * in_ch + inp];
            let expected =
                if (out == 0 && inp == 1) || (out == 1 && inp == 0) || (out >= 2 && inp == out) {
                    1.0
                } else {
                    0.0
                };
            if (value - expected).abs() > 0.001 {
                return false;
            }
        }
    }
    true
}

/// Check if matrix is a mono mix (all inputs summed equally to all outputs)
/// Uses equal-voltage summing: gain = 1/N where N = number of inputs
/// For stereo: 1/2 = 0.5 = -6dB per channel (preserves level for mono-compatible content)
fn is_mono_mix_matrix(in_ch: usize, out_ch: usize, matrix: &[f32]) -> bool {
    if matrix.len() != in_ch * out_ch || in_ch == 0 {
        return false;
    }

    // Expected gain for equal voltage (mono-compatible) mix
    let expected_gain = 1.0 / (in_ch as f32);

    for value in matrix {
        if (*value - expected_gain).abs() > 0.001 {
            return false;
        }
    }
    true
}

/// Check if matrix is M/S Encode (first two channels encoded, rest pass-through)
/// Mid = 0.5*L + 0.5*R, Side = 0.5*L - 0.5*R
fn is_ms_encode_matrix(in_ch: usize, out_ch: usize, matrix: &[f32]) -> bool {
    if in_ch < 2 || out_ch < 2 || matrix.len() != in_ch * out_ch {
        return false;
    }

    for out in 0..out_ch {
        for inp in 0..in_ch {
            let value = matrix[out * in_ch + inp];
            let expected = match (out, inp) {
                (0, 0) | (0, 1) | (1, 0) => 0.5,
                (1, 1) => -0.5,
                (o, i) if o >= 2 && i == o => 1.0,
                _ => 0.0,
            };
            if (value - expected).abs() > 0.001 {
                return false;
            }
        }
    }
    true
}

/// Check if matrix is M/S Decode (first two channels decoded, rest pass-through)
/// L = Mid + Side, R = Mid - Side
fn is_ms_decode_matrix(in_ch: usize, out_ch: usize, matrix: &[f32]) -> bool {
    if in_ch < 2 || out_ch < 2 || matrix.len() != in_ch * out_ch {
        return false;
    }

    for out in 0..out_ch {
        for inp in 0..in_ch {
            let value = matrix[out * in_ch + inp];
            let expected = match (out, inp) {
                (0, 0) | (0, 1) | (1, 0) => 1.0,
                (1, 1) => -1.0,
                (o, i) if o >= 2 && i == o => 1.0,
                _ => 0.0,
            };
            if (value - expected).abs() > 0.001 {
                return false;
            }
        }
    }
    true
}

/// Apply a preset to the matrix
pub fn apply_matrix_preset(in_ch: usize, out_ch: usize, matrix: &mut Vec<f32>, preset: &str) {
    matrix.resize(in_ch * out_ch, 0.0);
    matrix.fill(0.0);

    match preset {
        "Identity" => {
            for i in 0..in_ch.min(out_ch) {
                matrix[i * in_ch + i] = 1.0;
            }
        }
        "Swap L/R" => {
            if in_ch >= 2 && out_ch >= 2 {
                // Swap first two channels
                matrix[1] = 1.0; // Out 0 <- In 1
                matrix[in_ch] = 1.0; // Out 1 <- In 0
                // Pass through remaining channels
                for i in 2..in_ch.min(out_ch) {
                    matrix[i * in_ch + i] = 1.0;
                }
            }
        }
        "Mono Mix" => {
            // Equal-voltage summing: 1/N per channel
            // For stereo: 1/2 = 0.5 = -6dB per channel
            // This preserves level for mono-compatible content (L=R)
            let gain = 1.0 / (in_ch as f32);
            matrix.fill(gain);
        }
        "M/S Encode" => {
            // Mid/Side encoding (stereo only)
            // Mid = 0.5*L + 0.5*R, Side = 0.5*L - 0.5*R
            if in_ch >= 2 && out_ch >= 2 {
                matrix[0] = 0.5; // Out 0 (Mid) <- 0.5 * In 0 (L)
                matrix[1] = 0.5; // Out 0 (Mid) <- 0.5 * In 1 (R)
                matrix[in_ch] = 0.5; // Out 1 (Side) <- 0.5 * In 0 (L)
                matrix[in_ch + 1] = -0.5; // Out 1 (Side) <- -0.5 * In 1 (R)
                // Pass through remaining channels
                for i in 2..in_ch.min(out_ch) {
                    matrix[i * in_ch + i] = 1.0;
                }
            }
        }
        "M/S Decode" => {
            // Mid/Side decoding (stereo only)
            // L = Mid + Side, R = Mid - Side
            if in_ch >= 2 && out_ch >= 2 {
                matrix[0] = 1.0; // Out 0 (L) <- 1.0 * In 0 (Mid)
                matrix[1] = 1.0; // Out 0 (L) <- 1.0 * In 1 (Side)
                matrix[in_ch] = 1.0; // Out 1 (R) <- 1.0 * In 0 (Mid)
                matrix[in_ch + 1] = -1.0; // Out 1 (R) <- -1.0 * In 1 (Side)
                // Pass through remaining channels
                for i in 2..in_ch.min(out_ch) {
                    matrix[i * in_ch + i] = 1.0;
                }
            }
        }
        _ => {
            // Custom or unknown - set to identity as fallback
            for i in 0..in_ch.min(out_ch) {
                matrix[i * in_ch + i] = 1.0;
            }
        }
    }
}

/// Resize matrix preserving existing values where possible
/// New cells on diagonal get 1.0 (identity), others get 0.0
pub fn resize_matrix(
    matrix: &mut Vec<f32>,
    old_in: usize,
    old_out: usize,
    new_in: usize,
    new_out: usize,
) {
    let mut new_matrix = vec![0.0; new_in * new_out];

    // Copy existing values
    for out in 0..old_out.min(new_out) {
        for inp in 0..old_in.min(new_in) {
            new_matrix[out * new_in + inp] = matrix[out * old_in + inp];
        }
    }

    // Fill diagonal for new channels
    for i in old_in.min(old_out)..new_in.min(new_out) {
        new_matrix[i * new_in + i] = 1.0;
    }

    *matrix = new_matrix;
}

/// Map a speaker configuration string to its output channel count.
pub fn upmixer_output_channels(speaker_config: &str) -> usize {
    match speaker_config {
        "2.0" => 2,
        "2.1" => 3,
        "2.2" => 4,
        "5.0" => 5,
        "5.1" => 6,
        "7.1" => 8,
        "9.1" => 8,
        "5.1.2" => 8,
        "5.1.4" => 10,
        "7.1.2" => 10,
        "7.1.4" => 12,
        "9.1.2" => 12,
        "9.1.4" => 14,
        "9.1.6" => 16,
        _ => {
            log::warn!(
                "Unknown speaker config '{}', defaulting to 5.1 (6 channels)",
                speaker_config
            );
            6
        }
    }
}
