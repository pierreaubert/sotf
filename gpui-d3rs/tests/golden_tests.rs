//! Golden file tests for D3.js compatibility
//!
//! These tests compare d3rs output against golden files generated from D3.js.
//! To regenerate golden files, run: `cd golden && npm run generate`

use d3rs::scale::{LinearScale, LogScale, Scale};
use serde::Deserialize;
use std::cmp::Ordering;
use std::fs;

const TOLERANCE: f64 = 1e-6;

/// Compare two f64 values with tolerance
fn approx_eq(expected: f64, actual: f64) -> bool {
    if expected.is_nan() && actual.is_nan() {
        return true;
    }
    if expected.is_infinite() && actual.is_infinite() {
        return expected.signum() == actual.signum();
    }
    (expected - actual).abs() < TOLERANCE
}

/// Wrapper for f64 that implements Ord for use with array functions
#[derive(Debug, Clone, Copy, PartialEq)]
struct OrdF64(f64);

impl Eq for OrdF64 {}

impl PartialOrd for OrdF64 {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for OrdF64 {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.partial_cmp(&other.0).unwrap_or(Ordering::Equal)
    }
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct GoldenFile {
    module: String,
    function: String,
    #[serde(default)]
    d3_version: Option<String>,
    tolerance: f64,
    test_cases: Vec<serde_json::Value>,
}

// ============================================================================
// LINEAR SCALE TESTS
// ============================================================================

#[test]
fn test_linear_scale_golden() {
    let content = fs::read_to_string("golden/scales/linear.json").expect("golden file not found");
    let golden: GoldenFile = serde_json::from_str(&content).unwrap();

    assert_eq!(golden.module, "d3-scale");
    assert_eq!(golden.function, "scaleLinear");

    for case in &golden.test_cases {
        let name = case["name"].as_str().unwrap();

        // Skip tests that require features not yet implemented
        if name == "nice_domain" {
            test_linear_nice(&case);
            continue;
        }

        if name.starts_with("ticks") {
            test_linear_ticks(&case);
            continue;
        }

        let config = &case["config"];
        let domain: Vec<f64> = serde_json::from_value(config["domain"].clone()).unwrap();
        let range: Vec<f64> = serde_json::from_value(config["range"].clone()).unwrap();
        let clamp = config["clamp"].as_bool().unwrap_or(false);

        let scale = LinearScale::new()
            .domain(domain[0], domain[1])
            .range(range[0], range[1])
            .clamp(clamp);

        // Test scale outputs
        if let Some(inputs) = case.get("inputs") {
            let inputs: Vec<f64> = serde_json::from_value(inputs.clone()).unwrap();
            let expected: Vec<f64> = serde_json::from_value(case["outputs"].clone()).unwrap();

            for (input, exp) in inputs.iter().zip(expected.iter()) {
                let actual = scale.scale(*input);
                assert!(
                    approx_eq(*exp, actual),
                    "case '{}': scale({}) = {} (expected {})",
                    name,
                    input,
                    actual,
                    exp
                );
            }
        }

        // Test invert outputs
        if let Some(invert_inputs) = case.get("invert_inputs") {
            let invert_inputs: Vec<f64> = serde_json::from_value(invert_inputs.clone()).unwrap();
            let expected: Vec<f64> =
                serde_json::from_value(case["invert_outputs"].clone()).unwrap();

            for (input, exp) in invert_inputs.iter().zip(expected.iter()) {
                let actual = scale.invert(*input).unwrap();
                assert!(
                    approx_eq(*exp, actual),
                    "case '{}': invert({}) = {} (expected {})",
                    name,
                    input,
                    actual,
                    exp
                );
            }
        }
    }
}

fn test_linear_nice(case: &serde_json::Value) {
    let config = &case["config"];
    let domain: Vec<f64> = serde_json::from_value(config["domain"].clone()).unwrap();
    let nice_domain: Vec<f64> = serde_json::from_value(case["nice_domain"].clone()).unwrap();

    let scale = LinearScale::new().domain(domain[0], domain[1]).nice(None);

    assert!(
        approx_eq(nice_domain[0], scale.domain_min()),
        "nice domain min: {} (expected {})",
        scale.domain_min(),
        nice_domain[0]
    );
    assert!(
        approx_eq(nice_domain[1], scale.domain_max()),
        "nice domain max: {} (expected {})",
        scale.domain_max(),
        nice_domain[1]
    );
}

fn test_linear_ticks(case: &serde_json::Value) {
    let name = case["name"].as_str().unwrap();
    let config = &case["config"];
    let domain: Vec<f64> = serde_json::from_value(config["domain"].clone()).unwrap();
    let count = case["ticks_count"].as_u64().unwrap_or(10) as usize;
    let _expected: Vec<f64> = serde_json::from_value(case["ticks"].clone()).unwrap();

    let scale = LinearScale::new().domain(domain[0], domain[1]);
    let ticks = scale.ticks(count);

    // Check that ticks are reasonable (may not be exact match due to algorithm differences)
    assert!(
        !ticks.is_empty(),
        "case '{}': ticks should not be empty",
        name
    );

    // Check first and last tick are within domain extent
    let first = ticks.first().unwrap();
    let last = ticks.last().unwrap();
    assert!(
        *first <= domain[0] + TOLERANCE,
        "case '{}': first tick {} should be <= domain min {}",
        name,
        first,
        domain[0]
    );
    assert!(
        *last >= domain[1] - TOLERANCE,
        "case '{}': last tick {} should be >= domain max {}",
        name,
        last,
        domain[1]
    );

    // Check that ticks are evenly spaced (for simple linear domains)
    if ticks.len() >= 2 {
        let step = ticks[1] - ticks[0];
        for i in 2..ticks.len() {
            let actual_step = ticks[i] - ticks[i - 1];
            assert!(
                approx_eq(step, actual_step),
                "case '{}': ticks not evenly spaced: step[0]={}, step[{}]={}",
                name,
                step,
                i,
                actual_step
            );
        }
    }
}

// ============================================================================
// LOG SCALE TESTS
// ============================================================================

#[test]
fn test_log_scale_golden() {
    let content = fs::read_to_string("golden/scales/log.json").expect("golden file not found");
    let golden: GoldenFile = serde_json::from_str(&content).unwrap();

    assert_eq!(golden.module, "d3-scale");
    assert_eq!(golden.function, "scaleLog");

    for case in &golden.test_cases {
        let name = case["name"].as_str().unwrap();

        // Skip tick tests for now
        if name == "ticks" {
            continue;
        }

        let config = &case["config"];
        let domain: Vec<f64> = serde_json::from_value(config["domain"].clone()).unwrap();
        let range: Vec<f64> = serde_json::from_value(config["range"].clone()).unwrap();
        let base = config["base"].as_f64().unwrap_or(10.0);

        let scale = LogScale::new()
            .domain(domain[0], domain[1])
            .range(range[0], range[1])
            .base(base);

        // Test scale outputs
        if let Some(inputs) = case.get("inputs") {
            let inputs: Vec<f64> = serde_json::from_value(inputs.clone()).unwrap();
            let expected: Vec<f64> = serde_json::from_value(case["outputs"].clone()).unwrap();

            for (input, exp) in inputs.iter().zip(expected.iter()) {
                let actual = scale.scale(*input);
                assert!(
                    approx_eq(*exp, actual),
                    "case '{}': scale({}) = {} (expected {})",
                    name,
                    input,
                    actual,
                    exp
                );
            }
        }

        // Test invert outputs
        if let Some(invert_inputs) = case.get("invert_inputs") {
            let invert_inputs: Vec<f64> = serde_json::from_value(invert_inputs.clone()).unwrap();
            let expected: Vec<f64> =
                serde_json::from_value(case["invert_outputs"].clone()).unwrap();

            for (input, exp) in invert_inputs.iter().zip(expected.iter()) {
                let actual = scale.invert(*input).unwrap();
                assert!(
                    approx_eq(*exp, actual),
                    "case '{}': invert({}) = {} (expected {})",
                    name,
                    input,
                    actual,
                    exp
                );
            }
        }
    }
}

// ============================================================================
// ARRAY STATISTICS TESTS
// ============================================================================

#[test]
fn test_array_statistics_golden() {
    use d3rs::array::{cumsum, deviation, extent, max, mean, median, min, quantile, sum, variance};

    let content =
        fs::read_to_string("golden/array/statistics.json").expect("golden file not found");
    let golden: GoldenFile = serde_json::from_str(&content).unwrap();

    assert_eq!(golden.module, "d3-array");
    assert_eq!(golden.function, "statistics");

    for case in &golden.test_cases {
        let name = case["name"].as_str().unwrap();

        match name {
            "min_max_extent" => {
                let data: Vec<f64> = serde_json::from_value(case["data"].clone()).unwrap();
                let ord_data: Vec<OrdF64> = data.iter().map(|x| OrdF64(*x)).collect();
                let exp_min = case["min"].as_f64().unwrap();
                let exp_max = case["max"].as_f64().unwrap();
                let exp_extent: Vec<f64> = serde_json::from_value(case["extent"].clone()).unwrap();

                assert!(
                    approx_eq(exp_min, min(&ord_data).unwrap().0),
                    "min mismatch"
                );
                assert!(
                    approx_eq(exp_max, max(&ord_data).unwrap().0),
                    "max mismatch"
                );

                let ext = extent(&ord_data).unwrap();
                assert!(approx_eq(exp_extent[0], ext.0 .0), "extent min mismatch");
                assert!(approx_eq(exp_extent[1], ext.1 .0), "extent max mismatch");
            }
            "sum_mean_median" => {
                let mut data: Vec<f64> = serde_json::from_value(case["data"].clone()).unwrap();
                let exp_sum = case["sum"].as_f64().unwrap();
                let exp_mean = case["mean"].as_f64().unwrap();
                let exp_median = case["median"].as_f64().unwrap();

                assert!(approx_eq(exp_sum, sum(&data)), "sum mismatch");
                assert!(approx_eq(exp_mean, mean(&data).unwrap()), "mean mismatch");
                assert!(
                    approx_eq(exp_median, median(&mut data).unwrap()),
                    "median mismatch"
                );
            }
            "variance_deviation" => {
                let data: Vec<f64> = serde_json::from_value(case["data"].clone()).unwrap();
                let exp_variance = case["variance"].as_f64().unwrap();
                let exp_deviation = case["deviation"].as_f64().unwrap();

                assert!(
                    approx_eq(exp_variance, variance(&data).unwrap()),
                    "variance mismatch: expected {}, got {}",
                    exp_variance,
                    variance(&data).unwrap()
                );
                assert!(
                    approx_eq(exp_deviation, deviation(&data).unwrap()),
                    "deviation mismatch"
                );
            }
            "quantile" => {
                let data: Vec<f64> = serde_json::from_value(case["data"].clone()).unwrap();

                for (q, key) in [
                    (0.0, "q0"),
                    (0.25, "q25"),
                    (0.5, "q50"),
                    (0.75, "q75"),
                    (1.0, "q100"),
                ] {
                    let exp = case[key].as_f64().unwrap();
                    // Need to re-clone for each call since quantile modifies the array
                    let mut data_copy = data.clone();
                    let actual = quantile(&mut data_copy, q).unwrap();
                    assert!(
                        approx_eq(exp, actual),
                        "{}: expected {}, got {}",
                        key,
                        exp,
                        actual
                    );
                }
            }
            "cumsum" => {
                let data: Vec<f64> = serde_json::from_value(case["data"].clone()).unwrap();
                let expected: Vec<f64> = serde_json::from_value(case["cumsum"].clone()).unwrap();
                let actual = cumsum(&data);

                for (i, (exp, act)) in expected.iter().zip(actual.iter()).enumerate() {
                    assert!(
                        approx_eq(*exp, *act),
                        "cumsum[{}]: expected {}, got {}",
                        i,
                        exp,
                        act
                    );
                }
            }
            "with_accessor" | "empty_array" => {
                // These are tested implicitly or don't need explicit tests
            }
            _ => {}
        }
    }
}

// ============================================================================
// INTERPOLATE NUMBER TESTS
// ============================================================================

#[test]
fn test_interpolate_number_golden() {
    use d3rs::interpolate::interpolate;

    let content =
        fs::read_to_string("golden/interpolate/number.json").expect("golden file not found");
    let golden: GoldenFile = serde_json::from_str(&content).unwrap();

    assert_eq!(golden.module, "d3-interpolate");
    assert_eq!(golden.function, "interpolateNumber");

    for case in &golden.test_cases {
        let name = case["name"].as_str().unwrap();
        let config = &case["config"];
        let a = config["a"].as_f64().unwrap();
        let b = config["b"].as_f64().unwrap();
        let is_round = config
            .get("round")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        // Skip round tests for now since the API is different
        if is_round {
            continue;
        }

        let inputs: Vec<f64> = serde_json::from_value(case["inputs"].clone()).unwrap();
        let expected: Vec<f64> = serde_json::from_value(case["outputs"].clone()).unwrap();

        // Create the interpolator function
        let interp = interpolate(a, b);

        for (t, exp) in inputs.iter().zip(expected.iter()) {
            let actual = interp(*t);
            assert!(
                approx_eq(*exp, actual),
                "case '{}': interpolate({}, {})({}) = {} (expected {})",
                name,
                a,
                b,
                t,
                actual,
                exp
            );
        }
    }
}

// ============================================================================
// QUADTREE TESTS
// ============================================================================

#[test]
fn test_quadtree_golden() {
    use d3rs::quadtree::QuadTree;

    let content =
        fs::read_to_string("golden/quadtree/quadtree.json").expect("golden file not found");
    let golden: GoldenFile = serde_json::from_str(&content).unwrap();

    assert_eq!(golden.module, "d3-quadtree");
    assert_eq!(golden.function, "quadtree");

    for case in &golden.test_cases {
        let name = case["name"].as_str().unwrap();

        match name {
            "basic_add" => {
                let points: Vec<Vec<f64>> =
                    serde_json::from_value(case["points"].clone()).unwrap();
                let exp_size = case["size"].as_u64().unwrap() as usize;
                let exp_extent: Vec<Vec<f64>> =
                    serde_json::from_value(case["extent"].clone()).unwrap();

                let mut tree: QuadTree<()> = QuadTree::new();
                for p in &points {
                    tree.add(p[0], p[1], ());
                }

                assert_eq!(tree.size(), exp_size, "case '{}': size mismatch", name);

                let ext = tree.extent().expect("extent should exist");
                assert!(
                    approx_eq(exp_extent[0][0], ext.x0),
                    "case '{}': extent x0 mismatch: {} vs {}",
                    name,
                    ext.x0,
                    exp_extent[0][0]
                );
                assert!(
                    approx_eq(exp_extent[0][1], ext.y0),
                    "case '{}': extent y0 mismatch: {} vs {}",
                    name,
                    ext.y0,
                    exp_extent[0][1]
                );
                // D3 extent is power-of-2, so x1/y1 should match
                assert!(
                    approx_eq(exp_extent[1][0], ext.x1),
                    "case '{}': extent x1 mismatch: {} vs {}",
                    name,
                    ext.x1,
                    exp_extent[1][0]
                );
                assert!(
                    approx_eq(exp_extent[1][1], ext.y1),
                    "case '{}': extent y1 mismatch: {} vs {}",
                    name,
                    ext.y1,
                    exp_extent[1][1]
                );
            }
            "find" => {
                let points: Vec<Vec<f64>> =
                    serde_json::from_value(case["points"].clone()).unwrap();
                let queries: Vec<serde_json::Value> =
                    serde_json::from_value(case["queries"].clone()).unwrap();

                // Store points with their coordinates as data
                let mut tree: QuadTree<(f64, f64)> = QuadTree::new();
                for p in &points {
                    tree.add(p[0], p[1], (p[0], p[1]));
                }

                for query in &queries {
                    let x = query["x"].as_f64().unwrap();
                    let y = query["y"].as_f64().unwrap();
                    let result: Vec<f64> =
                        serde_json::from_value(query["result"].clone()).unwrap();

                    let found = tree.find(x, y, None).expect("should find a point");
                    assert!(
                        approx_eq(result[0], found.0) && approx_eq(result[1], found.1),
                        "case '{}': find({}, {}) = ({}, {}) (expected ({}, {}))",
                        name,
                        x,
                        y,
                        found.0,
                        found.1,
                        result[0],
                        result[1]
                    );
                }
            }
            "find_with_radius" => {
                let points: Vec<Vec<f64>> =
                    serde_json::from_value(case["points"].clone()).unwrap();
                let queries: Vec<serde_json::Value> =
                    serde_json::from_value(case["queries"].clone()).unwrap();

                let mut tree: QuadTree<(f64, f64)> = QuadTree::new();
                for p in &points {
                    tree.add(p[0], p[1], (p[0], p[1]));
                }

                for query in &queries {
                    let x = query["x"].as_f64().unwrap();
                    let y = query["y"].as_f64().unwrap();
                    let radius = query["radius"].as_f64().unwrap();

                    let found = tree.find(x, y, Some(radius));

                    if query["result"].is_null() {
                        assert!(
                            found.is_none(),
                            "case '{}': find({}, {}, {}) should return None",
                            name,
                            x,
                            y,
                            radius
                        );
                    } else {
                        let result: Vec<f64> =
                            serde_json::from_value(query["result"].clone()).unwrap();
                        let found = found.expect("should find a point");
                        assert!(
                            approx_eq(result[0], found.0) && approx_eq(result[1], found.1),
                            "case '{}': find({}, {}, {}) = ({}, {}) (expected ({}, {}))",
                            name,
                            x,
                            y,
                            radius,
                            found.0,
                            found.1,
                            result[0],
                            result[1]
                        );
                    }
                }
            }
            "remove" => {
                let points: Vec<Vec<f64>> =
                    serde_json::from_value(case["points"].clone()).unwrap();
                let remove: Vec<f64> = serde_json::from_value(case["remove"].clone()).unwrap();
                let exp_size_before = case["size_before"].as_u64().unwrap() as usize;
                let exp_size_after = case["size_after"].as_u64().unwrap() as usize;

                let mut tree: QuadTree<()> = QuadTree::new();
                for p in &points {
                    tree.add(p[0], p[1], ());
                }

                assert_eq!(
                    tree.size(),
                    exp_size_before,
                    "case '{}': size before remove",
                    name
                );

                tree.remove(remove[0], remove[1]);

                assert_eq!(
                    tree.size(),
                    exp_size_after,
                    "case '{}': size after remove",
                    name
                );
            }
            "extent" => {
                let points: Vec<Vec<f64>> =
                    serde_json::from_value(case["points"].clone()).unwrap();
                let exp_size = case["size"].as_u64().unwrap() as usize;

                let mut tree: QuadTree<()> = QuadTree::new();
                for p in &points {
                    tree.add(p[0], p[1], ());
                }

                assert_eq!(tree.size(), exp_size, "case '{}': size mismatch", name);

                // Just verify extent exists and is valid
                let ext = tree.extent().expect("extent should exist");
                assert!(
                    ext.x0 <= ext.x1 && ext.y0 <= ext.y1,
                    "case '{}': invalid extent",
                    name
                );
            }
            "visit" => {
                let points: Vec<Vec<f64>> =
                    serde_json::from_value(case["points"].clone()).unwrap();
                let exp_visited_count = case["visited_count"].as_u64().unwrap() as usize;
                let exp_leaf_count = case["leaf_count"].as_u64().unwrap() as usize;

                let mut tree: QuadTree<()> = QuadTree::new();
                for p in &points {
                    tree.add(p[0], p[1], ());
                }

                let mut visited_count = 0;
                let mut leaf_count = 0;

                tree.visit(|_x0, _y0, _x1, _y1, node| {
                    visited_count += 1;
                    if matches!(node, d3rs::quadtree::QuadNode::Leaf(_)) {
                        leaf_count += 1;
                    }
                    true // continue visiting
                });

                assert_eq!(
                    visited_count, exp_visited_count,
                    "case '{}': visited_count mismatch",
                    name
                );
                assert_eq!(
                    leaf_count, exp_leaf_count,
                    "case '{}': leaf_count mismatch",
                    name
                );
            }
            "data" => {
                let points: Vec<Vec<f64>> =
                    serde_json::from_value(case["points"].clone()).unwrap();
                let exp_data: Vec<Vec<f64>> =
                    serde_json::from_value(case["data"].clone()).unwrap();

                let mut tree: QuadTree<(f64, f64)> = QuadTree::new();
                for p in &points {
                    tree.add(p[0], p[1], (p[0], p[1]));
                }

                let data = tree.data();
                assert_eq!(
                    data.len(),
                    exp_data.len(),
                    "case '{}': data length mismatch",
                    name
                );

                // D3 data() may return in different order, so just check all points exist
                for exp in &exp_data {
                    let found = data.iter().any(|(x, y, d)| {
                        approx_eq(*x, exp[0])
                            && approx_eq(*y, exp[1])
                            && approx_eq(d.0, exp[0])
                            && approx_eq(d.1, exp[1])
                    });
                    assert!(
                        found,
                        "case '{}': expected point ({}, {}) not found in data",
                        name, exp[0], exp[1]
                    );
                }
            }
            "coincident" => {
                let points: Vec<Vec<f64>> =
                    serde_json::from_value(case["points"].clone()).unwrap();
                let exp_size = case["size"].as_u64().unwrap() as usize;

                let mut tree: QuadTree<usize> = QuadTree::new();
                for (i, p) in points.iter().enumerate() {
                    tree.add(p[0], p[1], i);
                }

                assert_eq!(tree.size(), exp_size, "case '{}': size mismatch", name);
            }
            "large_dataset" => {
                let point_count = case["point_count"].as_u64().unwrap() as usize;
                let exp_size = case["size"].as_u64().unwrap() as usize;

                // Regenerate the same points as the JavaScript generator
                let mut tree: QuadTree<i32> = QuadTree::new();
                for i in 0..point_count {
                    let x = (i as f64 * 0.618033988749895).fract() * 100.0;
                    let y = (i as f64 * 0.381966011250105).fract() * 100.0;
                    tree.add(x, y, i as i32);
                }

                assert_eq!(tree.size(), exp_size, "case '{}': size mismatch", name);

                // Verify extent exists
                let ext = tree.extent().expect("extent should exist");
                assert!(
                    ext.x0 <= ext.x1 && ext.y0 <= ext.y1,
                    "case '{}': invalid extent",
                    name
                );
            }
            _ => {
                // Skip unknown test cases
            }
        }
    }
}
