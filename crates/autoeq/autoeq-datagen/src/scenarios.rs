//! Room scenario definitions for RoomEQ data generation
//!
//! Defines 19 scenarios covering small/medium/large rooms with
//! stereo, 2.1, multi-sub, multi-seat, and surround (5.0/5.1/5.1.4) configurations.

use math_audio_xem_common::{
    BoundaryConfig, CrossoverFilter, Point3D, RectangularRoom, RoomGeometry, RoomSimulation,
    Source, SurfaceConfig,
};
use rand::rngs::SmallRng;
use rand::{Rng, SeedableRng};

/// A named scenario for data generation
#[derive(Clone)]
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

fn default_boundaries() -> BoundaryConfig {
    BoundaryConfig {
        floor: SurfaceConfig::Absorption { coefficient: 0.3 },
        ceiling: SurfaceConfig::Absorption { coefficient: 0.05 },
        walls: SurfaceConfig::Absorption { coefficient: 0.1 },
        ..Default::default()
    }
}

fn concrete_basement_boundaries() -> BoundaryConfig {
    BoundaryConfig {
        floor: SurfaceConfig::Absorption { coefficient: 0.1 },
        ceiling: SurfaceConfig::Absorption { coefficient: 0.1 },
        walls: SurfaceConfig::Absorption { coefficient: 0.05 },
        ..Default::default()
    }
}

fn treated_front_boundaries() -> BoundaryConfig {
    BoundaryConfig {
        floor: SurfaceConfig::Absorption { coefficient: 0.3 },
        ceiling: SurfaceConfig::Absorption { coefficient: 0.1 },
        walls: SurfaceConfig::Absorption { coefficient: 0.08 },
        front_wall: Some(SurfaceConfig::Absorption { coefficient: 0.4 }),
        back_wall: Some(SurfaceConfig::Absorption { coefficient: 0.2 }),
        left_wall: None,
        right_wall: None,
    }
}

fn asymmetric_side_boundaries() -> BoundaryConfig {
    BoundaryConfig {
        floor: SurfaceConfig::Absorption { coefficient: 0.25 },
        ceiling: SurfaceConfig::Absorption { coefficient: 0.08 },
        walls: SurfaceConfig::Absorption { coefficient: 0.12 },
        front_wall: None,
        back_wall: None,
        left_wall: Some(SurfaceConfig::Absorption { coefficient: 0.25 }),
        right_wall: Some(SurfaceConfig::Absorption { coefficient: 0.05 }),
    }
}

fn make_simulation_with_boundaries(
    room: RectangularRoom,
    sources: Vec<Source>,
    listening_positions: Vec<Point3D>,
    boundaries: BoundaryConfig,
) -> RoomSimulation {
    let mut sim = RoomSimulation::with_frequencies(
        RoomGeometry::Rectangular(room),
        sources,
        listening_positions,
        MIN_FREQ,
        MAX_FREQ,
        NUM_POINTS,
    );
    sim.boundaries = boundaries;
    sim
}

fn make_simulation(
    room: RectangularRoom,
    sources: Vec<Source>,
    listening_positions: Vec<Point3D>,
) -> RoomSimulation {
    make_simulation_with_boundaries(room, sources, listening_positions, default_boundaries())
}

/// Generate all 19 scenarios
pub fn all_scenarios() -> Vec<Scenario> {
    vec![
        scenario_01_small_stereo_2_0(),
        scenario_02_small_stereo_2_1(),
        scenario_03_small_stereo_2_2_mso(),
        scenario_03_small_stereo_2_2_cardioid(),
        scenario_03_small_stereo_2_2_group(),
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
        source_names: vec![
            "left".to_string(),
            "right".to_string(),
            "subwoofer".to_string(),
        ],
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
        simulation: make_simulation_with_boundaries(
            room,
            vec![left, right, sub1, sub2],
            vec![lp],
            concrete_basement_boundaries(),
        ),
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

/// Scenario 3c: Small room, 2 subs (grouped with mains) + HP mains, 1 LP
/// Each sub is directly below its corresponding main speaker.
/// Naming convention: `{main}_sub` signals Group config to config gen.
fn scenario_03_small_stereo_2_2_group() -> Scenario {
    let room = small_room();
    let left = make_hp_source(Point3D::new(0.8, 0.3, 1.1), "left");
    let right = make_hp_source(Point3D::new(2.2, 0.3, 1.1), "right");
    // Subs directly below their respective mains, on the floor
    let left_sub = make_lp_source(Point3D::new(0.8, 0.3, 0.15), "left_sub");
    let right_sub = make_lp_source(Point3D::new(2.2, 0.3, 0.15), "right_sub");
    let lp = Point3D::new(1.5, 2.0, 1.1);

    Scenario {
        name: "small_stereo_2_2_group".to_string(),
        description: "Small 3x3x2.4m room, grouped subs below mains".to_string(),
        simulation: make_simulation(room, vec![left, right, left_sub, right_sub], vec![lp]),
        source_names: vec![
            "left".to_string(),
            "right".to_string(),
            "left_sub".to_string(),
            "right_sub".to_string(),
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
        source_names: vec![
            "left".to_string(),
            "right".to_string(),
            "subwoofer".to_string(),
        ],
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
        simulation: make_simulation_with_boundaries(
            room,
            vec![left, right, sub1, sub2, sub3, sub4],
            vec![lp],
            treated_front_boundaries(),
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
        simulation: make_simulation_with_boundaries(
            room,
            vec![left, right],
            vec![lp0, lp1, lp2],
            asymmetric_side_boundaries(),
        ),
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
        simulation: make_simulation_with_boundaries(
            room,
            vec![left, right],
            vec![lp],
            treated_front_boundaries(),
        ),
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
        simulation: make_simulation_with_boundaries(
            room,
            vec![left, right, sub],
            vec![lp],
            treated_front_boundaries(),
        ),
        source_names: vec![
            "left".to_string(),
            "right".to_string(),
            "subwoofer".to_string(),
        ],
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
        simulation: make_simulation_with_boundaries(
            room,
            vec![left, right, sub1, sub2, sub3, sub4],
            vec![lp],
            concrete_basement_boundaries(),
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
        simulation: make_simulation_with_boundaries(
            room,
            vec![left, right, sub],
            vec![lp0, lp1, lp2],
            asymmetric_side_boundaries(),
        ),
        source_names: vec![
            "left".to_string(),
            "right".to_string(),
            "subwoofer".to_string(),
        ],
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
        simulation: make_simulation_with_boundaries(
            room,
            vec![left, right, sub1, sub2],
            vec![lp0, lp1],
            treated_front_boundaries(),
        ),
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
        simulation: make_simulation_with_boundaries(
            room,
            vec![left, right, center, surr_left, surr_right],
            vec![lp],
            treated_front_boundaries(),
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
        simulation: make_simulation_with_boundaries(
            room,
            vec![left, right, center, surr_left, surr_right, sub],
            vec![lp],
            treated_front_boundaries(),
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
        simulation: make_simulation_with_boundaries(
            room,
            vec![
                left, right, center, surr_left, surr_right, top_fl, top_fr, top_rl, top_rr, sub,
            ],
            vec![lp],
            treated_front_boundaries(),
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
        simulation: make_simulation_with_boundaries(
            room,
            vec![left, right, center, surr_left, surr_right, sub],
            vec![lp],
            asymmetric_side_boundaries(),
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
        simulation: make_simulation_with_boundaries(
            room,
            vec![
                left, right, center, surr_left, surr_right, top_fl, top_fr, top_rl, top_rr, sub,
            ],
            vec![lp],
            asymmetric_side_boundaries(),
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

fn jitter_absorption_coefficient(rng: &mut SmallRng, base: f64, amount: f64) -> f64 {
    let delta: f64 = rng.random_range(-amount..amount);
    let value = base + delta;
    value.max(0.02).min(0.9)
}

fn jitter_surface(
    rng: &mut SmallRng,
    surface: &SurfaceConfig,
    amount: f64,
) -> SurfaceConfig {
    match surface {
        SurfaceConfig::Absorption { coefficient } => SurfaceConfig::Absorption {
            coefficient: jitter_absorption_coefficient(rng, *coefficient, amount),
        },
        other => other.clone(),
    }
}

fn jitter_boundaries(
    rng: &mut SmallRng,
    base: &BoundaryConfig,
    amount: f64,
) -> BoundaryConfig {
    BoundaryConfig {
        floor: jitter_surface(rng, &base.floor, amount),
        ceiling: jitter_surface(rng, &base.ceiling, amount),
        walls: jitter_surface(rng, &base.walls, amount),
        front_wall: base
            .front_wall
            .as_ref()
            .map(|s| jitter_surface(rng, s, amount)),
        back_wall: base
            .back_wall
            .as_ref()
            .map(|s| jitter_surface(rng, s, amount)),
        left_wall: base
            .left_wall
            .as_ref()
            .map(|s| jitter_surface(rng, s, amount)),
        right_wall: base
            .right_wall
            .as_ref()
            .map(|s| jitter_surface(rng, s, amount)),
    }
}

fn jitter_point_within_room(
    rng: &mut SmallRng,
    p: Point3D,
    max_dx: f64,
    max_dy: f64,
    max_dz: f64,
    room: &RectangularRoom,
) -> Point3D {
    let dx: f64 = rng.random_range(-max_dx..max_dx);
    let dy: f64 = rng.random_range(-max_dy..max_dy);
    let dz: f64 = rng.random_range(-max_dz..max_dz);

    let mut x = p.x + dx;
    let mut y = p.y + dy;
    let mut z = p.z + dz;

    if x < 0.1 {
        x = 0.1;
    }
    if x > room.width - 0.1 {
        x = room.width - 0.1;
    }
    if y < 0.1 {
        y = 0.1;
    }
    if y > room.depth - 0.1 {
        y = room.depth - 0.1;
    }
    if z < 0.1 {
        z = 0.1;
    }
    if z > room.height - 0.1 {
        z = room.height - 0.1;
    }

    Point3D::new(x, y, z)
}

fn randomized_scenario_variant(
    base: &Scenario,
    variant_index: usize,
    seed: u64,
) -> Scenario {
    let mut rng = SmallRng::seed_from_u64(seed);
    let mut simulation = base.simulation.clone();

    let room = match simulation.room {
        RoomGeometry::Rectangular(ref room) => room,
        _ => return base.clone(),
    };

    let max_dx = room.width * 0.03;
    let max_dy = room.depth * 0.03;
    let max_dz = room.height * 0.02;

    for source in &mut simulation.sources {
        source.position = jitter_point_within_room(
            &mut rng,
            source.position,
            max_dx,
            max_dy,
            max_dz,
            &room,
        );
    }

    let lp_max_dx = room.width * 0.04;
    let lp_max_dy = room.depth * 0.04;
    let lp_max_dz = room.height * 0.03;

    for lp in &mut simulation.listening_positions {
        *lp = jitter_point_within_room(
            &mut rng,
            *lp,
            lp_max_dx,
            lp_max_dy,
            lp_max_dz,
            &room,
        );
    }

    simulation.boundaries = jitter_boundaries(&mut rng, &simulation.boundaries, 0.05);

    Scenario {
        name: format!("{}_v{}", base.name, variant_index + 1),
        description: base.description.clone(),
        simulation,
        source_names: base.source_names.clone(),
    }
}

pub fn randomized_scenarios(seed: u64, variants_per_scenario: usize) -> Vec<Scenario> {
    let base = all_scenarios();
    let mut out = Vec::new();

    for (idx, scenario) in base.into_iter().enumerate() {
        let scenario_seed = seed ^ ((idx as u64) << 32);
        for v in 0..variants_per_scenario {
            let seed_v = scenario_seed ^ (v as u64);
            out.push(randomized_scenario_variant(&scenario, v, seed_v));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_scenarios_count() {
        let scenarios = all_scenarios();
        assert_eq!(scenarios.len(), 19);
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
            assert!(*scenario.simulation.frequencies.last().unwrap() <= MAX_FREQ + 1.0);
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
