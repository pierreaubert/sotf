use super::misc::spherical_to_cartesian;

/// Calculate panning gain for a speaker based on source position
/// Uses modified Vector Base Amplitude Panning (VBAP) with improved height handling
///
/// # Arguments
/// * `source_azimuth` - Source azimuth in degrees
/// * `source_elevation` - Source elevation in degrees
/// * `speaker_azimuth` - Speaker azimuth in degrees
/// * `speaker_elevation` - Speaker elevation in degrees
///
/// # Returns
/// Gain value (0.0 to 1.0)
pub fn calculate_panning_gain(
    source_azimuth: f32,
    source_elevation: f32,
    speaker_azimuth: f32,
    speaker_elevation: f32,
) -> f32 {
    let src = spherical_to_cartesian(source_azimuth, source_elevation);
    let spk = spherical_to_cartesian(speaker_azimuth, speaker_elevation);

    // Calculate dot product (cosine of angle between unit vectors)
    let dot_product = src[0] * spk[0] + src[1] * spk[1] + src[2] * spk[2];

    // Clamp to [0, 1]
    let cosine_gain = dot_product.max(0.0);

    // Apply modified panning law for more even distribution
    // Use power law with exponent 0.5 (square root) for gentler rolloff
    // This helps height channels receive more signal and reduces "hole in middle" effect
    // Standard VBAP uses linear (power 1.0), but 0.5-0.7 is more perceptually uniform
    let gain = cosine_gain.powf(0.5);

    log::trace!(
        "[VBAP] source=({:>6.1}°, {:>5.1}°) speaker=({:>6.1}°, {:>5.1}°) cosine={:.4} gain={:.4}",
        source_azimuth,
        source_elevation,
        speaker_azimuth,
        speaker_elevation,
        cosine_gain,
        gain
    );

    gain
}

/// Calculate panning gain with rear wrap-around for speakers beyond 90° from source
///
/// When a speaker is more than 90° away from the source position, the standard VBAP
/// algorithm produces zero gain. This function treats such speakers as receiving
/// a "phantom source" from the rear (source position + 180°), with an attenuation
/// factor to maintain front-back separation.
///
/// This mimics how commercial upmixers create an enveloping soundfield by projecting
/// stereo content to rear speakers.
///
/// # Arguments
/// * `source_azimuth` - Source azimuth in degrees
/// * `source_elevation` - Source elevation in degrees
/// * `speaker_azimuth` - Speaker azimuth in degrees
/// * `speaker_elevation` - Speaker elevation in degrees
/// * `wrap_attenuation` - Attenuation factor for wrapped sources (0.0 to 1.0)
///
/// # Returns
/// Gain value (0.0 to 1.0)
pub fn calculate_panning_gain_with_wraparound(
    source_azimuth: f32,
    source_elevation: f32,
    speaker_azimuth: f32,
    speaker_elevation: f32,
    wrap_attenuation: f32,
) -> f32 {
    // Try direct path first
    let direct_gain = calculate_panning_gain(
        source_azimuth,
        source_elevation,
        speaker_azimuth,
        speaker_elevation,
    );

    // If direct gain is significant, use it
    if direct_gain > 0.01 {
        return direct_gain;
    }

    // Calculate wrapped source position (from rear)
    let wrapped_azimuth = if source_azimuth > 0.0 {
        source_azimuth - 180.0
    } else {
        source_azimuth + 180.0
    };

    let wrapped_gain = calculate_panning_gain(
        wrapped_azimuth,
        source_elevation,
        speaker_azimuth,
        speaker_elevation,
    );

    wrapped_gain * wrap_attenuation
}
