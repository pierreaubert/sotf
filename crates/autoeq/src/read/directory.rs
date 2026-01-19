use autoeq_env::DATA_CACHED;
use std::path::PathBuf;

/// Return the cache directory for a given speaker under `data_cached/speakers/org.spinorama/` using sanitized name
pub fn data_dir_for(speaker: &str) -> PathBuf {
    let mut p = PathBuf::from(DATA_CACHED);
    p.push("speakers");
    p.push("org.spinorama");
    p.push(sanitize_dir_name(speaker));
    p
}

/// Sanitize a single path component by replacing non-alphanumeric characters
/// (except space, dash and underscore) with underscores. This is used to map
/// user-provided speaker names to safe directory names inside `data/`.
pub fn sanitize_dir_name(name: &str) -> String {
    // Keep alnum, space, dash, underscore; replace others with underscore.
    let mut out = String::with_capacity(name.len());
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || ch == ' ' || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    // Trim leading/trailing spaces and underscores
    out.trim().trim_matches('_').to_string()
}

/// Return the cache filename for a measurement, neutralizing any path
/// separators. For example, "Estimated In-Room Response" becomes
/// "Estimated In-Room Response.json" and "A/B" becomes "A-B.json".
pub fn measurement_filename(measurement: &str) -> String {
    // Only neutralize path separators; keep the name otherwise to match saved files.
    let safe = measurement.replace(['/', '\\'], "-");
    format!("{}.json", safe)
}
