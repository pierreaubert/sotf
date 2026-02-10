//! Room scenario definitions for RoomEQ data generation
//!
//! Defines 17 scenarios covering small/medium/large rooms with
//! stereo, 2.1, multi-sub, multi-seat, and surround (5.0/5.1/5.1.4) configurations.

use math_audio_xem_common::{
    BoundaryConfig, CrossoverFilter, Point3D, RectangularRoom, RoomGeometry, RoomSimulation,
    Source, SurfaceConfig,
};

/// A named scenario for data generation
pub struct Scenario {
    /// Unique name for file/directory naming
    pub name: String,
    /// Human-readable description
    pub description: String,
    /// The simulation configuration
    pub simulation: RoomSimulation,
    /// Source names matching `simulation.sources` ordering
    pub source_names: Vec<String>,
}

/// Frequency configuration shared by all scenarios: 20-500 Hz, 100 points, log spacing
/// RoomEQ optimizer runs below 500 Hz so higher frequencies are not needed.
const MIN_FREQ: f64 = 20.0;
const MAX_FREQ: f64 = 500.0;
const NUM_POINTS: usize = 100;

/// Crossover frequency for sub/satellite splits
const CROSSOVER_FREQ: f64 = 80.0;
/// Butterworth order for crossover filters
const CROSSOVER_ORDER: u32 = 4;

fn make_hp_source(position: Point3D, name: &str) -> Source {
    Source::classical(position, 60.0, 40.0, 1.0)
        .with_name(name.to_string())
        .with_crossover(CrossoverFilter::Highpass {
            cutoff_freq: CROSSOVER_FREQ,
            order: CROSSOVER_ORDER,
        })
}

fn make_lp_source(position: Point3D, name: &str) -> Source {
    Source::omnidirectional(position, 1.0)
        .with_name(name.to_string())
        .with_crossover(CrossoverFilter::Lowpass {
            cutoff_freq: CROSSOVER_FREQ,
            order: CROSSOVER_ORDER,
        })
}

fn make_fullrange_source(position: Point3D, name: &str) -> Source {
    Source::classical(position, 60.0, 40.0, 1.0).with_name(name.to_string())
}

/// Typical domestic room boundary absorption coefficients.
/// Floor (carpet/rug): α ≈ 0.3, ceiling (plaster): α ≈ 0.05, walls (drywall): α ≈ 0.1.
fn default_boundaries() -> BoundaryConfig {
    BoundaryConfig {
        floor: SurfaceConfig::Absorption { coefficient: 0.3 },
        ceiling: SurfaceConfig::Absorption { coefficient: 0.05 },
        walls: SurfaceConfig::Absorption { coefficient: 0.1 },
        ..Default::default()
    }
}

fn make_simulation(
    room: RectangularRoom,
    sources: Vec<Source>,
    listening_positions: Vec<Point3D>,
) -> RoomSimulation {
    let mut sim = RoomSimulation::with_frequencies(
        RoomGeometry::Rectangular(room),
        sources,
        listening_positions,
        MIN_FREQ,
        MAX_FREQ,
        NUM_POINTS,
    );
    sim.boundaries = default_boundaries();
    sim
}

/// Generate all 17 scenarios
pub fn all_scenarios() -> Vec<Scenario> {
    vec![
        scenario_01_small_stereo_2_0(),
        scenario_02_small_stereo_2_1(),
        scenario_03_small_stereo_2_2_mso(),
        scenario_03_small_stereo_2_2_cardioid(),
        scenario_04_medium_stereo(),
        scenario_05_medium_2_1(),
        scenario_06_medium_multi_sub_4(),
        scenario_07_medium_multi_seat(),
        scenario_08_large_stereo(),
        scenario_09_large_2_1(),
        scenario_10_large_multi_sub_4(),
        scenario_11_large_multi_seat_2_1(),
        scenario_12_medium_multi_sub_multi_seat(),
        scenario_13_medium_surround_5_0(),
        scenario_14_medium_surround_5_1(),
        scenario_15_medium_surround_5_1_4(),
        scenario_16_large_surround_5_1(),
        scenario_17_large_surround_5_1_4(),
    ]
}

/// Look up a scenario by name
pub fn scenario_by_name(name: &str) -> Option<Scenario> {
    all_scenarios().into_iter().find(|s| s.name == name)
}

// ============================================================================
// Small room: 3 x 3 x 2.4 m
// ============================================================================

fn small_room() -> RectangularRoom {
    RectangularRoom::new(3.0, 3.0, 2.4)
}

/// Scenario 1: Small room, stereo 2.0, fullrange, 1 LP
fn scenario_01_small_stereo_2_0() -> Scenario {
    let room = small_room();
    let left = make_fullrange_source(Point3D::new(0.8, 0.3, 1.1), "left");
    let right = make_fullrange_source(Point3D::new(2.2, 0.3, 1.1), "right");
    let lp = Point3D::new(1.5, 2.0, 1.1);

    Scenario {
        name: "small_stereo_2_0".to_string(),
        description: "Small 3x3x2.4m room, stereo 2.0, fullrange".to_string(),
        simulation: make_simulation(room, vec![left, right], vec![lp]),
        source_names: vec!["left".to_string(), "right".to_string()],
    }
}

/// Scenario 2: Small room, 2.1, HP@80Hz mains + LP@80Hz sub, 1 LP
fn scenario_02_small_stereo_2_1() -> Scenario {
    let room = small_room();
    let left = make_hp_source(Point3D::new(0.8, 0.3, 1.1), "left");
    let right = make_hp_source(Point3D::new(2.2, 0.3, 1.1), "right");
    let sub = make_lp_source(Point3D::new(0.3, 0.3, 0.15), "subwoofer");
    let lp = Point3D::new(1.5, 2.0, 1.1);

    Scenario {
        name: "small_stereo_2_1".to_string(),
        description: "Small 3x3x2.4m room, 2.1, sub at front-left corner".to_string(),
        simulation: make_simulation(room, vec![left, right, sub], vec![lp]),
        source_names: vec!["left".to_string(), "right".to_string(), "subwoofer".to_string()],
    }
}

/// Scenario 3a: Small room, 2 subs (corners) + HP mains, 1 LP (MSO)
fn scenario_03_small_stereo_2_2_mso() -> Scenario {
    let room = small_room();
    let left = make_hp_source(Point3D::new(0.8, 0.3, 1.1), "left");
    let right = make_hp_source(Point3D::new(2.2, 0.3, 1.1), "right");
    let sub1 = make_lp_source(Point3D::new(0.15, 0.15, 0.15), "sub1");
    let sub2 = make_lp_source(Point3D::new(2.85, 0.15, 0.15), "sub2");
    let lp = Point3D::new(1.5, 2.0, 1.1);

    Scenario {
        name: "small_stereo_2_2_mso".to_string(),
        description: "Small 3x3x2.4m room, 2 subs in front corners (MSO)".to_string(),
        simulation: make_simulation(room, vec![left, right, sub1, sub2], vec![lp]),
        source_names: vec![
            "left".to_string(),
            "right".to_string(),
            "sub1".to_string(),
            "sub2".to_string(),
        ],
    }
}

/// Scenario 3b: Small room, 2 subs (stacked cardioid) + HP mains, 1 LP
fn scenario_03_small_stereo_2_2_cardioid() -> Scenario {
    let room = small_room();
    let left = make_hp_source(Point3D::new(0.8, 0.3, 1.1), "left");
    let right = make_hp_source(Point3D::new(2.2, 0.3, 1.1), "right");
    // Stacked subs at front-left (like single sub in scenario 2)
    // Bottom sub at Z=0.15, Top sub at Z=0.65 (0.5m separation)
    let sub_bottom = make_lp_source(Point3D::new(0.3, 0.3, 0.15), "sub_bottom");
    let sub_top = make_lp_source(Point3D::new(0.3, 0.3, 0.65), "sub_top");
    let lp = Point3D::new(1.5, 2.0, 1.1);

    Scenario {
        name: "small_stereo_2_2_cardioid".to_string(),
        description: "Small 3x3x2.4m room, stacked cardioid subs".to_string(),
        simulation: make_simulation(room, vec![left, right, sub_bottom, sub_top], vec![lp]),
        source_names: vec![
            "left".to_string(),
            "right".to_string(),
            "sub_bottom".to_string(),
            "sub_top".to_string(),
        ],
    }
}

// ============================================================================
// Medium room: 5 x 4 x 2.5 m
// ============================================================================

fn medium_room() -> RectangularRoom {
    RectangularRoom::new(5.0, 4.0, 2.5)
}

/// Scenario 4: Medium room, stereo 2.0, fullrange, 1 LP
fn scenario_04_medium_stereo() -> Scenario {
    let room = medium_room();
    let left = make_fullrange_source(Point3D::new(1.2, 0.4, 1.1), "left");
    let right = make_fullrange_source(Point3D::new(3.8, 0.4, 1.1), "right");
    let lp = Point3D::new(2.5, 2.5, 1.1);

    Scenario {
        name: "medium_stereo_2_0".to_string(),
        description: "Medium 5x4x2.5m room, stereo 2.0, fullrange".to_string(),
        simulation: make_simulation(room, vec![left, right], vec![lp]),
        source_names: vec!["left".to_string(), "right".to_string()],
    }
}

/// Scenario 5: Medium room, 2.1, 1 LP
fn scenario_05_medium_2_1() -> Scenario {
    let room = medium_room();
    let left = make_hp_source(Point3D::new(1.2, 0.4, 1.1), "left");
    let right = make_hp_source(Point3D::new(3.8, 0.4, 1.1), "right");
    let sub = make_lp_source(Point3D::new(0.5, 0.5, 0.15), "subwoofer");
    let lp = Point3D::new(2.5, 2.5, 1.1);

    Scenario {
        name: "medium_stereo_2_1".to_string(),
        description: "Medium 5x4x2.5m room, 2.1".to_string(),
        simulation: make_simulation(room, vec![left, right, sub], vec![lp]),
        source_names: vec!["left".to_string(), "right".to_string(), "subwoofer".to_string()],
    }
}

/// Scenario 6: Medium room, 4 subs (corners) + HP mains, 1 LP
fn scenario_06_medium_multi_sub_4() -> Scenario {
    let room = medium_room();
    let left = make_hp_source(Point3D::new(1.2, 0.4, 1.1), "left");
    let right = make_hp_source(Point3D::new(3.8, 0.4, 1.1), "right");
    let sub1 = make_lp_source(Point3D::new(0.15, 0.15, 0.15), "sub1");
    let sub2 = make_lp_source(Point3D::new(4.85, 0.15, 0.15), "sub2");
    let sub3 = make_lp_source(Point3D::new(0.15, 3.85, 0.15), "sub3");
    let sub4 = make_lp_source(Point3D::new(4.85, 3.85, 0.15), "sub4");
    let lp = Point3D::new(2.5, 2.5, 1.1);

    Scenario {
        name: "medium_multi_sub_4".to_string(),
        description: "Medium 5x4x2.5m room, 4 corner subs".to_string(),
        simulation: make_simulation(
            room,
            vec![left, right, sub1, sub2, sub3, sub4],
            vec![lp],
        ),
        source_names: vec![
            "left".to_string(),
            "right".to_string(),
            "sub1".to_string(),
            "sub2".to_string(),
            "sub3".to_string(),
            "sub4".to_string(),
        ],
    }
}

/// Scenario 7: Medium room, stereo fullrange, 3 listening positions
fn scenario_07_medium_multi_seat() -> Scenario {
    let room = medium_room();
    let left = make_fullrange_source(Point3D::new(1.2, 0.4, 1.1), "left");
    let right = make_fullrange_source(Point3D::new(3.8, 0.4, 1.1), "right");
    let lp0 = Point3D::new(2.5, 2.5, 1.1); // center seat
    let lp1 = Point3D::new(1.5, 2.5, 1.1); // left seat
    let lp2 = Point3D::new(3.5, 2.5, 1.1); // right seat

    Scenario {
        name: "medium_multi_seat".to_string(),
        description: "Medium 5x4x2.5m room, stereo, 3 seats".to_string(),
        simulation: make_simulation(room, vec![left, right], vec![lp0, lp1, lp2]),
        source_names: vec!["left".to_string(), "right".to_string()],
    }
}

// ============================================================================
// Large room: 7 x 5.5 x 2.6 m
// ============================================================================

fn large_room() -> RectangularRoom {
    RectangularRoom::new(7.0, 5.5, 2.6)
}

/// Scenario 8: Large room, stereo 2.0, fullrange, 1 LP
fn scenario_08_large_stereo() -> Scenario {
    let room = large_room();
    let left = make_fullrange_source(Point3D::new(1.5, 0.4, 1.1), "left");
    let right = make_fullrange_source(Point3D::new(5.5, 0.4, 1.1), "right");
    let lp = Point3D::new(3.5, 3.5, 1.1);

    Scenario {
        name: "large_stereo_2_0".to_string(),
        description: "Large 7x5.5x2.6m room, stereo 2.0, fullrange".to_string(),
        simulation: make_simulation(room, vec![left, right], vec![lp]),
        source_names: vec!["left".to_string(), "right".to_string()],
    }
}

/// Scenario 9: Large room, 2.1, 1 LP
fn scenario_09_large_2_1() -> Scenario {
    let room = large_room();
    let left = make_hp_source(Point3D::new(1.5, 0.4, 1.1), "left");
    let right = make_hp_source(Point3D::new(5.5, 0.4, 1.1), "right");
    let sub = make_lp_source(Point3D::new(0.5, 0.5, 0.15), "subwoofer");
    let lp = Point3D::new(3.5, 3.5, 1.1);

    Scenario {
        name: "large_stereo_2_1".to_string(),
        description: "Large 7x5.5x2.6m room, 2.1".to_string(),
        simulation: make_simulation(room, vec![left, right, sub], vec![lp]),
        source_names: vec!["left".to_string(), "right".to_string(), "subwoofer".to_string()],
    }
}

/// Scenario 10: Large room, 4 subs + HP mains, 1 LP
fn scenario_10_large_multi_sub_4() -> Scenario {
    let room = large_room();
    let left = make_hp_source(Point3D::new(1.5, 0.4, 1.1), "left");
    let right = make_hp_source(Point3D::new(5.5, 0.4, 1.1), "right");
    let sub1 = make_lp_source(Point3D::new(0.15, 0.15, 0.15), "sub1");
    let sub2 = make_lp_source(Point3D::new(6.85, 0.15, 0.15), "sub2");
    let sub3 = make_lp_source(Point3D::new(0.15, 5.35, 0.15), "sub3");
    let sub4 = make_lp_source(Point3D::new(6.85, 5.35, 0.15), "sub4");
    let lp = Point3D::new(3.5, 3.5, 1.1);

    Scenario {
        name: "large_multi_sub_4".to_string(),
        description: "Large 7x5.5x2.6m room, 4 corner subs".to_string(),
        simulation: make_simulation(
            room,
            vec![left, right, sub1, sub2, sub3, sub4],
            vec![lp],
        ),
        source_names: vec![
            "left".to_string(),
            "right".to_string(),
            "sub1".to_string(),
            "sub2".to_string(),
            "sub3".to_string(),
            "sub4".to_string(),
        ],
    }
}

/// Scenario 11: Large room, 2.1, 3 listening positions
fn scenario_11_large_multi_seat_2_1() -> Scenario {
    let room = large_room();
    let left = make_hp_source(Point3D::new(1.5, 0.4, 1.1), "left");
    let right = make_hp_source(Point3D::new(5.5, 0.4, 1.1), "right");
    let sub = make_lp_source(Point3D::new(0.5, 0.5, 0.15), "subwoofer");
    let lp0 = Point3D::new(3.5, 3.5, 1.1);
    let lp1 = Point3D::new(2.3, 3.5, 1.1);
    let lp2 = Point3D::new(4.7, 3.5, 1.1);

    Scenario {
        name: "large_multi_seat_2_1".to_string(),
        description: "Large 7x5.5x2.6m room, 2.1, 3 seats".to_string(),
        simulation: make_simulation(room, vec![left, right, sub], vec![lp0, lp1, lp2]),
        source_names: vec!["left".to_string(), "right".to_string(), "subwoofer".to_string()],
    }
}

/// Scenario 12: Medium room, 2 subs + HP mains, 2 listening positions
fn scenario_12_medium_multi_sub_multi_seat() -> Scenario {
    let room = medium_room();
    let left = make_hp_source(Point3D::new(1.2, 0.4, 1.1), "left");
    let right = make_hp_source(Point3D::new(3.8, 0.4, 1.1), "right");
    let sub1 = make_lp_source(Point3D::new(0.15, 0.15, 0.15), "sub1");
    let sub2 = make_lp_source(Point3D::new(4.85, 0.15, 0.15), "sub2");
    let lp0 = Point3D::new(2.5, 2.5, 1.1);
    let lp1 = Point3D::new(2.5, 3.2, 1.1);

    Scenario {
        name: "medium_multi_sub_multi_seat".to_string(),
        description: "Medium 5x4x2.5m room, 2 subs, 2 seats".to_string(),
        simulation: make_simulation(room, vec![left, right, sub1, sub2], vec![lp0, lp1]),
        source_names: vec![
            "left".to_string(),
            "right".to_string(),
            "sub1".to_string(),
            "sub2".to_string(),
        ],
    }
}

// ============================================================================
// Medium room surround: 5 x 4 x 2.5 m
// Speaker placement follows ITU-R BS.775 / Dolby guidelines:
//   - L/R at ±30° from center, front wall
//   - Center at 0° (front wall center)
//   - SL/SR at ±110° from center, behind listener
//   - Sub at front-left corner, floor level
//   - Height channels (Atmos) near ceiling, at ±45° front / ±135° rear
// ============================================================================

/// Scenario 13: Medium room, 5.0 surround (L/R/C/SL/SR), fullrange, 1 LP
fn scenario_13_medium_surround_5_0() -> Scenario {
    let room = medium_room();
    let left = make_fullrange_source(Point3D::new(1.2, 0.4, 1.1), "left");
    let right = make_fullrange_source(Point3D::new(3.8, 0.4, 1.1), "right");
    let center = make_fullrange_source(Point3D::new(2.5, 0.4, 1.1), "center");
    let surr_left = make_fullrange_source(Point3D::new(0.4, 3.5, 1.1), "surround_left");
    let surr_right = make_fullrange_source(Point3D::new(4.6, 3.5, 1.1), "surround_right");
    let lp = Point3D::new(2.5, 2.5, 1.1);

    Scenario {
        name: "medium_surround_5_0".to_string(),
        description: "Medium 5x4x2.5m room, 5.0 surround, fullrange".to_string(),
        simulation: make_simulation(
            room,
            vec![left, right, center, surr_left, surr_right],
            vec![lp],
        ),
        source_names: vec![
            "left".to_string(),
            "right".to_string(),
            "center".to_string(),
            "surround_left".to_string(),
            "surround_right".to_string(),
        ],
    }
}

/// Scenario 14: Medium room, 5.1 surround (L/R/C/SL/SR + LFE), HP mains, 1 LP
fn scenario_14_medium_surround_5_1() -> Scenario {
    let room = medium_room();
    let left = make_hp_source(Point3D::new(1.2, 0.4, 1.1), "left");
    let right = make_hp_source(Point3D::new(3.8, 0.4, 1.1), "right");
    let center = make_hp_source(Point3D::new(2.5, 0.4, 1.1), "center");
    let surr_left = make_hp_source(Point3D::new(0.4, 3.5, 1.1), "surround_left");
    let surr_right = make_hp_source(Point3D::new(4.6, 3.5, 1.1), "surround_right");
    let sub = make_lp_source(Point3D::new(0.5, 0.5, 0.15), "subwoofer");
    let lp = Point3D::new(2.5, 2.5, 1.1);

    Scenario {
        name: "medium_surround_5_1".to_string(),
        description: "Medium 5x4x2.5m room, 5.1 surround".to_string(),
        simulation: make_simulation(
            room,
            vec![left, right, center, surr_left, surr_right, sub],
            vec![lp],
        ),
        source_names: vec![
            "left".to_string(),
            "right".to_string(),
            "center".to_string(),
            "surround_left".to_string(),
            "surround_right".to_string(),
            "subwoofer".to_string(),
        ],
    }
}

/// Scenario 15: Medium room, 5.1.4 Atmos (5.1 + 4 height channels), HP all, 1 LP
fn scenario_15_medium_surround_5_1_4() -> Scenario {
    let room = medium_room();
    // Ear-level speakers
    let left = make_hp_source(Point3D::new(1.2, 0.4, 1.1), "left");
    let right = make_hp_source(Point3D::new(3.8, 0.4, 1.1), "right");
    let center = make_hp_source(Point3D::new(2.5, 0.4, 1.1), "center");
    let surr_left = make_hp_source(Point3D::new(0.4, 3.5, 1.1), "surround_left");
    let surr_right = make_hp_source(Point3D::new(4.6, 3.5, 1.1), "surround_right");
    // Height speakers near ceiling
    let top_fl = make_hp_source(Point3D::new(1.2, 1.0, 2.3), "top_front_left");
    let top_fr = make_hp_source(Point3D::new(3.8, 1.0, 2.3), "top_front_right");
    let top_rl = make_hp_source(Point3D::new(1.2, 3.5, 2.3), "top_rear_left");
    let top_rr = make_hp_source(Point3D::new(3.8, 3.5, 2.3), "top_rear_right");
    // Subwoofer
    let sub = make_lp_source(Point3D::new(0.5, 0.5, 0.15), "subwoofer");
    let lp = Point3D::new(2.5, 2.5, 1.1);

    Scenario {
        name: "medium_surround_5_1_4".to_string(),
        description: "Medium 5x4x2.5m room, 5.1.4 Dolby Atmos".to_string(),
        simulation: make_simulation(
            room,
            vec![
                left, right, center, surr_left, surr_right, top_fl, top_fr, top_rl, top_rr, sub,
            ],
            vec![lp],
        ),
        source_names: vec![
            "left".to_string(),
            "right".to_string(),
            "center".to_string(),
            "surround_left".to_string(),
            "surround_right".to_string(),
            "top_front_left".to_string(),
            "top_front_right".to_string(),
            "top_rear_left".to_string(),
            "top_rear_right".to_string(),
            "subwoofer".to_string(),
        ],
    }
}

// ============================================================================
// Large room surround: 7 x 5.5 x 2.6 m
// ============================================================================

/// Scenario 16: Large room, 5.1 surround, HP mains, 1 LP
fn scenario_16_large_surround_5_1() -> Scenario {
    let room = large_room();
    let left = make_hp_source(Point3D::new(1.5, 0.4, 1.1), "left");
    let right = make_hp_source(Point3D::new(5.5, 0.4, 1.1), "right");
    let center = make_hp_source(Point3D::new(3.5, 0.4, 1.1), "center");
    let surr_left = make_hp_source(Point3D::new(0.5, 4.5, 1.1), "surround_left");
    let surr_right = make_hp_source(Point3D::new(6.5, 4.5, 1.1), "surround_right");
    let sub = make_lp_source(Point3D::new(0.5, 0.5, 0.15), "subwoofer");
    let lp = Point3D::new(3.5, 3.5, 1.1);

    Scenario {
        name: "large_surround_5_1".to_string(),
        description: "Large 7x5.5x2.6m room, 5.1 surround".to_string(),
        simulation: make_simulation(
            room,
            vec![left, right, center, surr_left, surr_right, sub],
            vec![lp],
        ),
        source_names: vec![
            "left".to_string(),
            "right".to_string(),
            "center".to_string(),
            "surround_left".to_string(),
            "surround_right".to_string(),
            "subwoofer".to_string(),
        ],
    }
}

/// Scenario 17: Large room, 5.1.4 Atmos, HP all, 1 LP
fn scenario_17_large_surround_5_1_4() -> Scenario {
    let room = large_room();
    // Ear-level speakers
    let left = make_hp_source(Point3D::new(1.5, 0.4, 1.1), "left");
    let right = make_hp_source(Point3D::new(5.5, 0.4, 1.1), "right");
    let center = make_hp_source(Point3D::new(3.5, 0.4, 1.1), "center");
    let surr_left = make_hp_source(Point3D::new(0.5, 4.5, 1.1), "surround_left");
    let surr_right = make_hp_source(Point3D::new(6.5, 4.5, 1.1), "surround_right");
    // Height speakers near ceiling
    let top_fl = make_hp_source(Point3D::new(1.5, 1.0, 2.4), "top_front_left");
    let top_fr = make_hp_source(Point3D::new(5.5, 1.0, 2.4), "top_front_right");
    let top_rl = make_hp_source(Point3D::new(1.5, 4.5, 2.4), "top_rear_left");
    let top_rr = make_hp_source(Point3D::new(5.5, 4.5, 2.4), "top_rear_right");
    // Subwoofer
    let sub = make_lp_source(Point3D::new(0.5, 0.5, 0.15), "subwoofer");
    let lp = Point3D::new(3.5, 3.5, 1.1);

    Scenario {
        name: "large_surround_5_1_4".to_string(),
        description: "Large 7x5.5x2.6m room, 5.1.4 Dolby Atmos".to_string(),
        simulation: make_simulation(
            room,
            vec![
                left, right, center, surr_left, surr_right, top_fl, top_fr, top_rl, top_rr, sub,
            ],
            vec![lp],
        ),
        source_names: vec![
            "left".to_string(),
            "right".to_string(),
            "center".to_string(),
            "surround_left".to_string(),
            "surround_right".to_string(),
            "top_front_left".to_string(),
            "top_front_right".to_string(),
            "top_rear_left".to_string(),
            "top_rear_right".to_string(),
            "subwoofer".to_string(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_scenarios_count() {
        let scenarios = all_scenarios();
        assert_eq!(scenarios.len(), 18);
    }

    #[test]
    fn test_scenario_names_unique() {
        let scenarios = all_scenarios();
        let names: Vec<&str> = scenarios.iter().map(|s| s.name.as_str()).collect();
        let mut sorted = names.clone();
        sorted.sort();
        sorted.dedup();
        assert_eq!(names.len(), sorted.len(), "Scenario names must be unique");
    }

    #[test]
    fn test_all_scenarios_have_valid_frequencies() {
        for scenario in all_scenarios() {
            assert_eq!(
                scenario.simulation.frequencies.len(),
                NUM_POINTS,
                "Scenario {} should have {} frequency points",
                scenario.name,
                NUM_POINTS,
            );
            assert!(scenario.simulation.frequencies[0] >= MIN_FREQ - 1.0);
            assert!(
                *scenario.simulation.frequencies.last().unwrap() <= MAX_FREQ + 1.0
            );
        }
    }

    #[test]
    fn test_source_names_match_sources() {
        for scenario in all_scenarios() {
            assert_eq!(
                scenario.source_names.len(),
                scenario.simulation.sources.len(),
                "Scenario {} source_names/sources mismatch",
                scenario.name,
            );
        }
    }

    #[test]
    fn test_scenario_lookup() {
        let s = scenario_by_name("medium_stereo_2_0");
        assert!(s.is_some());
        assert!(scenario_by_name("nonexistent").is_none());
    }
}
