use std::path::PathBuf;

const DATA_CACHED: &str = "data_cached";
const CACHE_BUNDLE_ID: &str = "org.spinorama.sotf";
const CACHE_ENV_VAR: &str = "SOTF_CACHE_DIR";

/// Resolve the root cache directory used to store downloaded measurements.
///
/// Resolution order:
/// 1. `$SOTF_CACHE_DIR` if set (tests, CI, dev overrides).
/// 2. A `./data_cached` directory in the current working directory if it
///    already exists (preserves the in-tree dev workflow used by the
///    benchmark and download CLIs).
/// 3. The platform user cache directory joined with `org.spinorama.sotf`
///    (`~/Library/Caches/org.spinorama.sotf` on macOS,
///    `~/.cache/org.spinorama.sotf` on Linux,
///    `%LOCALAPPDATA%\org.spinorama.sotf` on Windows).
/// 4. As a last resort, the relative `./data_cached` path.
pub fn cache_root() -> PathBuf {
    if let Ok(custom) = std::env::var(CACHE_ENV_VAR)
        && !custom.is_empty()
    {
        return PathBuf::from(custom);
    }

    let legacy = PathBuf::from(DATA_CACHED);
    if legacy.is_dir() {
        return legacy;
    }

    if let Some(base) = directories::BaseDirs::new() {
        let mut p = base.cache_dir().to_path_buf();
        p.push(CACHE_BUNDLE_ID);
        return p;
    }

    legacy
}

/// Return the cache directory for a given speaker under
/// `<cache_root>/speakers/org.spinorama/<sanitized name>`.
pub fn data_dir_for(speaker: &str) -> PathBuf {
    let mut p = cache_root();
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
