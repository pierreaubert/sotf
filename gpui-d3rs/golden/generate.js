/**
 * Golden file generator for d3rs compatibility testing
 *
 * This script generates JSON golden files containing D3.js outputs
 * that can be compared against Rust implementations.
 *
 * Usage:
 *   node generate.js           # Generate all golden files
 *   node generate.js scales    # Generate only scale tests
 *   node generate.js interpolate
 *   node generate.js array
 *   node generate.js color
 *   node generate.js shape
 */

const d3 = require('d3');
const fs = require('fs');
const path = require('path');

const TOLERANCE = 1e-6;

// Utility to create golden file structure
function createGoldenFile(module, func, testCases) {
  return {
    module,
    function: func,
    d3_version: d3.version,
    tolerance: TOLERANCE,
    generated_at: new Date().toISOString(),
    test_cases: testCases
  };
}

// ============================================================================
// SCALE GENERATORS
// ============================================================================

function generateLinearScaleTests() {
  const testCases = [];

  // Basic domain/range
  {
    const scale = d3.scaleLinear().domain([0, 100]).range([0, 500]);
    const inputs = [0, 25, 50, 75, 100];
    testCases.push({
      name: "basic_domain_range",
      config: { domain: [0, 100], range: [0, 500] },
      inputs,
      outputs: inputs.map(v => scale(v)),
      invert_inputs: [0, 125, 250, 375, 500],
      invert_outputs: [0, 125, 250, 375, 500].map(v => scale.invert(v))
    });
  }

  // Inverted range
  {
    const scale = d3.scaleLinear().domain([0, 100]).range([500, 0]);
    const inputs = [0, 50, 100];
    testCases.push({
      name: "inverted_range",
      config: { domain: [0, 100], range: [500, 0] },
      inputs,
      outputs: inputs.map(v => scale(v))
    });
  }

  // Negative domain
  {
    const scale = d3.scaleLinear().domain([-100, 100]).range([0, 1]);
    const inputs = [-100, -50, 0, 50, 100];
    testCases.push({
      name: "negative_domain",
      config: { domain: [-100, 100], range: [0, 1] },
      inputs,
      outputs: inputs.map(v => scale(v))
    });
  }

  // Extrapolation (values outside domain)
  {
    const scale = d3.scaleLinear().domain([0, 100]).range([0, 500]);
    const inputs = [-50, 150];
    testCases.push({
      name: "extrapolation",
      config: { domain: [0, 100], range: [0, 500], clamp: false },
      inputs,
      outputs: inputs.map(v => scale(v))
    });
  }

  // Clamped
  {
    const scale = d3.scaleLinear().domain([0, 100]).range([0, 500]).clamp(true);
    const inputs = [-50, 0, 50, 100, 150];
    testCases.push({
      name: "clamped",
      config: { domain: [0, 100], range: [0, 500], clamp: true },
      inputs,
      outputs: inputs.map(v => scale(v))
    });
  }

  // Nice domain
  {
    const scale = d3.scaleLinear().domain([0.123, 0.987]).nice();
    testCases.push({
      name: "nice_domain",
      config: { domain: [0.123, 0.987] },
      nice_domain: scale.domain()
    });
  }

  // Ticks
  {
    const scale = d3.scaleLinear().domain([0, 100]);
    testCases.push({
      name: "ticks_default",
      config: { domain: [0, 100] },
      ticks_count: 10,
      ticks: scale.ticks(10)
    });
  }

  {
    const scale = d3.scaleLinear().domain([0, 100]);
    testCases.push({
      name: "ticks_5",
      config: { domain: [0, 100] },
      ticks_count: 5,
      ticks: scale.ticks(5)
    });
  }

  // Float precision
  {
    const scale = d3.scaleLinear().domain([0.0, 1.0]).range([0.0, 100.0]);
    const inputs = [0.0, 0.1, 0.25, 0.333333, 0.5, 0.666666, 0.75, 0.9, 1.0];
    testCases.push({
      name: "float_precision",
      config: { domain: [0, 1], range: [0, 100] },
      inputs,
      outputs: inputs.map(v => scale(v))
    });
  }

  const golden = createGoldenFile("d3-scale", "scaleLinear", testCases);
  fs.writeFileSync(path.join(__dirname, 'scales', 'linear.json'), JSON.stringify(golden, null, 2));
  console.log('Generated: scales/linear.json');
}

function generateLogScaleTests() {
  const testCases = [];

  // Basic log scale
  {
    const scale = d3.scaleLog().domain([1, 1000]).range([0, 1]);
    const inputs = [1, 10, 100, 1000];
    testCases.push({
      name: "basic_log",
      config: { domain: [1, 1000], range: [0, 1], base: 10 },
      inputs,
      outputs: inputs.map(v => scale(v)),
      invert_inputs: [0, 0.25, 0.5, 0.75, 1],
      invert_outputs: [0, 0.25, 0.5, 0.75, 1].map(v => scale.invert(v))
    });
  }

  // Frequency range (audio)
  {
    const scale = d3.scaleLog().domain([20, 20000]).range([0, 1]);
    const inputs = [20, 100, 1000, 10000, 20000];
    testCases.push({
      name: "frequency_range",
      config: { domain: [20, 20000], range: [0, 1], base: 10 },
      inputs,
      outputs: inputs.map(v => scale(v))
    });
  }

  // Base 2
  {
    const scale = d3.scaleLog().base(2).domain([1, 16]).range([0, 1]);
    const inputs = [1, 2, 4, 8, 16];
    testCases.push({
      name: "base_2",
      config: { domain: [1, 16], range: [0, 1], base: 2 },
      inputs,
      outputs: inputs.map(v => scale(v))
    });
  }

  // Base e (natural log)
  {
    const scale = d3.scaleLog().base(Math.E).domain([1, Math.E * Math.E]).range([0, 1]);
    const inputs = [1, Math.E, Math.E * Math.E];
    testCases.push({
      name: "base_e",
      config: { domain: [1, Math.E * Math.E], range: [0, 1], base: Math.E },
      inputs,
      outputs: inputs.map(v => scale(v))
    });
  }

  // Ticks
  {
    const scale = d3.scaleLog().domain([1, 1000]);
    testCases.push({
      name: "ticks",
      config: { domain: [1, 1000], base: 10 },
      ticks: scale.ticks()
    });
  }

  const golden = createGoldenFile("d3-scale", "scaleLog", testCases);
  fs.writeFileSync(path.join(__dirname, 'scales', 'log.json'), JSON.stringify(golden, null, 2));
  console.log('Generated: scales/log.json');
}

function generatePowScaleTests() {
  const testCases = [];

  // Square (exponent 2)
  {
    const scale = d3.scalePow().exponent(2).domain([0, 10]).range([0, 100]);
    const inputs = [0, 1, 2, 3, 5, 10];
    testCases.push({
      name: "exponent_2",
      config: { domain: [0, 10], range: [0, 100], exponent: 2 },
      inputs,
      outputs: inputs.map(v => scale(v))
    });
  }

  // Sqrt (exponent 0.5)
  {
    const scale = d3.scaleSqrt().domain([0, 100]).range([0, 10]);
    const inputs = [0, 1, 4, 9, 16, 25, 100];
    testCases.push({
      name: "sqrt",
      config: { domain: [0, 100], range: [0, 10], exponent: 0.5 },
      inputs,
      outputs: inputs.map(v => scale(v))
    });
  }

  // Cubic (exponent 3)
  {
    const scale = d3.scalePow().exponent(3).domain([0, 10]).range([0, 1000]);
    const inputs = [0, 1, 2, 5, 10];
    testCases.push({
      name: "exponent_3",
      config: { domain: [0, 10], range: [0, 1000], exponent: 3 },
      inputs,
      outputs: inputs.map(v => scale(v))
    });
  }

  // Invert
  {
    const scale = d3.scalePow().exponent(2).domain([0, 10]).range([0, 100]);
    const inputs = [0, 25, 100];
    testCases.push({
      name: "invert",
      config: { domain: [0, 10], range: [0, 100], exponent: 2 },
      invert_inputs: inputs,
      invert_outputs: inputs.map(v => scale.invert(v))
    });
  }

  const golden = createGoldenFile("d3-scale", "scalePow", testCases);
  fs.writeFileSync(path.join(__dirname, 'scales', 'pow.json'), JSON.stringify(golden, null, 2));
  console.log('Generated: scales/pow.json');
}

function generateQuantizeScaleTests() {
  const testCases = [];

  // Basic quantize
  {
    const scale = d3.scaleQuantize().domain([0, 100]).range(['a', 'b', 'c', 'd']);
    const inputs = [0, 12, 25, 37, 50, 62, 75, 87, 100];
    testCases.push({
      name: "basic",
      config: { domain: [0, 100], range: ['a', 'b', 'c', 'd'] },
      inputs,
      outputs: inputs.map(v => scale(v)),
      thresholds: scale.thresholds()
    });
  }

  // Numeric range
  {
    const scale = d3.scaleQuantize().domain([0, 1]).range([0, 1, 2, 3, 4]);
    const inputs = [0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0];
    testCases.push({
      name: "numeric_range",
      config: { domain: [0, 1], range: [0, 1, 2, 3, 4] },
      inputs,
      outputs: inputs.map(v => scale(v))
    });
  }

  // Invert extent
  {
    const scale = d3.scaleQuantize().domain([0, 100]).range(['low', 'medium', 'high']);
    testCases.push({
      name: "invert_extent",
      config: { domain: [0, 100], range: ['low', 'medium', 'high'] },
      invert_extent: {
        low: scale.invertExtent('low'),
        medium: scale.invertExtent('medium'),
        high: scale.invertExtent('high')
      }
    });
  }

  const golden = createGoldenFile("d3-scale", "scaleQuantize", testCases);
  fs.writeFileSync(path.join(__dirname, 'scales', 'quantize.json'), JSON.stringify(golden, null, 2));
  console.log('Generated: scales/quantize.json');
}

function generateQuantileScaleTests() {
  const testCases = [];

  // Basic quantile
  {
    const data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    const scale = d3.scaleQuantile().domain(data).range(['q1', 'q2', 'q3', 'q4']);
    const inputs = [1, 3, 5, 7, 10];
    testCases.push({
      name: "basic",
      config: { domain: data, range: ['q1', 'q2', 'q3', 'q4'] },
      inputs,
      outputs: inputs.map(v => scale(v)),
      quantiles: scale.quantiles()
    });
  }

  // With outliers
  {
    const data = [1, 1, 1, 2, 2, 3, 3, 5, 10, 100];
    const scale = d3.scaleQuantile().domain(data).range(['low', 'medium', 'high']);
    testCases.push({
      name: "with_outliers",
      config: { domain: data, range: ['low', 'medium', 'high'] },
      quantiles: scale.quantiles()
    });
  }

  const golden = createGoldenFile("d3-scale", "scaleQuantile", testCases);
  fs.writeFileSync(path.join(__dirname, 'scales', 'quantile.json'), JSON.stringify(golden, null, 2));
  console.log('Generated: scales/quantile.json');
}

function generateThresholdScaleTests() {
  const testCases = [];

  // Basic threshold
  {
    const scale = d3.scaleThreshold().domain([0, 1]).range(['negative', 'zero', 'positive']);
    const inputs = [-1, -0.5, 0, 0.5, 1, 2];
    testCases.push({
      name: "basic",
      config: { domain: [0, 1], range: ['negative', 'zero', 'positive'] },
      inputs,
      outputs: inputs.map(v => scale(v))
    });
  }

  // Multiple thresholds
  {
    const scale = d3.scaleThreshold().domain([10, 20, 30, 40]).range(['F', 'D', 'C', 'B', 'A']);
    const inputs = [0, 10, 15, 20, 25, 30, 35, 40, 50];
    testCases.push({
      name: "grades",
      config: { domain: [10, 20, 30, 40], range: ['F', 'D', 'C', 'B', 'A'] },
      inputs,
      outputs: inputs.map(v => scale(v))
    });
  }

  const golden = createGoldenFile("d3-scale", "scaleThreshold", testCases);
  fs.writeFileSync(path.join(__dirname, 'scales', 'threshold.json'), JSON.stringify(golden, null, 2));
  console.log('Generated: scales/threshold.json');
}

// ============================================================================
// INTERPOLATE GENERATORS
// ============================================================================

function generateInterpolateNumberTests() {
  const testCases = [];

  // Basic number interpolation
  {
    const interp = d3.interpolateNumber(0, 100);
    const ts = [0, 0.25, 0.5, 0.75, 1];
    testCases.push({
      name: "basic",
      config: { a: 0, b: 100 },
      inputs: ts,
      outputs: ts.map(t => interp(t))
    });
  }

  // Negative range
  {
    const interp = d3.interpolateNumber(-100, 100);
    const ts = [0, 0.25, 0.5, 0.75, 1];
    testCases.push({
      name: "negative_range",
      config: { a: -100, b: 100 },
      inputs: ts,
      outputs: ts.map(t => interp(t))
    });
  }

  // Extrapolation
  {
    const interp = d3.interpolateNumber(0, 100);
    const ts = [-0.5, 0, 0.5, 1, 1.5];
    testCases.push({
      name: "extrapolation",
      config: { a: 0, b: 100 },
      inputs: ts,
      outputs: ts.map(t => interp(t))
    });
  }

  // Round
  {
    const interp = d3.interpolateRound(0, 100);
    const ts = [0, 0.25, 0.5, 0.75, 1];
    testCases.push({
      name: "round",
      config: { a: 0, b: 100, round: true },
      inputs: ts,
      outputs: ts.map(t => interp(t))
    });
  }

  const golden = createGoldenFile("d3-interpolate", "interpolateNumber", testCases);
  fs.writeFileSync(path.join(__dirname, 'interpolate', 'number.json'), JSON.stringify(golden, null, 2));
  console.log('Generated: interpolate/number.json');
}

function generateInterpolateColorTests() {
  const testCases = [];

  // RGB interpolation
  {
    const interp = d3.interpolateRgb('red', 'blue');
    const ts = [0, 0.25, 0.5, 0.75, 1];
    testCases.push({
      name: "rgb_red_blue",
      config: { a: 'red', b: 'blue', space: 'rgb' },
      inputs: ts,
      outputs: ts.map(t => interp(t))
    });
  }

  // HSL interpolation
  {
    const interp = d3.interpolateHsl('red', 'blue');
    const ts = [0, 0.25, 0.5, 0.75, 1];
    testCases.push({
      name: "hsl_red_blue",
      config: { a: 'red', b: 'blue', space: 'hsl' },
      inputs: ts,
      outputs: ts.map(t => interp(t))
    });
  }

  // HSL long
  {
    const interp = d3.interpolateHslLong('red', 'blue');
    const ts = [0, 0.25, 0.5, 0.75, 1];
    testCases.push({
      name: "hsl_long_red_blue",
      config: { a: 'red', b: 'blue', space: 'hsl-long' },
      inputs: ts,
      outputs: ts.map(t => interp(t))
    });
  }

  // Lab interpolation
  {
    const interp = d3.interpolateLab('red', 'blue');
    const ts = [0, 0.25, 0.5, 0.75, 1];
    testCases.push({
      name: "lab_red_blue",
      config: { a: 'red', b: 'blue', space: 'lab' },
      inputs: ts,
      outputs: ts.map(t => interp(t))
    });
  }

  // HCL interpolation
  {
    const interp = d3.interpolateHcl('red', 'blue');
    const ts = [0, 0.25, 0.5, 0.75, 1];
    testCases.push({
      name: "hcl_red_blue",
      config: { a: 'red', b: 'blue', space: 'hcl' },
      inputs: ts,
      outputs: ts.map(t => interp(t))
    });
  }

  // Cubehelix interpolation
  {
    const interp = d3.interpolateCubehelix('red', 'blue');
    const ts = [0, 0.25, 0.5, 0.75, 1];
    testCases.push({
      name: "cubehelix_red_blue",
      config: { a: 'red', b: 'blue', space: 'cubehelix' },
      inputs: ts,
      outputs: ts.map(t => interp(t))
    });
  }

  // Hex colors
  {
    const interp = d3.interpolateRgb('#ff0000', '#0000ff');
    const ts = [0, 0.5, 1];
    testCases.push({
      name: "rgb_hex",
      config: { a: '#ff0000', b: '#0000ff', space: 'rgb' },
      inputs: ts,
      outputs: ts.map(t => interp(t))
    });
  }

  const golden = createGoldenFile("d3-interpolate", "interpolateColor", testCases);
  fs.writeFileSync(path.join(__dirname, 'interpolate', 'color.json'), JSON.stringify(golden, null, 2));
  console.log('Generated: interpolate/color.json');
}

// ============================================================================
// ARRAY GENERATORS
// ============================================================================

function generateArrayStatisticsTests() {
  const testCases = [];

  // min/max/extent
  {
    const data = [3, 1, 4, 1, 5, 9, 2, 6, 5, 3, 5];
    testCases.push({
      name: "min_max_extent",
      data,
      min: d3.min(data),
      max: d3.max(data),
      extent: d3.extent(data)
    });
  }

  // sum/mean/median
  {
    const data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    testCases.push({
      name: "sum_mean_median",
      data,
      sum: d3.sum(data),
      mean: d3.mean(data),
      median: d3.median(data)
    });
  }

  // variance/deviation
  {
    const data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    testCases.push({
      name: "variance_deviation",
      data,
      variance: d3.variance(data),
      deviation: d3.deviation(data)
    });
  }

  // quantile
  {
    const data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    testCases.push({
      name: "quantile",
      data,
      q0: d3.quantile(data, 0),
      q25: d3.quantile(data, 0.25),
      q50: d3.quantile(data, 0.5),
      q75: d3.quantile(data, 0.75),
      q100: d3.quantile(data, 1)
    });
  }

  // cumsum
  {
    const data = [1, 2, 3, 4, 5];
    testCases.push({
      name: "cumsum",
      data,
      cumsum: Array.from(d3.cumsum(data))
    });
  }

  // With accessor
  {
    const data = [{ value: 10 }, { value: 20 }, { value: 30 }];
    testCases.push({
      name: "with_accessor",
      data: data.map(d => d.value),
      min: d3.min(data, d => d.value),
      max: d3.max(data, d => d.value),
      mean: d3.mean(data, d => d.value)
    });
  }

  // Empty array
  {
    testCases.push({
      name: "empty_array",
      data: [],
      min: d3.min([]),
      max: d3.max([]),
      mean: d3.mean([])
    });
  }

  const golden = createGoldenFile("d3-array", "statistics", testCases);
  fs.writeFileSync(path.join(__dirname, 'array', 'statistics.json'), JSON.stringify(golden, null, 2));
  console.log('Generated: array/statistics.json');
}

function generateArrayBisectTests() {
  const testCases = [];

  // Basic bisect
  {
    const arr = [1, 2, 3, 4, 5];
    testCases.push({
      name: "basic",
      array: arr,
      bisect_left: {
        0: d3.bisectLeft(arr, 0),
        1: d3.bisectLeft(arr, 1),
        2.5: d3.bisectLeft(arr, 2.5),
        5: d3.bisectLeft(arr, 5),
        6: d3.bisectLeft(arr, 6)
      },
      bisect_right: {
        0: d3.bisectRight(arr, 0),
        1: d3.bisectRight(arr, 1),
        2.5: d3.bisectRight(arr, 2.5),
        5: d3.bisectRight(arr, 5),
        6: d3.bisectRight(arr, 6)
      }
    });
  }

  // With duplicates
  {
    const arr = [1, 2, 2, 2, 3, 4, 5];
    testCases.push({
      name: "with_duplicates",
      array: arr,
      bisect_left_2: d3.bisectLeft(arr, 2),
      bisect_right_2: d3.bisectRight(arr, 2)
    });
  }

  // Float array
  {
    const arr = [0.1, 0.2, 0.3, 0.4, 0.5];
    testCases.push({
      name: "floats",
      array: arr,
      bisect_left_025: d3.bisectLeft(arr, 0.25),
      bisect_right_025: d3.bisectRight(arr, 0.25)
    });
  }

  const golden = createGoldenFile("d3-array", "bisect", testCases);
  fs.writeFileSync(path.join(__dirname, 'array', 'bisect.json'), JSON.stringify(golden, null, 2));
  console.log('Generated: array/bisect.json');
}

function generateArrayBinTests() {
  const testCases = [];

  // Basic binning
  {
    const data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    const bins = d3.bin().thresholds(5)(data);
    testCases.push({
      name: "basic",
      data,
      threshold_count: 5,
      bins: bins.map(b => ({
        x0: b.x0,
        x1: b.x1,
        length: b.length,
        values: Array.from(b)
      }))
    });
  }

  // Custom domain
  {
    const data = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
    const bins = d3.bin().domain([0, 10]).thresholds(10)(data);
    testCases.push({
      name: "custom_domain",
      data,
      domain: [0, 10],
      threshold_count: 10,
      bins: bins.map(b => ({
        x0: b.x0,
        x1: b.x1,
        length: b.length
      }))
    });
  }

  // Sturges thresholds
  {
    const data = Array.from({ length: 100 }, (_, i) => i);
    const bins = d3.bin().thresholds(d3.thresholdSturges)(data);
    testCases.push({
      name: "sturges",
      data_length: 100,
      bin_count: bins.length
    });
  }

  const golden = createGoldenFile("d3-array", "bin", testCases);
  fs.writeFileSync(path.join(__dirname, 'array', 'bin.json'), JSON.stringify(golden, null, 2));
  console.log('Generated: array/bin.json');
}

function generateArrayTicksTests() {
  const testCases = [];

  // Basic ticks
  {
    testCases.push({
      name: "basic_0_100",
      start: 0,
      stop: 100,
      count: 10,
      ticks: d3.ticks(0, 100, 10)
    });
  }

  // Negative range
  {
    testCases.push({
      name: "negative_range",
      start: -50,
      stop: 50,
      count: 10,
      ticks: d3.ticks(-50, 50, 10)
    });
  }

  // Small range
  {
    testCases.push({
      name: "small_range",
      start: 0,
      stop: 1,
      count: 5,
      ticks: d3.ticks(0, 1, 5)
    });
  }

  // Large range
  {
    testCases.push({
      name: "large_range",
      start: 0,
      stop: 1000000,
      count: 5,
      ticks: d3.ticks(0, 1000000, 5)
    });
  }

  // Nice
  {
    testCases.push({
      name: "nice",
      start: 0.123,
      stop: 0.987,
      count: 5,
      ticks: d3.ticks(0.123, 0.987, 5),
      nice: d3.nice(0.123, 0.987, 5)
    });
  }

  // tickStep
  {
    testCases.push({
      name: "tick_step",
      start: 0,
      stop: 100,
      count: 10,
      tick_step: d3.tickStep(0, 100, 10)
    });
  }

  const golden = createGoldenFile("d3-array", "ticks", testCases);
  fs.writeFileSync(path.join(__dirname, 'array', 'ticks.json'), JSON.stringify(golden, null, 2));
  console.log('Generated: array/ticks.json');
}

// ============================================================================
// COLOR GENERATORS
// ============================================================================

function generateColorTests() {
  const testCases = [];

  // RGB parsing
  {
    const colors = ['red', 'green', 'blue', '#ff0000', '#00ff00', '#0000ff', 'rgb(255, 128, 0)', 'rgba(100, 150, 200, 0.5)'];
    testCases.push({
      name: "parsing",
      colors: colors.map(c => {
        const color = d3.color(c);
        return {
          input: c,
          r: color.r,
          g: color.g,
          b: color.b,
          opacity: color.opacity,
          hex: color.formatHex(),
          rgb: color.formatRgb()
        };
      })
    });
  }

  // HSL conversion
  {
    const colors = ['red', 'green', 'blue', 'yellow', 'cyan', 'magenta'];
    testCases.push({
      name: "hsl_conversion",
      colors: colors.map(c => {
        const rgb = d3.color(c);
        const hsl = d3.hsl(c);
        return {
          input: c,
          h: hsl.h,
          s: hsl.s,
          l: hsl.l
        };
      })
    });
  }

  // Brighter/darker
  {
    const base = d3.color('steelblue');
    testCases.push({
      name: "brighter_darker",
      base: base.formatHex(),
      brighter_1: base.brighter(1).formatHex(),
      brighter_2: base.brighter(2).formatHex(),
      darker_1: base.darker(1).formatHex(),
      darker_2: base.darker(2).formatHex()
    });
  }

  // Color schemes
  {
    testCases.push({
      name: "schemes",
      category10: d3.schemeCategory10,
      tableau10: d3.schemeTableau10,
      paired: d3.schemePaired,
      set1: d3.schemeSet1
    });
  }

  const golden = createGoldenFile("d3-color", "color", testCases);
  fs.writeFileSync(path.join(__dirname, 'color', 'color.json'), JSON.stringify(golden, null, 2));
  console.log('Generated: color/color.json');
}

// ============================================================================
// SHAPE GENERATORS
// ============================================================================

function generateArcTests() {
  const testCases = [];

  // Basic arc
  {
    const arc = d3.arc()
      .innerRadius(0)
      .outerRadius(100)
      .startAngle(0)
      .endAngle(Math.PI / 2);

    testCases.push({
      name: "basic_quarter",
      config: {
        innerRadius: 0,
        outerRadius: 100,
        startAngle: 0,
        endAngle: Math.PI / 2
      },
      path: arc(),
      centroid: arc.centroid()
    });
  }

  // Donut arc
  {
    const arc = d3.arc()
      .innerRadius(50)
      .outerRadius(100)
      .startAngle(0)
      .endAngle(Math.PI);

    testCases.push({
      name: "donut_half",
      config: {
        innerRadius: 50,
        outerRadius: 100,
        startAngle: 0,
        endAngle: Math.PI
      },
      path: arc(),
      centroid: arc.centroid()
    });
  }

  // With corner radius
  {
    const arc = d3.arc()
      .innerRadius(50)
      .outerRadius(100)
      .cornerRadius(10)
      .startAngle(0)
      .endAngle(Math.PI / 2);

    testCases.push({
      name: "corner_radius",
      config: {
        innerRadius: 50,
        outerRadius: 100,
        cornerRadius: 10,
        startAngle: 0,
        endAngle: Math.PI / 2
      },
      path: arc()
    });
  }

  const golden = createGoldenFile("d3-shape", "arc", testCases);
  fs.writeFileSync(path.join(__dirname, 'shape', 'arc.json'), JSON.stringify(golden, null, 2));
  console.log('Generated: shape/arc.json');
}

function generatePieTests() {
  const testCases = [];

  // Basic pie
  {
    const data = [1, 2, 3, 4];
    const pie = d3.pie();
    const arcs = pie(data);

    testCases.push({
      name: "basic",
      data,
      arcs: arcs.map(a => ({
        value: a.value,
        startAngle: a.startAngle,
        endAngle: a.endAngle,
        padAngle: a.padAngle,
        index: a.index
      }))
    });
  }

  // With padding
  {
    const data = [1, 2, 3, 4];
    const pie = d3.pie().padAngle(0.05);
    const arcs = pie(data);

    testCases.push({
      name: "with_padding",
      data,
      padAngle: 0.05,
      arcs: arcs.map(a => ({
        value: a.value,
        startAngle: a.startAngle,
        endAngle: a.endAngle,
        padAngle: a.padAngle
      }))
    });
  }

  // Custom start/end angles
  {
    const data = [1, 2, 3];
    const pie = d3.pie()
      .startAngle(-Math.PI / 2)
      .endAngle(Math.PI / 2);
    const arcs = pie(data);

    testCases.push({
      name: "half_pie",
      data,
      startAngle: -Math.PI / 2,
      endAngle: Math.PI / 2,
      arcs: arcs.map(a => ({
        value: a.value,
        startAngle: a.startAngle,
        endAngle: a.endAngle
      }))
    });
  }

  // Sorted
  {
    const data = [3, 1, 4, 1, 5];
    const pie = d3.pie().sort((a, b) => b - a);
    const arcs = pie(data);

    testCases.push({
      name: "sorted_descending",
      data,
      arcs: arcs.map(a => ({
        value: a.value,
        index: a.index,
        startAngle: a.startAngle,
        endAngle: a.endAngle
      }))
    });
  }

  const golden = createGoldenFile("d3-shape", "pie", testCases);
  fs.writeFileSync(path.join(__dirname, 'shape', 'pie.json'), JSON.stringify(golden, null, 2));
  console.log('Generated: shape/pie.json');
}

function generateLineTests() {
  const testCases = [];

  // Linear curve
  {
    const data = [[0, 0], [10, 20], [20, 10], [30, 30], [40, 15]];
    const line = d3.line();
    testCases.push({
      name: "linear",
      data,
      curve: "linear",
      path: line(data)
    });
  }

  // Step curve
  {
    const data = [[0, 0], [10, 20], [20, 10], [30, 30]];
    const line = d3.line().curve(d3.curveStep);
    testCases.push({
      name: "step",
      data,
      curve: "step",
      path: line(data)
    });
  }

  // Basis curve
  {
    const data = [[0, 0], [10, 20], [20, 10], [30, 30], [40, 15]];
    const line = d3.line().curve(d3.curveBasis);
    testCases.push({
      name: "basis",
      data,
      curve: "basis",
      path: line(data)
    });
  }

  // Cardinal curve
  {
    const data = [[0, 0], [10, 20], [20, 10], [30, 30], [40, 15]];
    const line = d3.line().curve(d3.curveCardinal);
    testCases.push({
      name: "cardinal",
      data,
      curve: "cardinal",
      path: line(data)
    });
  }

  // Catmull-Rom curve
  {
    const data = [[0, 0], [10, 20], [20, 10], [30, 30], [40, 15]];
    const line = d3.line().curve(d3.curveCatmullRom);
    testCases.push({
      name: "catmull_rom",
      data,
      curve: "catmullRom",
      path: line(data)
    });
  }

  // Monotone X
  {
    const data = [[0, 0], [10, 20], [20, 10], [30, 30], [40, 15]];
    const line = d3.line().curve(d3.curveMonotoneX);
    testCases.push({
      name: "monotone_x",
      data,
      curve: "monotoneX",
      path: line(data)
    });
  }

  // Natural
  {
    const data = [[0, 0], [10, 20], [20, 10], [30, 30], [40, 15]];
    const line = d3.line().curve(d3.curveNatural);
    testCases.push({
      name: "natural",
      data,
      curve: "natural",
      path: line(data)
    });
  }

  const golden = createGoldenFile("d3-shape", "line", testCases);
  fs.writeFileSync(path.join(__dirname, 'shape', 'line.json'), JSON.stringify(golden, null, 2));
  console.log('Generated: shape/line.json');
}

function generateSymbolTests() {
  const testCases = [];

  const symbolTypes = [
    { name: 'circle', type: d3.symbolCircle },
    { name: 'cross', type: d3.symbolCross },
    { name: 'diamond', type: d3.symbolDiamond },
    { name: 'square', type: d3.symbolSquare },
    { name: 'star', type: d3.symbolStar },
    { name: 'triangle', type: d3.symbolTriangle },
    { name: 'wye', type: d3.symbolWye }
  ];

  for (const { name, type } of symbolTypes) {
    const symbol = d3.symbol().type(type).size(64);
    testCases.push({
      name,
      size: 64,
      path: symbol()
    });
  }

  // Different sizes
  {
    const sizes = [16, 64, 256];
    for (const size of sizes) {
      const symbol = d3.symbol().type(d3.symbolCircle).size(size);
      testCases.push({
        name: `circle_size_${size}`,
        size,
        path: symbol()
      });
    }
  }

  const golden = createGoldenFile("d3-shape", "symbol", testCases);
  fs.writeFileSync(path.join(__dirname, 'shape', 'symbol.json'), JSON.stringify(golden, null, 2));
  console.log('Generated: shape/symbol.json');
}

function generateStackTests() {
  const testCases = [];

  // Basic stack
  {
    const data = [
      { month: 'Jan', apples: 10, oranges: 20, bananas: 15 },
      { month: 'Feb', apples: 15, oranges: 25, bananas: 10 },
      { month: 'Mar', apples: 20, oranges: 30, bananas: 20 }
    ];

    const stack = d3.stack().keys(['apples', 'oranges', 'bananas']);
    const stacked = stack(data);

    testCases.push({
      name: "basic",
      data,
      keys: ['apples', 'oranges', 'bananas'],
      result: stacked.map(series => ({
        key: series.key,
        values: series.map(d => [d[0], d[1]])
      }))
    });
  }

  // With offset expand (normalize to 100%)
  {
    const data = [
      { a: 10, b: 20, c: 30 },
      { a: 20, b: 30, c: 40 },
      { a: 30, b: 40, c: 50 }
    ];

    const stack = d3.stack()
      .keys(['a', 'b', 'c'])
      .offset(d3.stackOffsetExpand);
    const stacked = stack(data);

    testCases.push({
      name: "offset_expand",
      data,
      keys: ['a', 'b', 'c'],
      offset: 'expand',
      result: stacked.map(series => ({
        key: series.key,
        values: series.map(d => [d[0], d[1]])
      }))
    });
  }

  // Diverging offset
  {
    const data = [
      { pos: 10, neg: -5 },
      { pos: 20, neg: -10 },
      { pos: 15, neg: -8 }
    ];

    const stack = d3.stack()
      .keys(['pos', 'neg'])
      .offset(d3.stackOffsetDiverging);
    const stacked = stack(data);

    testCases.push({
      name: "offset_diverging",
      data,
      keys: ['pos', 'neg'],
      offset: 'diverging',
      result: stacked.map(series => ({
        key: series.key,
        values: series.map(d => [d[0], d[1]])
      }))
    });
  }

  const golden = createGoldenFile("d3-shape", "stack", testCases);
  fs.writeFileSync(path.join(__dirname, 'shape', 'stack.json'), JSON.stringify(golden, null, 2));
  console.log('Generated: shape/stack.json');
}

// ============================================================================
// QUADTREE GENERATORS
// ============================================================================

function generateQuadtreeTests() {
  const testCases = [];

  // Basic quadtree with add
  {
    const tree = d3.quadtree();
    tree.add([0, 0]);
    tree.add([1, 0]);
    tree.add([0, 1]);
    tree.add([1, 1]);
    tree.add([0.5, 0.5]);

    testCases.push({
      name: "basic_add",
      points: [[0, 0], [1, 0], [0, 1], [1, 1], [0.5, 0.5]],
      size: tree.size(),
      extent: tree.extent()
    });
  }

  // Find nearest neighbor
  {
    const tree = d3.quadtree()
      .addAll([[0, 0], [1, 0], [0, 1], [1, 1], [0.5, 0.5]]);

    const queries = [
      { x: 0.3, y: 0.3 },
      { x: 0.9, y: 0.1 },
      { x: 0.5, y: 0.5 },
      { x: 2, y: 2 }
    ];

    testCases.push({
      name: "find",
      points: [[0, 0], [1, 0], [0, 1], [1, 1], [0.5, 0.5]],
      queries: queries.map(q => ({
        x: q.x,
        y: q.y,
        result: tree.find(q.x, q.y)
      }))
    });
  }

  // Find with radius
  {
    const tree = d3.quadtree()
      .addAll([[0, 0], [1, 0], [0, 1], [1, 1], [0.5, 0.5]]);

    testCases.push({
      name: "find_with_radius",
      points: [[0, 0], [1, 0], [0, 1], [1, 1], [0.5, 0.5]],
      queries: [
        { x: 0.5, y: 0.5, radius: 0.1, result: tree.find(0.5, 0.5, 0.1) },
        { x: 0.5, y: 0.5, radius: 0.5, result: tree.find(0.5, 0.5, 0.5) },
        { x: 0.5, y: 0.5, radius: 1.0, result: tree.find(0.5, 0.5, 1.0) },
        { x: 10, y: 10, radius: 0.5, result: tree.find(10, 10, 0.5) }
      ]
    });
  }

  // Remove points
  // Note: D3.js remove() requires the exact same object reference that was added.
  // Our Rust implementation removes by coordinates, which is more practical.
  // This test uses find() to get the reference first, which is what users should do in D3.
  {
    const tree = d3.quadtree()
      .addAll([[0, 0], [1, 0], [0, 1], [1, 1]]);

    const sizeBefore = tree.size();
    // Find the point first to get the reference
    const pointToRemove = tree.find(1, 0, 0.001);
    if (pointToRemove) {
      tree.remove(pointToRemove);
    }
    const sizeAfter = tree.size();

    testCases.push({
      name: "remove",
      points: [[0, 0], [1, 0], [0, 1], [1, 1]],
      remove: [1, 0],
      size_before: sizeBefore,
      size_after: sizeAfter
    });
  }

  // Extent
  {
    const tree = d3.quadtree()
      .addAll([[0, 0], [3, 2], [-1, 5], [7, -3]]);

    testCases.push({
      name: "extent",
      points: [[0, 0], [3, 2], [-1, 5], [7, -3]],
      extent: tree.extent(),
      size: tree.size()
    });
  }

  // Visit traversal
  {
    const tree = d3.quadtree()
      .addAll([[0, 0], [1, 0], [0, 1], [1, 1]]);

    const visited = [];
    tree.visit((node, x0, y0, x1, y1) => {
      visited.push({
        x0, y0, x1, y1,
        is_leaf: !node.length
      });
      return false; // continue visiting
    });

    testCases.push({
      name: "visit",
      points: [[0, 0], [1, 0], [0, 1], [1, 1]],
      visited_count: visited.length,
      leaf_count: visited.filter(v => v.is_leaf).length
    });
  }

  // Data extraction
  {
    const points = [[0, 0], [1, 0], [0, 1], [1, 1], [0.5, 0.5]];
    const tree = d3.quadtree().addAll(points);

    testCases.push({
      name: "data",
      points,
      data: tree.data()
    });
  }

  // Coincident points
  {
    const tree = d3.quadtree();
    tree.add([5, 5]);
    tree.add([5, 5]);
    tree.add([5, 5]);

    testCases.push({
      name: "coincident",
      points: [[5, 5], [5, 5], [5, 5]],
      size: tree.size(),
      data: tree.data()
    });
  }

  // Large dataset for performance reference
  {
    const points = [];
    for (let i = 0; i < 100; i++) {
      // Use golden ratio for even distribution
      const x = (i * 0.618033988749895) % 1 * 100;
      const y = (i * 0.381966011250105) % 1 * 100;
      points.push([x, y]);
    }

    const tree = d3.quadtree().addAll(points);

    testCases.push({
      name: "large_dataset",
      point_count: 100,
      size: tree.size(),
      extent: tree.extent(),
      // Find some specific points
      find_50_50: tree.find(50, 50),
      find_0_0: tree.find(0, 0),
      find_100_100: tree.find(100, 100)
    });
  }

  const golden = createGoldenFile("d3-quadtree", "quadtree", testCases);
  fs.writeFileSync(path.join(__dirname, 'quadtree', 'quadtree.json'), JSON.stringify(golden, null, 2));
  console.log('Generated: quadtree/quadtree.json');
}

// ============================================================================
// MAIN
// ============================================================================

function generateAllScales() {
  generateLinearScaleTests();
  generateLogScaleTests();
  generatePowScaleTests();
  generateQuantizeScaleTests();
  generateQuantileScaleTests();
  generateThresholdScaleTests();
}

function generateAllInterpolate() {
  generateInterpolateNumberTests();
  generateInterpolateColorTests();
}

function generateAllArray() {
  generateArrayStatisticsTests();
  generateArrayBisectTests();
  generateArrayBinTests();
  generateArrayTicksTests();
}

function generateAllColor() {
  generateColorTests();
}

function generateAllShape() {
  generateArcTests();
  generatePieTests();
  generateLineTests();
  generateSymbolTests();
  generateStackTests();
}

function generateAllQuadtree() {
  generateQuadtreeTests();
}

function generateAll() {
  console.log(`Generating golden files using D3.js v${d3.version}...\n`);
  generateAllScales();
  generateAllInterpolate();
  generateAllArray();
  generateAllColor();
  generateAllShape();
  generateAllQuadtree();
  console.log('\nDone!');
}

// Parse command line arguments
const args = process.argv.slice(2);
if (args.length === 0) {
  generateAll();
} else {
  for (const arg of args) {
    switch (arg) {
      case 'scales':
        generateAllScales();
        break;
      case 'interpolate':
        generateAllInterpolate();
        break;
      case 'array':
        generateAllArray();
        break;
      case 'color':
        generateAllColor();
        break;
      case 'shape':
        generateAllShape();
        break;
      case 'quadtree':
        generateAllQuadtree();
        break;
      default:
        console.error(`Unknown module: ${arg}`);
        process.exit(1);
    }
  }
}
