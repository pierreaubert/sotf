#![no_main]
use libfuzzer_sys::fuzz_target;
use std::process::Command;
use std::fs;
use tempfile::TempDir;

fuzz_target!(|data: &[u8]| {
    // Try to parse as JSON configuration
    if let Ok(config_str) = std::str::from_utf8(data) {
        // Skip very small inputs
        if data.len() < 10 {
            return;
        }

        // Try to create a valid-looking CSV
        if config_str.contains("freq") && config_str.contains("spl") {
            if let Ok(temp_dir) = TempDir::new() {
                let csv_path = temp_dir.path().join("fuzz.csv");
                let output_path = temp_dir.path().join("output");

                if let Ok(content) = sanitize_csv(config_str) {
                    if let Ok(()) = fs::write(&csv_path, content) {
                        // Try to run autoeq (don't care about success, just no panics)
                        let _ = Command::new("cargo")
                            .args(&["run", "--bin", "autoeq", "--", "--curve"])
                            .arg(&csv_path)
                            .arg("--output")
                            .arg(&output_path)
                            .arg("--max-iter", "10")  // Limit iterations for fuzzing
                            .output();
                    }
                }
            }
        }
    }
});

/// Sanitize input to valid CSV format
fn sanitize_csv(input: &str) -> Result<String, ()> {
    let mut lines = Vec::new();
    lines.push("freq,spl".to_string());

    for line in input.lines() {
        let parts: Vec<&str> = line.split(',').collect();
        if parts.len() >= 2 {
            if let Ok(freq) = parts[0].trim().parse::<f64>() {
                if let Ok(spl) = parts[1].trim().parse::<f64>() {
                    if freq > 0.0 && freq <= 100000.0 && spl > 0.0 && spl <= 150.0 {
                        lines.push(format!("{},{}", freq, spl));
                    }
                }
            }
        }
    }

    if lines.len() >= 2 {
        Ok(lines.join("\n"))
    } else {
        Err(())
    }
}
