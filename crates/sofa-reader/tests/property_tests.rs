// ============================================================================
// Property-Based Tests for SOFA / HRTF coordinate utilities
// ============================================================================
//
// This module uses proptest to verify spherical/cartesian round-trips,
// angular-distance symmetry, and nearest-neighbor ordering invariants.

use proptest::prelude::*;
use sofa_reader::{SofaFile, SourcePosition};

/// Construct a minimal SofaFile from a list of positions.
fn sofa_file_with_positions(positions: Vec<SourcePosition>) -> SofaFile {
    let m = positions.len().max(1);
    let ir_length = 1;
    SofaFile {
        sample_rate: 48000.0,
        num_measurements: m,
        ir_length,
        positions,
        impulse_responses: vec![0.0f32; m * 2 * ir_length],
        convention: "test".to_string(),
        data_sample_rate: Some(48000.0),
    }
}

// ============================================================================
// Coordinate round-trip properties
// ============================================================================

proptest! {
    /// INVARIANT: to_cartesian_unit_vector -> from_cartesian round-trips
    /// azimuth and elevation (distance becomes 1 because the input is a unit
    /// vector). Azimuth wrap-around is handled by atan2.
    #[test]
    fn spherical_cartesian_roundtrip(
        azimuth in -180.0f32..180.0,
        elevation in -80.0f32..80.0, // avoid pole singularity in round-trip
    ) {
        let original = SourcePosition::new(azimuth, elevation, 1.0);
        let [x, y, z] = original.to_cartesian_unit_vector();
        let back = SourcePosition::from_cartesian(x, y, z);

        prop_assert!(
            back.azimuth.is_finite() && back.elevation.is_finite() && back.distance.is_finite(),
            "Round-trip produced non-finite values: {:?}", back
        );

        let az_diff = (back.azimuth - original.azimuth + 180.0).rem_euclid(360.0) - 180.0;
        prop_assert!(
            az_diff.abs() < 1e-4,
            "Azimuth round-trip drift: {} -> {}", original.azimuth, back.azimuth
        );
        prop_assert!(
            (back.elevation - original.elevation).abs() < 1e-4,
            "Elevation round-trip drift: {} -> {}", original.elevation, back.elevation
        );
        prop_assert!(
            (back.distance - 1.0).abs() < 1e-4,
            "Distance should be 1.0 for unit vector, got {}", back.distance
        );
    }

    /// INVARIANT: from_cartesian -> to_cartesian_unit_vector round-trips
    /// direction for non-origin points.
    #[test]
    fn cartesian_spherical_roundtrip(
        x in -10.0f32..10.0,
        y in -10.0f32..10.0,
        z in -10.0f32..10.0,
    ) {
        // Skip near-origin where direction is undefined.
        prop_assume!(x.hypot(y).hypot(z) > 1e-3);

        let spherical = SourcePosition::from_cartesian(x, y, z);
        let [x2, y2, z2] = spherical.to_cartesian_unit_vector();
        let back = SourcePosition::from_cartesian(x2, y2, z2);

        prop_assert!(
            back.azimuth.is_finite() && back.elevation.is_finite() && back.distance.is_finite(),
            "Round-trip produced non-finite values: {:?}", back
        );

        let az_diff = (back.azimuth - spherical.azimuth + 180.0).rem_euclid(360.0) - 180.0;
        prop_assert!(
            az_diff.abs() < 1e-4,
            "Azimuth round-trip drift: {} -> {}", spherical.azimuth, back.azimuth
        );
        prop_assert!(
            (back.elevation - spherical.elevation).abs() < 1e-4,
            "Elevation round-trip drift: {} -> {}", spherical.elevation, back.elevation
        );
    }
}

// ============================================================================
// Angular distance properties
// ============================================================================

proptest! {
    /// INVARIANT: angular_distance is symmetric.
    #[test]
    fn angular_distance_symmetry(
        a_az in -180.0f32..180.0,
        a_el in -90.0f32..90.0,
        a_dist in 0.1f32..10.0,
        b_az in -180.0f32..180.0,
        b_el in -90.0f32..90.0,
        b_dist in 0.1f32..10.0,
    ) {
        let a = SourcePosition::new(a_az, a_el, a_dist);
        let b = SourcePosition::new(b_az, b_el, b_dist);

        let d_ab = a.angular_distance(&b);
        let d_ba = b.angular_distance(&a);

        prop_assert!(
            d_ab.is_finite() && d_ba.is_finite(),
            "angular_distance returned non-finite: {} <=> {}", d_ab, d_ba
        );
        prop_assert!(
            (d_ab - d_ba).abs() < 1e-4,
            "angular_distance not symmetric: {} vs {}", d_ab, d_ba
        );
    }

    /// INVARIANT: angular_distance to self is (near) zero.
    #[test]
    fn angular_distance_to_self_is_zero(
        az in -180.0f32..180.0,
        el in -90.0f32..90.0,
        dist in 0.1f32..10.0,
    ) {
        let pos = SourcePosition::new(az, el, dist);
        let d = pos.angular_distance(&pos);

        prop_assert!(d.is_finite(), "Distance to self should be finite, got {}", d);
        prop_assert!(d.abs() < 1e-5, "Distance to self should be ~0, got {}", d);
    }

    /// INVARIANT: angular_distance is non-negative and at most 180 degrees.
    #[test]
    fn angular_distance_bounded(
        a_az in -180.0f32..180.0,
        a_el in -90.0f32..90.0,
        b_az in -180.0f32..180.0,
        b_el in -90.0f32..90.0,
    ) {
        let a = SourcePosition::new(a_az, a_el, 1.0);
        let b = SourcePosition::new(b_az, b_el, 1.0);
        let d = a.angular_distance(&b);

        prop_assert!(d.is_finite(), "angular_distance should be finite, got {}", d);
        prop_assert!(d >= 0.0, "angular_distance should be >= 0, got {}", d);
        prop_assert!(d <= 180.0 + 1e-4, "angular_distance should be <= 180, got {}", d);
    }

    /// INVARIANT: find_nearest distance is symmetric (uses lookup_distance
    /// internally, which must be symmetric for the index to be consistent).
    #[test]
    fn find_nearest_distance_symmetric_for_two_positions(
        a_az in -180.0f32..180.0,
        a_el in -90.0f32..90.0,
        a_dist in 0.1f32..10.0,
        b_az in -180.0f32..180.0,
        b_el in -90.0f32..90.0,
        b_dist in 0.1f32..10.0,
    ) {
        let a = SourcePosition::new(a_az, a_el, a_dist);
        let b = SourcePosition::new(b_az, b_el, b_dist);

        let sf_a = sofa_file_with_positions(vec![a, b]);
        let sf_b = sofa_file_with_positions(vec![b, a]);

        let (_, d_ab) = sf_a.find_nearest(&a);
        let (_, d_ba) = sf_b.find_nearest(&b);

        prop_assert!(
            d_ab.is_finite() && d_ba.is_finite(),
            "find_nearest returned non-finite: {} <=> {}", d_ab, d_ba
        );
        prop_assert!(
            (d_ab - d_ba).abs() < 1e-4,
            "find_nearest distance not symmetric: {} vs {}", d_ab, d_ba
        );
    }
}

// ============================================================================
// Nearest-neighbor ordering properties
// ============================================================================

proptest! {
    /// INVARIANT: find_nearest returns a finite distance for non-empty files.
    #[test]
    fn find_nearest_finite(
        positions in prop::collection::vec(
            (-180.0f32..180.0, -90.0f32..90.0, 0.1f32..10.0),
            1..16,
        ),
        query in (-180.0f32..180.0, -90.0f32..90.0, 0.1f32..10.0),
    ) {
        let positions: Vec<SourcePosition> = positions
            .into_iter()
            .map(|(az, el, dist)| SourcePosition::new(az, el, dist))
            .collect();
        let sf = sofa_file_with_positions(positions);
        let (query_az, query_el, query_dist) = query;
        let query_pos = SourcePosition::new(query_az, query_el, query_dist);

        let (idx, dist) = sf.find_nearest(&query_pos);

        prop_assert!(idx < sf.positions.len(), "Index {} out of bounds {}", idx, sf.positions.len());
        prop_assert!(dist.is_finite(), "find_nearest distance should be finite, got {}", dist);
    }

    /// INVARIANT: find_three_nearest returns distances sorted in non-decreasing
    /// order, and each index is in bounds.
    #[test]
    fn find_three_nearest_sorted(
        positions in prop::collection::vec(
            (-180.0f32..180.0, -90.0f32..90.0, 0.1f32..10.0),
            3..16,
        ),
        query in (-180.0f32..180.0, -90.0f32..90.0, 0.1f32..10.0),
    ) {
        let positions: Vec<SourcePosition> = positions
            .into_iter()
            .map(|(az, el, dist)| SourcePosition::new(az, el, dist))
            .collect();
        let sf = sofa_file_with_positions(positions);
        let (query_az, query_el, query_dist) = query;
        let query_pos = SourcePosition::new(query_az, query_el, query_dist);

        let nearest = sf.find_three_nearest(&query_pos);

        for (i, (idx, dist)) in nearest.iter().enumerate() {
            prop_assert!(
                *idx < sf.positions.len(),
                "nearest[{}] index {} out of bounds {}",
                i,
                idx,
                sf.positions.len()
            );
            prop_assert!(dist.is_finite(), "nearest[{}] distance should be finite, got {}", i, dist);
        }

        prop_assert!(
            nearest[0].1 <= nearest[1].1 + 1e-5,
            "Nearest distances not sorted: {:?}", nearest
        );
        prop_assert!(
            nearest[1].1 <= nearest[2].1 + 1e-5,
            "Nearest distances not sorted: {:?}", nearest
        );
    }

    /// INVARIANT: The first element of find_three_nearest has distance equal
    /// to find_nearest for files with >= 3 positions.
    #[test]
    fn find_three_nearest_first_matches_find_nearest(
        positions in prop::collection::vec(
            (-180.0f32..180.0, -90.0f32..90.0, 0.1f32..10.0),
            3..16,
        ),
        query in (-180.0f32..180.0, -90.0f32..90.0, 0.1f32..10.0),
    ) {
        let positions: Vec<SourcePosition> = positions
            .into_iter()
            .map(|(az, el, dist)| SourcePosition::new(az, el, dist))
            .collect();
        let sf = sofa_file_with_positions(positions);
        let (query_az, query_el, query_dist) = query;
        let query_pos = SourcePosition::new(query_az, query_el, query_dist);

        let (_, nearest_dist) = sf.find_nearest(&query_pos);
        let three = sf.find_three_nearest(&query_pos);

        prop_assert!(
            (three[0].1 - nearest_dist).abs() < 1e-4,
            "find_three_nearest[0] distance {} != find_nearest distance {}",
            three[0].1,
            nearest_dist
        );
    }
}

// ============================================================================
// Finite-output sanity properties
// ============================================================================

proptest! {
    /// INVARIANT: from_cartesian never produces NaN/Inf for finite inputs.
    #[test]
    fn from_cartesian_finite_output(
        x in -100.0f32..100.0,
        y in -100.0f32..100.0,
        z in -100.0f32..100.0,
    ) {
        let pos = SourcePosition::from_cartesian(x, y, z);

        prop_assert!(
            pos.azimuth.is_finite() && pos.elevation.is_finite() && pos.distance.is_finite(),
            "from_cartesian produced non-finite output for ({}, {}, {}): {:?}",
            x, y, z, pos
        );
    }

    /// INVARIANT: to_cartesian_unit_vector never produces NaN/Inf.
    #[test]
    fn to_cartesian_unit_vector_finite_output(
        az in -180.0f32..180.0,
        el in -90.0f32..90.0,
        dist in 0.1f32..10.0,
    ) {
        let pos = SourcePosition::new(az, el, dist);
        let [x, y, z] = pos.to_cartesian_unit_vector();

        prop_assert!(
            x.is_finite() && y.is_finite() && z.is_finite(),
            "to_cartesian_unit_vector produced non-finite output: [{}, {}, {}]",
            x, y, z
        );
    }
}
