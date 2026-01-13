use crate::database::MusicDatabase;
use std::collections::HashSet;
use std::path::PathBuf;

/// Generate track recommendations based on current queue or listening history
///
/// Logic:
/// 1. Identify "seed" tracks:
///    - If queue is not empty, use the last 5 tracks from the queue.
///    - If queue is empty, use the top 5 most played tracks from history.
/// 2. Fetch bliss analysis for seed tracks.
/// 3. Fetch bliss analysis for all candidate tracks in the library.
/// 4. Calculate similarity (distance) between candidates and seeds.
/// 5. Select closest matches that are not already in the queue.
/// 6. Return enough tracks to fill the target duration.
pub fn recommend_tracks(
    db: &MusicDatabase,
    current_queue_paths: &[PathBuf],
    target_duration_secs: u64,
) -> Result<Vec<PathBuf>, Box<dyn std::error::Error>> {
    let mut recommendations = Vec::new();
    let mut current_duration = 0;

    log::info!(
        "[Recommendation] Starting generation. Queue size: {}, Target duration: {}s",
        current_queue_paths.len(),
        target_duration_secs
    );

    // 1. Identify seed tracks
    let mut seed_paths = Vec::new();
    if !current_queue_paths.is_empty() {
        // Use last 5 tracks from queue as seeds
        let start_idx = current_queue_paths.len().saturating_sub(5);
        seed_paths.extend_from_slice(&current_queue_paths[start_idx..]);
        log::info!(
            "[Recommendation] Using last {} tracks from queue as seeds",
            seed_paths.len()
        );
    } else {
        // Fallback: Use top 5 most played tracks
        let top_tracks = db.get_top_tracks_by_play_count(5)?;
        seed_paths.extend(top_tracks);
        log::info!(
            "[Recommendation] Queue empty. Using top {} most played tracks as seeds",
            seed_paths.len()
        );
    }

    if seed_paths.is_empty() {
        log::warn!(
            "[Recommendation] No seed tracks found (empty library or no history). Cannot recommend."
        );
        return Ok(Vec::new());
    }

    // 2. Fetch bliss analysis for seed tracks
    let mut seed_analyses = Vec::new();
    for path in &seed_paths {
        if let Some(analysis) = db.get_bliss_analysis(path)? {
            seed_analyses.push(analysis);
        } else {
            log::debug!(
                "[Recommendation] Seed track has no bliss analysis: {:?}",
                path
            );
        }
    }

    log::info!(
        "[Recommendation] Found bliss analysis for {}/{} seed tracks",
        seed_analyses.len(),
        seed_paths.len()
    );

    if seed_analyses.is_empty() {
        log::warn!("[Recommendation] No seed tracks have bliss analysis data.");
        return Ok(Vec::new());
    }

    // 3. Fetch bliss analysis for all candidates
    let candidates = db.get_all_bliss_features()?;
    log::info!(
        "[Recommendation] Found {} total candidates with bliss features",
        candidates.len()
    );

    // Set of paths to exclude (already in queue or already picked)
    let mut excluded_paths: HashSet<PathBuf> = current_queue_paths.iter().cloned().collect();

    // 4. Score candidates
    let mut scored_candidates: Vec<(PathBuf, u64, f32)> = Vec::new();

    for (path, analysis, duration) in candidates {
        if excluded_paths.contains(&path) {
            continue;
        }

        // Find min distance to any seed
        let mut min_dist = f32::MAX;
        for seed in &seed_analyses {
            let dist = analysis.distance(seed);
            if dist < min_dist {
                min_dist = dist;
            }
        }

        scored_candidates.push((path, duration, min_dist));
    }

    log::info!(
        "[Recommendation] Scored {} candidates",
        scored_candidates.len()
    );

    // 5. Sort by distance (ascending)
    // Use partial_cmp and handle NaNs (shouldn't happen with bliss features usually)
    scored_candidates.sort_by(|a, b| a.2.partial_cmp(&b.2).unwrap_or(std::cmp::Ordering::Equal));

    // 6. Select tracks
    for (path, duration, score) in scored_candidates {
        if current_duration >= target_duration_secs {
            break;
        }

        recommendations.push(path.clone());
        excluded_paths.insert(path);
        current_duration += duration;
        log::debug!(
            "[Recommendation] Selected: {:?} (score: {:.4}, duration: {}s)",
            recommendations.last().unwrap(),
            score,
            duration
        );
    }

    log::info!(
        "[Recommendation] Generated {} recommendations (total duration: {}s)",
        recommendations.len(),
        current_duration
    );

    Ok(recommendations)
}
