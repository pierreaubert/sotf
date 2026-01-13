// Tes fixtures for binaural decoder tests
//
// Provides utilities to create synthetic SOFA files for testing

use std::f32::consts::PI;
use std::path::PathBuf;

/// Create a minimal test SOFA file with synthetic HRTFs
///
/// # Arguments
/// * `filename` - Output filename
/// * `num_positions` - Number of HRTF measurement positions
/// * `ir_length` - Length of impulse responses in samples
/// * `sample_rate` - Sample rate in Hz
///
/// # Returns
/// Path to the created SOFA file
pub fn create_test_sofa_file(
    filename: &str,
    num_positions: usize,
    ir_length: usize,
    sample_rate: f32,
) -> PathBuf {
    use netcdf;
    use std::env;

    let temp_dir = env::temp_dir();
    let path = temp_dir.join(filename);

    // Create NetCDF file
    let mut file = netcdf::create(&path).expect("Failed to create NetCDF file");

    // Add global attributes
    file.add_attribute("Conventions", "SOFA").unwrap();
    file.add_attribute("SOFAConventions", "SimpleFreeFieldHRIR")
        .unwrap();
    file.add_attribute("SOFAConventionsVersion", "1.0").unwrap();
    file.add_attribute("DataType", "FIR").unwrap();
    file.add_attribute("RoomType", "free field").unwrap();
    file.add_attribute("Title", "Test SOFA file").unwrap();

    // Define dimensions
    file.add_dimension("M", num_positions).unwrap(); // Measurements
    file.add_dimension("R", 2).unwrap(); // Receivers (L/R ears)
    file.add_dimension("N", ir_length).unwrap(); // Samples
    file.add_dimension("C", 3).unwrap(); // Coordinates (az, el, r)

    // Add Data.SamplingRate variable
    let mut sr_var = file.add_variable::<f32>("Data.SamplingRate", &[]).unwrap();
    sr_var
        .put_value::<_, &[std::ops::RangeFull; 0]>(sample_rate, &[])
        .unwrap();

    // Generate source positions (distributed around sphere)
    let mut positions = vec![0.0f32; num_positions * 3];
    for i in 0..num_positions {
        let theta = 2.0 * PI * (i as f32) / (num_positions as f32);
        let phi = (i as f32 / num_positions as f32) * PI - PI / 2.0;

        positions[i * 3] = theta.to_degrees(); // Azimuth
        positions[i * 3 + 1] = phi.to_degrees(); // Elevation
        positions[i * 3 + 2] = 1.0; // Distance
    }

    let mut pos_var = file
        .add_variable::<f32>("SourcePosition", &["M", "C"])
        .unwrap();
    pos_var.put_values(&positions, &[.., ..]).unwrap();

    // Add variable attributes using file-level attribute with variable:name format
    file.add_attribute("SourcePosition:Type", "spherical")
        .unwrap();
    file.add_attribute("SourcePosition:Units", "degree, degree, metre")
        .unwrap();

    // Generate synthetic HRTF impulse responses
    // Simple model: delayed impulse with frequency-dependent decay
    let mut ir_data = vec![0.0f32; num_positions * 2 * ir_length];

    for m in 0..num_positions {
        let azimuth = positions[m * 3].to_radians();

        // ITD (Interaural Time Difference) based on azimuth
        // Simple sphere head model: ITD = (r/c) * (azimuth + sin(azimuth))
        let head_radius = 0.0875; // 8.75 cm
        let speed_of_sound = 343.0; // m/s
        let itd_samples =
            (head_radius / speed_of_sound * sample_rate * (azimuth + azimuth.sin())) as i32;

        // Generate left ear IR
        let left_delay = (ir_length / 4).saturating_sub(itd_samples.max(0) as usize);
        let left_offset = m * 2 * ir_length;
        if left_delay < ir_length {
            // Simple impulse with exponential decay
            for i in 0..(ir_length - left_delay).min(50) {
                let t = i as f32 / sample_rate;
                let decay = (-t * 2000.0).exp();
                ir_data[left_offset + left_delay + i] = decay * (2.0 * PI * 1000.0 * t).sin() * 0.1;
            }
        }

        // Generate right ear IR
        let right_delay = ir_length / 4 + itd_samples.max(0) as usize;
        let right_offset = m * 2 * ir_length + ir_length;
        if right_delay < ir_length {
            for i in 0..(ir_length - right_delay).min(50) {
                let t = i as f32 / sample_rate;
                let decay = (-t * 2000.0).exp();
                ir_data[right_offset + right_delay + i] =
                    decay * (2.0 * PI * 1000.0 * t).sin() * 0.1;
            }
        }

        // ILD (Interaural Level Difference) - attenuate contralateral ear
        let ild_factor = (1.0 - azimuth.abs() / PI).max(0.3);
        if azimuth > 0.0 {
            // Source on left, attenuate right
            for i in 0..ir_length {
                ir_data[right_offset + i] *= ild_factor;
            }
        } else {
            // Source on right, attenuate left
            for i in 0..ir_length {
                ir_data[left_offset + i] *= ild_factor;
            }
        }
    }

    let mut ir_var = file
        .add_variable::<f32>("Data.IR", &["M", "R", "N"])
        .unwrap();

    // Debug: Check if IR data is non-zero
    let non_zero_count = ir_data.iter().filter(|&&x| x != 0.0).count();
    println!("  Non-zero IR samples: {}/{}", non_zero_count, ir_data.len());

    ir_var.put_values(&ir_data, &[.., .., ..]).unwrap();

    // Add receiver positions (ears)
    let receiver_pos = vec![
        90.0, 0.0, 0.0, // Left ear
        -90.0, 0.0, 0.0, // Right ear
    ];
    let mut recv_var = file
        .add_variable::<f32>("ReceiverPosition", &["R", "C"])
        .unwrap();
    recv_var.put_values(&receiver_pos, &[.., ..]).unwrap();

    file.add_attribute("ReceiverPosition:Type", "spherical")
        .unwrap();

    // Add listener position
    let listener_pos = vec![0.0, 0.0, 0.0];
    let mut list_var = file
        .add_variable::<f32>("ListenerPosition", &["C"])
        .unwrap();
    list_var.put_values(&listener_pos, &[..]).unwrap();

    drop(file); // Close file

    println!("Created test SOFA file: {:?}", path);
    println!(
        "  Positions: {}, IR length: {}, Sample rate: {} Hz",
        num_positions, ir_length, sample_rate
    );

    path
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_create_sofa_file() {
        let path = create_test_sofa_file("test_fixture.sofa", 10, 128, 48000.0);

        // Verify file exists
        assert!(path.exists(), "SOFA file was not created");

        // Verify it's a valid NetCDF file
        let file = netcdf::open(&path).expect("Failed to open created SOFA file");

        // Check dimensions
        assert_eq!(file.dimension("M").unwrap().len(), 10);
        assert_eq!(file.dimension("R").unwrap().len(), 2);
        assert_eq!(file.dimension("N").unwrap().len(), 128);

        // Clean up
        fs::remove_file(path).ok();
    }
}
