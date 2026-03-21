// ============================================================================
// HRTF Database Scanning and Anthropometric Matching
// ============================================================================
//
// Scans a directory of SOFA files and selects the best match for a given set
// of anthropometric measurements (head width, ear height).
//
// # Matching Strategy
//
// Phase 1 (initial implementation):
//   Parse numeric tokens from the filename, looking for conventions such as:
//     - "head_15cm.sofa"   → head_width = 15.0
//     - "ear_10cm.sofa"    → ear_height = 10.0
//     - "hw15_eh10.sofa"   → head_width = 15.0, ear_height = 10.0
//   Scoring = -(|Δhead| + |Δear|) — higher is better (less mismatch).
//   Files with no parseable dims score 0.0 and are valid fallbacks.
//
// Phase 2 (future): load SOFA files, compute spectral fingerprint (ILD at
//   4 kHz, low-frequency energy ratio, high-frequency roll-off), and rank by
//   correlation with the anthropometric model.

use std::path::{Path, PathBuf};

/// A candidate SOFA file discovered in the database directory.
#[derive(Debug, Clone)]
pub struct SofaCandidate {
    pub path: PathBuf,
    /// Head width extracted from filename, if available.
    pub head_width_cm: Option<f32>,
    /// Ear height extracted from filename, if available.
    pub ear_height_cm: Option<f32>,
    /// Anthropometric match score (higher = better).
    /// Range: (−∞, 0.0].  0.0 means no dims extracted (neutral).
    pub score: f32,
}

/// Scan `dir` for `.sofa` files (non-recursive) and return them sorted by
/// decreasing match score (best match first).
///
/// Returns an empty `Vec` if the directory cannot be read.
pub fn scan_and_rank(dir: &Path, head_width_cm: f32, ear_height_cm: f32) -> Vec<SofaCandidate> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(err) => {
            log::warn!(
                "[BinauralDecoder] Could not read HRTF database directory '{}': {}",
                dir.display(),
                err
            );
            return Vec::new();
        }
    };

    let mut candidates: Vec<SofaCandidate> = entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            e.path()
                .extension()
                .and_then(|s| s.to_str())
                .map(|s| s.eq_ignore_ascii_case("sofa"))
                .unwrap_or(false)
        })
        .map(|e| {
            let path = e.path();
            let (hw, eh) = parse_filename_dims(&path);
            let score = compute_score(hw, eh, head_width_cm, ear_height_cm);
            SofaCandidate {
                path,
                head_width_cm: hw,
                ear_height_cm: eh,
                score,
            }
        })
        .collect();

    // Sort by score descending; break ties alphabetically for determinism.
    candidates.sort_by(|a, b| {
        b.score
            .partial_cmp(&a.score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.path.cmp(&b.path))
    });

    candidates
}

/// Return the best-matching SOFA path from a directory, or `None` if the
/// directory is empty / unreadable.
pub fn best_match(dir: &Path, head_width_cm: f32, ear_height_cm: f32) -> Option<PathBuf> {
    scan_and_rank(dir, head_width_cm, ear_height_cm)
        .into_iter()
        .next()
        .map(|c| c.path)
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Try to parse head-width and ear-height values from the SOFA filename.
///
/// Recognised patterns (case-insensitive):
///   - `head<N>` or `hw<N>` or `head_<N>` → head width
///   - `ear<N>` or `eh<N>` or `ear_<N>`  → ear height
///
/// `N` is any unsigned integer or one-decimal-place float, optionally followed
/// by `cm`.
fn parse_filename_dims(path: &Path) -> (Option<f32>, Option<f32>) {
    let stem = match path.file_stem().and_then(|s| s.to_str()) {
        Some(s) => s.to_lowercase(),
        None => return (None, None),
    };

    let head_width = extract_tagged_value(&stem, &["head", "hw"]);
    let ear_height = extract_tagged_value(&stem, &["ear", "eh"]);

    (head_width, ear_height)
}

/// Search for any of `tags` (case-already-lowered) followed by an optional
/// separator `_` and a numeric value, and return the numeric value.
fn extract_tagged_value(text: &str, tags: &[&str]) -> Option<f32> {
    for &tag in tags {
        if let Some(pos) = text.find(tag) {
            let rest = &text[pos + tag.len()..];
            // Skip optional separator
            let rest = rest.trim_start_matches('_');
            // Read numeric prefix
            let num_str: String = rest
                .chars()
                .take_while(|c| c.is_ascii_digit() || *c == '.')
                .collect();
            if !num_str.is_empty() && let Ok(v) = num_str.parse::<f32>() {
                return Some(v);
            }
        }
    }
    None
}

/// Compute match score: negative total absolute deviation.
/// Files with no parseable dims get score 0.0 (neutral fallback).
fn compute_score(
    file_hw: Option<f32>,
    file_eh: Option<f32>,
    target_hw: f32,
    target_eh: f32,
) -> f32 {
    match (file_hw, file_eh) {
        (None, None) => 0.0,
        (Some(hw), None) => -(hw - target_hw).abs(),
        (None, Some(eh)) => -(eh - target_eh).abs(),
        (Some(hw), Some(eh)) => -((hw - target_hw).abs() + (eh - target_eh).abs()),
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_filename_head_only() {
        let path = PathBuf::from("/some/dir/head15cm.sofa");
        let (hw, eh) = parse_filename_dims(&path);
        assert_eq!(hw, Some(15.0));
        assert_eq!(eh, None);
    }

    #[test]
    fn test_parse_filename_hw_prefix() {
        let path = PathBuf::from("hw_17_ear_09.sofa");
        let (hw, eh) = parse_filename_dims(&path);
        assert_eq!(hw, Some(17.0));
        assert_eq!(eh, Some(9.0));
    }

    #[test]
    fn test_parse_filename_both() {
        let path = PathBuf::from("head_15cm_ear_10cm.sofa");
        let (hw, eh) = parse_filename_dims(&path);
        assert_eq!(hw, Some(15.0));
        assert_eq!(eh, Some(10.0));
    }

    #[test]
    fn test_parse_filename_no_dims() {
        let path = PathBuf::from("generic_hrtf.sofa");
        let (hw, eh) = parse_filename_dims(&path);
        assert_eq!(hw, None);
        assert_eq!(eh, None);
    }

    #[test]
    fn test_compute_score_both_dims() {
        // Exact match → score 0.0
        let s = compute_score(Some(15.0), Some(10.0), 15.0, 10.0);
        assert!((s - 0.0).abs() < 1e-6);

        // 2 cm head mismatch + 1 cm ear mismatch → score = -(2+1) = -3
        let s2 = compute_score(Some(17.0), Some(11.0), 15.0, 10.0);
        assert!((s2 - (-3.0)).abs() < 1e-6);
    }

    #[test]
    fn test_compute_score_no_dims_is_neutral() {
        let s = compute_score(None, None, 15.0, 10.0);
        assert!((s - 0.0).abs() < 1e-6);
    }

    #[test]
    fn test_scan_empty_dir_returns_empty() {
        // Point at a non-existent directory — should return empty without panic.
        let result = scan_and_rank(Path::new("/nonexistent/hrtf/db"), 15.0, 10.0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_scan_and_rank_ordering() {
        // Create a temp dir with synthetic SOFA filenames and verify ranking.
        let tmp = std::env::temp_dir().join("sotf_test_hrtf_db");
        let _ = std::fs::create_dir_all(&tmp);

        // Create empty .sofa files with various dims
        let files = [
            "head_13_ear_08.sofa",  // hw mismatch 2, eh mismatch 2 → score -4
            "head_15_ear_10.sofa",  // exact match → score 0
            "head_16_ear_10.sofa",  // hw mismatch 1, exact eh → score -1
            "generic.sofa",         // no dims → score 0
        ];
        for name in &files {
            let _ = std::fs::write(tmp.join(name), b"");
        }

        let ranked = scan_and_rank(&tmp, 15.0, 10.0);
        assert!(ranked.len() >= 3, "Should find at least 3 .sofa files");

        // The two score-0 entries (exact match + generic) should come first,
        // then hw16, then hw13
        let scores: Vec<f32> = ranked.iter().map(|c| c.score).collect();
        for w in scores.windows(2) {
            assert!(
                w[0] >= w[1],
                "Scores should be non-increasing: {:?}",
                scores
            );
        }

        // Clean up
        for name in &files {
            let _ = std::fs::remove_file(tmp.join(name));
        }
        let _ = std::fs::remove_dir(&tmp);
    }
}
