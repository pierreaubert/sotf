/**
 * Golden file generator for complete D3.js Observable examples
 *
 * Unlike generate.js (unit-level) and generate_examples.js (feature-level),
 * this generator reproduces COMPLETE Observable notebook examples end-to-end.
 * Each test captures the full visualization pipeline: data -> scales -> layout -> color -> axes.
 *
 * Sources:
 *   - https://observablehq.com/@d3/hexbin
 *   - https://observablehq.com/@d3/streamgraph
 *   - https://observablehq.com/@d3/orthographic-to-equirectangular
 *   - https://observablehq.com/@d3/zoomable-circle-packing
 *   - https://observablehq.com/@d3/sunburst
 *   - https://observablehq.com/@d3/versor-dragging
 *   - https://observablehq.com/@d3/box-plot
 *   - https://observablehq.com/@d3/force-directed-graph
 *   - https://observablehq.com/@d3/sankey
 *   - https://observablehq.com/@d3/chord-diagram
 *   - https://observablehq.com/@d3/stacked-bar-chart
 *   - https://observablehq.com/@d3/stacked-area-chart
 *   - https://observablehq.com/@d3/line-chart
 *   - https://observablehq.com/@d3/pie-chart
 *   - https://observablehq.com/@d3/donut-chart
 *
 * Usage:
 *   node generate_observable_examples.js              # Generate all
 *   node generate_observable_examples.js hexbin        # Generate only hexbin
 *   node generate_observable_examples.js pie donut     # Generate multiple
 */

const d3 = require('d3');
const fs = require('fs');
const path = require('path');

const TOLERANCE = 1e-6;
const OUTPUT_DIR = path.join(__dirname, 'observable');

function createGoldenFile(source_url, module, func, testCases) {
  return {
    source: source_url,
    module,
    function: func,
    d3_version: d3.version,
    tolerance: TOLERANCE,
    generated_at: new Date().toISOString(),
    test_cases: testCases
  };
}

function round(v, decimals = 6) {
  if (v === null || v === undefined || isNaN(v)) return v;
  const f = Math.pow(10, decimals);
  return Math.round(v * f) / f;
}

function writeGolden(filename, golden) {
  fs.mkdirSync(OUTPUT_DIR, { recursive: true });
  fs.writeFileSync(path.join(OUTPUT_DIR, filename), JSON.stringify(golden, null, 2));
  console.log(`Generated: observable/${filename}`);
}

// ============================================================================
// HEXBIN — https://observablehq.com/@d3/hexbin
// ============================================================================

function generateHexbinObservable() {
  const { hexbin } = require('d3-hexbin');
  const testCases = [];

  const data = [];
  for (let i = 0; i < 200; i++) {
    const t = i / 199;
    const carat = 0.2 + Math.pow(t, 2) * 4.8;
    const basePrice = 326 * Math.pow(carat / 0.2, 1.8);
    const noise = 1.0 + 0.3 * Math.sin(i * 7.3) + 0.15 * Math.cos(i * 13.1);
    const price = Math.min(18823, Math.max(326, basePrice * noise));
    data.push({ carat: round(carat, 4), price: round(price, 2) });
  }

  const width = 928, height = 928;
  const marginTop = 20, marginRight = 20, marginBottom = 30, marginLeft = 40;
  const radius = 8;

  const x = d3.scaleLog().domain(d3.extent(data, d => d.carat)).range([marginLeft, width - marginRight]);
  const y = d3.scaleLog().domain(d3.extent(data, d => d.price)).range([height - marginBottom, marginTop]);

  const hex = hexbin()
    .x(d => x(d.carat)).y(d => y(d.price))
    .radius(radius * width / 928)
    .extent([[marginLeft, marginTop], [width - marginRight, height - marginBottom]]);

  const bins = hex(data);
  const maxCount = d3.max(bins, d => d.length);
  const color = d3.scaleSequential(d3.interpolateBuPu).domain([0, maxCount / 2]);
  const xTicks = x.ticks(width / 80);
  const yTicks = y.ticks();

  testCases.push({
    name: "hexbin_diamonds",
    description: "Complete hexbin chart from Observable",
    layout: { width, height, marginTop, marginRight, marginBottom, marginLeft, radius },
    data, data_count: data.length,
    x_scale: {
      type: "log", domain: x.domain(), range: x.range(),
      samples: data.filter((_, i) => i % 40 === 0).map(d => ({ input: d.carat, output: round(x(d.carat)) }))
    },
    y_scale: {
      type: "log", domain: y.domain(), range: y.range(),
      samples: data.filter((_, i) => i % 40 === 0).map(d => ({ input: d.price, output: round(y(d.price)) }))
    },
    hexbin_config: {
      radius: radius * width / 928,
      extent: [[marginLeft, marginTop], [width - marginRight, height - marginBottom]],
      hexagon_path: hex.hexagon()
    },
    bins: bins.map(b => ({ x: round(b.x), y: round(b.y), count: b.length })).sort((a, b) => a.x - b.x || a.y - b.y),
    bin_count: bins.length, max_bin_count: maxCount,
    color_scale: {
      type: "sequential", interpolator: "BuPu", domain: color.domain(),
      samples: Array.from({ length: Math.min(maxCount, 20) }, (_, i) => ({ count: i + 1, color: color(i + 1) }))
    },
    x_axis: {
      orientation: "bottom",
      ticks: xTicks.map(t => ({ value: t, position: round(x(t)) })),
      tick_format: xTicks.map(t => d3.format("")(t))
    },
    y_axis: {
      orientation: "left",
      ticks: yTicks.map(t => ({ value: t, position: round(y(t)) })),
      tick_format: yTicks.map(t => d3.format(".1s")(t))
    },
    bin_colors: bins.map(b => ({ x: round(b.x), y: round(b.y), count: b.length, fill: color(b.length) }))
      .sort((a, b) => a.x - b.x || a.y - b.y)
  });

  writeGolden('hexbin.json', createGoldenFile("https://observablehq.com/@d3/hexbin", "d3-hexbin", "hexbin_observable", testCases));
}

// ============================================================================
// STREAMGRAPH — https://observablehq.com/@d3/streamgraph
// ============================================================================

function generateStreamgraphObservable() {
  const testCases = [];

  // Deterministic multi-series time data (5 categories, 20 time steps)
  const categories = ["alpha", "beta", "gamma", "delta", "epsilon"];
  const n = 20;
  const rawData = [];
  for (let t = 0; t < n; t++) {
    for (let c = 0; c < categories.length; c++) {
      const base = 10 + 5 * c;
      const val = base + 8 * Math.sin(t * 0.5 + c * 1.3) + 3 * Math.cos(t * 0.3 + c * 0.7);
      rawData.push({ time: t, category: categories[c], value: round(Math.max(0, val), 2) });
    }
  }

  const width = 928, height = 500;
  const marginTop = 20, marginRight = 20, marginBottom = 30, marginLeft = 20;

  // Build stack data: array of maps keyed by category
  const grouped = d3.rollup(rawData, v => v[0].value, d => d.time, d => d.category);
  const times = Array.from(grouped.keys()).sort((a, b) => a - b);

  // Build matrix for stack: rows=times, columns=categories
  const stackData = times.map(t => {
    const row = {};
    for (const cat of categories) {
      row[cat] = grouped.get(t)?.get(cat) ?? 0;
    }
    return row;
  });

  const stack = d3.stack()
    .keys(categories)
    .order(d3.stackOrderInsideOut)
    .offset(d3.stackOffsetWiggle);

  const series = stack(stackData);

  const xScale = d3.scaleLinear()
    .domain([0, n - 1])
    .range([marginLeft, width - marginRight]);

  const yExtent = d3.extent(series.flat(2));
  const yScale = d3.scaleLinear()
    .domain(yExtent)
    .range([height - marginBottom, marginTop]);

  const area = d3.area()
    .x((d, i) => xScale(i))
    .y0(d => yScale(d[0]))
    .y1(d => yScale(d[1]));

  testCases.push({
    name: "streamgraph",
    description: "Streamgraph with stackOffsetWiggle and stackOrderInsideOut",
    layout: { width, height, marginTop, marginRight, marginBottom, marginLeft },
    categories,
    time_steps: n,
    raw_data: rawData,
    stack_config: { order: "insideOut", offset: "wiggle" },
    // Stack output: for each series, the stacked [y0, y1] values
    series: series.map(s => ({
      key: s.key,
      index: s.index,
      values: s.map(d => [round(d[0]), round(d[1])])
    })),
    x_scale: {
      type: "linear", domain: xScale.domain(), range: xScale.range(),
      samples: [0, 5, 10, 15, 19].map(t => ({ input: t, output: round(xScale(t)) }))
    },
    y_scale: {
      type: "linear", domain: yScale.domain().map(v => round(v)), range: yScale.range()
    },
    // Area paths for each series
    area_paths: series.map(s => ({
      key: s.key,
      path: area(s)
    }))
  });

  writeGolden('streamgraph.json', createGoldenFile(
    "https://observablehq.com/@d3/streamgraph", "d3-shape", "streamgraph_observable", testCases));
}

// ============================================================================
// ORTHOGRAPHIC TO EQUIRECTANGULAR — https://observablehq.com/@d3/orthographic-to-equirectangular
// ============================================================================

function generateOrthoToEquirectObservable() {
  const testCases = [];

  // Test projection of sample points through both projections
  const points = [
    [-122.4194, 37.7749],  // San Francisco
    [2.3522, 48.8566],     // Paris
    [139.6917, 35.6895],   // Tokyo
    [-43.1729, -22.9068],  // Rio de Janeiro
    [28.9784, 41.0082],    // Istanbul
    [0, 0],                // Null Island
    [180, 0],              // Antimeridian
    [0, 90],               // North Pole
    [0, -90],              // South Pole
  ];

  const width = 928, height = 500;
  const scale_ortho = 250;
  const scale_equirect = width / (2 * Math.PI);

  // Orthographic projection
  const ortho = d3.geoOrthographic()
    .scale(scale_ortho)
    .translate([width / 2, height / 2])
    .rotate([0, 0]);

  // Equirectangular projection
  const equirect = d3.geoEquirectangular()
    .scale(scale_equirect)
    .translate([width / 2, height / 2]);

  // Project points through both projections
  const orthoResults = points.map(p => {
    const proj = ortho(p);
    return { lon: p[0], lat: p[1], x: proj ? round(proj[0]) : null, y: proj ? round(proj[1]) : null };
  });

  const equirectResults = points.map(p => {
    const proj = equirect(p);
    return { lon: p[0], lat: p[1], x: proj ? round(proj[0]) : null, y: proj ? round(proj[1]) : null };
  });

  // Interpolated projection at various t values
  // Manually interpolate: lerp between ortho-raw and equirect-raw outputs
  const interpolatedResults = [0, 0.25, 0.5, 0.75, 1.0].map(t => {
    const samples = points.slice(0, 5).map(p => {
      const o = ortho(p);
      const e = equirect(p);
      if (!o || !e) return { lon: p[0], lat: p[1], x: null, y: null };
      return {
        lon: p[0], lat: p[1],
        x: round(o[0] + t * (e[0] - o[0])),
        y: round(o[1] + t * (e[1] - o[1]))
      };
    });
    return { t, samples };
  });

  // Inversion tests
  const invertTests = [[width / 2, height / 2], [100, 100], [800, 400]].map(([x, y]) => ({
    x, y,
    ortho_invert: ortho.invert([x, y])?.map(v => round(v)) ?? null,
    equirect_invert: equirect.invert([x, y])?.map(v => round(v)) ?? null
  }));

  testCases.push({
    name: "ortho_to_equirect",
    description: "Projection interpolation between orthographic and equirectangular",
    layout: { width, height },
    ortho_config: { scale: scale_ortho, translate: [width / 2, height / 2], rotate: [0, 0] },
    equirect_config: { scale: round(scale_equirect), translate: [width / 2, height / 2] },
    orthographic: orthoResults,
    equirectangular: equirectResults,
    interpolated: interpolatedResults,
    inversion: invertTests
  });

  writeGolden('ortho_to_equirect.json', createGoldenFile(
    "https://observablehq.com/@d3/orthographic-to-equirectangular", "d3-geo", "projection_interpolation", testCases));
}

// ============================================================================
// ZOOMABLE CIRCLE PACKING — https://observablehq.com/@d3/zoomable-circle-packing
// ============================================================================

function generateCirclePackingObservable() {
  const testCases = [];

  // Flare-like hierarchy data
  const flareData = {
    name: "root",
    children: [
      { name: "analytics", children: [
        { name: "cluster", children: [
          { name: "AgglomerativeCluster", value: 3938 },
          { name: "CommunityStructure", value: 3812 },
          { name: "HierarchicalCluster", value: 6714 }
        ]},
        { name: "graph", children: [
          { name: "BetweennessCentrality", value: 3534 },
          { name: "LinkDistance", value: 5731 }
        ]},
        { name: "optimization", children: [
          { name: "AspectRatioLayout", value: 7074 }
        ]}
      ]},
      { name: "animate", children: [
        { name: "Easing", value: 17010 },
        { name: "FunctionSequence", value: 5842 },
        { name: "interpolate", children: [
          { name: "ArrayInterpolator", value: 1983 },
          { name: "ColorInterpolator", value: 2047 },
          { name: "NumberInterpolator", value: 1781 }
        ]},
        { name: "Parallel", value: 5176 },
        { name: "Pause", value: 449 }
      ]},
      { name: "data", children: [
        { name: "converters", children: [
          { name: "Converters", value: 721 },
          { name: "DelimitedTextConverter", value: 4294 },
          { name: "JSONConverter", value: 2220 }
        ]},
        { name: "DataField", value: 1759 },
        { name: "DataSchema", value: 2165 },
        { name: "DataUtil", value: 3322 }
      ]},
      { name: "display", children: [
        { name: "DirtySprite", value: 8833 },
        { name: "LineSprite", value: 1732 },
        { name: "RectSprite", value: 3623 },
        { name: "TextSprite", value: 10066 }
      ]},
      { name: "query", children: [
        { name: "AggregateExpression", value: 1616 },
        { name: "And", value: 1027 },
        { name: "Average", value: 891 },
        { name: "Count", value: 781 },
        { name: "Query", value: 13896 },
        { name: "Sum", value: 791 },
        { name: "Variable", value: 1124 }
      ]}
    ]
  };

  const width = 928, height = 928;

  const pack = d3.pack().size([width, height]).padding(3);
  const root = d3.hierarchy(flareData).sum(d => d.value).sort((a, b) => b.value - a.value);
  pack(root);

  // Color scale
  const color = d3.scaleLinear().domain([0, 5]).range(["hsl(152,80%,80%)", "hsl(228,30%,40%)"])
    .interpolate(d3.interpolateHcl);

  testCases.push({
    name: "circle_packing",
    description: "Zoomable circle packing layout from Observable",
    layout: { width, height, padding: 3 },
    node_count: root.descendants().length,
    leaf_count: root.leaves().length,
    root_value: root.value,
    // All nodes with positions
    nodes: root.descendants().map(d => ({
      name: d.data.name,
      depth: d.depth,
      height: d.height,
      value: d.value,
      x: round(d.x),
      y: round(d.y),
      r: round(d.r),
      is_leaf: !d.children,
      child_count: d.children ? d.children.length : 0
    })),
    // Color at each depth
    depth_colors: [0, 1, 2, 3, 4, 5].map(d => ({
      depth: d,
      color: color(d)
    })),
    // Zoom: interpolateZoom between root and first child
    zoom_test: {
      from: [root.x, root.y, root.r * 2],
      to: [root.children[0].x, root.children[0].y, root.children[0].r * 2],
      // interpolateZoom at t=0.5
      midpoint: (() => {
        const iz = d3.interpolateZoom(
          [root.x, root.y, root.r * 2],
          [root.children[0].x, root.children[0].y, root.children[0].r * 2]
        );
        const mid = iz(0.5);
        return [round(mid[0]), round(mid[1]), round(mid[2])];
      })()
    }
  });

  writeGolden('circle_packing.json', createGoldenFile(
    "https://observablehq.com/@d3/zoomable-circle-packing", "d3-hierarchy", "circle_packing", testCases));
}

// ============================================================================
// SUNBURST — https://observablehq.com/@d3/sunburst
// ============================================================================

function generateSunburstObservable() {
  const testCases = [];

  // Same flare-like data
  const flareData = {
    name: "root",
    children: [
      { name: "analytics", children: [
        { name: "cluster", value: 10000 },
        { name: "graph", value: 8000 },
        { name: "optimization", value: 5000 }
      ]},
      { name: "animate", children: [
        { name: "Easing", value: 17000 },
        { name: "Parallel", value: 5000 },
        { name: "interpolate", children: [
          { name: "ArrayInterp", value: 2000 },
          { name: "ColorInterp", value: 3000 },
          { name: "NumberInterp", value: 1800 }
        ]}
      ]},
      { name: "data", children: [
        { name: "DataField", value: 1800 },
        { name: "DataSchema", value: 2200 },
        { name: "DataUtil", value: 3300 }
      ]},
      { name: "display", children: [
        { name: "DirtySprite", value: 8800 },
        { name: "LineSprite", value: 1700 },
        { name: "TextSprite", value: 10000 }
      ]}
    ]
  };

  const width = 928, height = 928;
  const radius = Math.min(width, height) / 2;

  const root = d3.hierarchy(flareData)
    .sum(d => d.value)
    .sort((a, b) => b.value - a.value);

  const partition = d3.partition().size([2 * Math.PI, radius]);
  partition(root);

  // Arc generator (matches Observable sunburst)
  const padding = 1;
  const arc = d3.arc()
    .startAngle(d => d.x0)
    .endAngle(d => d.x1)
    .padAngle(d => Math.min((d.x1 - d.x0) / 2, 2 * padding / radius))
    .padRadius(radius / 2)
    .innerRadius(d => d.y0)
    .outerRadius(d => d.y1 - padding);

  testCases.push({
    name: "sunburst",
    description: "Sunburst partition layout from Observable",
    layout: { width, height, radius, padding },
    node_count: root.descendants().length,
    // Partition coordinates and arc paths for each node
    nodes: root.descendants().map(d => ({
      name: d.data.name,
      depth: d.depth,
      value: d.value,
      x0: round(d.x0),
      x1: round(d.x1),
      y0: round(d.y0),
      y1: round(d.y1),
      arc_path: d.depth > 0 ? arc(d) : null
    })),
    // Root node has special handling (full circle, no arc)
    root_extent: { x0: round(root.x0), x1: round(root.x1), y0: root.y0, y1: root.y1 }
  });

  writeGolden('sunburst.json', createGoldenFile(
    "https://observablehq.com/@d3/sunburst", "d3-hierarchy", "sunburst", testCases));
}

// ============================================================================
// VERSOR DRAGGING — https://observablehq.com/@d3/versor-dragging
// Quaternion-based rotation for orthographic globe dragging
// ============================================================================

function generateVersorDraggingObservable() {
  const testCases = [];

  // Versor math (quaternion operations from the Observable example)
  function cross(a, b) {
    return [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
  }
  function dot(a, b) {
    return a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  }
  function cartesian(coords) {
    const lambda = coords[0] * Math.PI / 180, phi = coords[1] * Math.PI / 180;
    return [Math.cos(phi) * Math.cos(lambda), Math.cos(phi) * Math.sin(lambda), Math.sin(phi)];
  }
  function quaternionMultiply(a, b) {
    return [
      a[0] * b[0] - a[1] * b[1] - a[2] * b[2] - a[3] * b[3],
      a[0] * b[1] + a[1] * b[0] + a[2] * b[3] - a[3] * b[2],
      a[0] * b[2] - a[1] * b[3] + a[2] * b[0] + a[3] * b[1],
      a[0] * b[3] + a[1] * b[2] - a[2] * b[1] + a[3] * b[0]
    ];
  }
  // Versor: quaternion from two unit vectors on the sphere
  function versor(a, b) {
    const c = cross(a, b);
    const d = dot(a, b);
    const l = Math.sqrt(c[0] * c[0] + c[1] * c[1] + c[2] * c[2]);
    if (l === 0) return [1, 0, 0, 0];
    const theta = Math.atan2(l, d);
    const s = Math.sin(theta / 2) / l;
    return [Math.cos(theta / 2), c[0] * s, c[1] * s, c[2] * s];
  }
  // Convert quaternion to Euler angles [lambda, phi, gamma]
  function quaternionToEuler(q) {
    return [
      Math.atan2(2 * (q[0] * q[1] + q[2] * q[3]), 1 - 2 * (q[1] * q[1] + q[2] * q[2])) * 180 / Math.PI,
      Math.asin(Math.max(-1, Math.min(1, 2 * (q[0] * q[2] - q[3] * q[1])))) * 180 / Math.PI,
      Math.atan2(2 * (q[0] * q[3] + q[1] * q[2]), 1 - 2 * (q[2] * q[2] + q[3] * q[3])) * 180 / Math.PI
    ];
  }

  const width = 928, height = 500;

  // Test: project a point, drag to new position, compute rotation
  const projection = d3.geoOrthographic()
    .scale(250)
    .translate([width / 2, height / 2])
    .rotate([0, 0, 0]);

  // Simulate drag from San Francisco to Tokyo
  const startGeo = [-122.4, 37.8]; // SF
  const endGeo = [139.7, 35.7];    // Tokyo

  const p0 = cartesian(startGeo);
  const p1 = cartesian(endGeo);
  const q = versor(p0, p1);
  const euler = quaternionToEuler(q);

  // Project several cities before and after rotation
  const cities = [
    { name: "San Francisco", coords: [-122.4, 37.8] },
    { name: "New York", coords: [-74.0, 40.7] },
    { name: "London", coords: [-0.1, 51.5] },
    { name: "Tokyo", coords: [139.7, 35.7] },
    { name: "Sydney", coords: [151.2, -33.9] },
    { name: "Sao Paulo", coords: [-46.6, -23.5] }
  ];

  const beforeRotation = cities.map(c => {
    const p = projection(c.coords);
    return { name: c.name, x: p ? round(p[0]) : null, y: p ? round(p[1]) : null };
  });

  projection.rotate(euler);
  const afterRotation = cities.map(c => {
    const p = projection(c.coords);
    return { name: c.name, x: p ? round(p[0]) : null, y: p ? round(p[1]) : null };
  });

  testCases.push({
    name: "versor_dragging",
    description: "Quaternion-based globe rotation via versor drag",
    layout: { width, height },
    projection_config: { scale: 250, translate: [width / 2, height / 2] },
    drag: {
      start_geo: startGeo, end_geo: endGeo,
      start_cartesian: p0.map(v => round(v)),
      end_cartesian: p1.map(v => round(v)),
      quaternion: q.map(v => round(v)),
      euler_rotation: euler.map(v => round(v))
    },
    before_rotation: beforeRotation,
    after_rotation: afterRotation
  });

  writeGolden('versor_dragging.json', createGoldenFile(
    "https://observablehq.com/@d3/versor-dragging", "d3-geo", "versor_dragging", testCases));
}

// ============================================================================
// BOX PLOT — https://observablehq.com/@d3/box-plot
// ============================================================================

function generateBoxPlotObservable() {
  const testCases = [];

  // Deterministic data: 5 groups with varying distributions
  const groups = ["A", "B", "C", "D", "E"];
  const data = [];
  for (const [gi, group] of groups.entries()) {
    const base = 20 + gi * 15;
    const spread = 5 + gi * 3;
    for (let i = 0; i < 50; i++) {
      // Deterministic pseudo-random using sin
      const r = Math.sin(i * 7.3 + gi * 13.1) * 0.5 + 0.5;
      let value = base + (r - 0.5) * spread * 2;
      // Add a few outliers
      if (i % 17 === 0) value = base + spread * 4 * (r > 0.5 ? 1 : -1);
      data.push({ group, value: round(value, 2) });
    }
  }

  const width = 928, height = 500;
  const marginTop = 20, marginRight = 20, marginBottom = 30, marginLeft = 40;

  // Compute box plot statistics per group
  const boxStats = groups.map(group => {
    const values = data.filter(d => d.group === group).map(d => d.value).sort(d3.ascending);
    const q1 = d3.quantile(values, 0.25);
    const q2 = d3.quantile(values, 0.50);
    const q3 = d3.quantile(values, 0.75);
    const iqr = q3 - q1;
    const r0 = Math.max(d3.min(values), q1 - iqr * 1.5);
    const r1 = Math.min(d3.max(values), q3 + iqr * 1.5);
    const outliers = values.filter(v => v < r0 || v > r1);

    return {
      group,
      count: values.length,
      min: round(d3.min(values)),
      max: round(d3.max(values)),
      q1: round(q1),
      median: round(q2),
      q3: round(q3),
      iqr: round(iqr),
      whisker_low: round(r0),
      whisker_high: round(r1),
      outliers: outliers.map(v => round(v))
    };
  });

  // Scales
  const xScale = d3.scaleBand()
    .domain(groups)
    .range([marginLeft, width - marginRight])
    .paddingInner(0.4);

  const allValues = data.map(d => d.value);
  const yScale = d3.scaleLinear()
    .domain(d3.extent(allValues))
    .nice()
    .range([height - marginBottom, marginTop]);

  testCases.push({
    name: "box_plot",
    description: "Box plot with quartiles, whiskers, and outliers",
    layout: { width, height, marginTop, marginRight, marginBottom, marginLeft },
    data_count: data.length,
    groups: boxStats,
    x_scale: {
      type: "band", domain: groups,
      range: [marginLeft, width - marginRight],
      padding: 0.4,
      bandwidth: round(xScale.bandwidth()),
      step: round(xScale.step()),
      positions: groups.map(g => ({ group: g, position: round(xScale(g)) }))
    },
    y_scale: {
      type: "linear", domain: yScale.domain().map(v => round(v)),
      range: yScale.range(),
      samples: boxStats.map(s => ({
        group: s.group,
        q1_y: round(yScale(s.q1)),
        median_y: round(yScale(s.median)),
        q3_y: round(yScale(s.q3)),
        whisker_low_y: round(yScale(s.whisker_low)),
        whisker_high_y: round(yScale(s.whisker_high))
      }))
    }
  });

  writeGolden('box_plot.json', createGoldenFile(
    "https://observablehq.com/@d3/box-plot", "d3-array", "box_plot", testCases));
}

// ============================================================================
// FORCE-DIRECTED GRAPH — https://observablehq.com/@d3/force-directed-graph
// ============================================================================

function generateForceDirectedObservable() {
  const testCases = [];

  // Les Miserables-like graph (small subset)
  const nodes = [
    { id: "Myriel", group: 1 }, { id: "Napoleon", group: 1 },
    { id: "Labarre", group: 2 }, { id: "Valjean", group: 2 },
    { id: "Marguerite", group: 3 }, { id: "Mme.deR", group: 2 },
    { id: "Isabeau", group: 2 }, { id: "Gervais", group: 2 },
    { id: "Tholomyes", group: 3 }, { id: "Listolier", group: 3 },
    { id: "Fameuil", group: 3 }, { id: "Blacheville", group: 3 },
    { id: "Favourite", group: 3 }, { id: "Dahlia", group: 3 },
    { id: "Zephine", group: 3 }, { id: "Fantine", group: 3 },
    { id: "Cosette", group: 4 }, { id: "Javert", group: 4 },
    { id: "Fauchelevent", group: 5 }, { id: "Bamatabois", group: 5 }
  ];

  const links = [
    { source: "Napoleon", target: "Myriel", value: 1 },
    { source: "Labarre", target: "Valjean", value: 1 },
    { source: "Mme.deR", target: "Valjean", value: 1 },
    { source: "Isabeau", target: "Valjean", value: 1 },
    { source: "Gervais", target: "Valjean", value: 1 },
    { source: "Marguerite", target: "Valjean", value: 1 },
    { source: "Tholomyes", target: "Fantine", value: 3 },
    { source: "Listolier", target: "Tholomyes", value: 4 },
    { source: "Fameuil", target: "Tholomyes", value: 4 },
    { source: "Blacheville", target: "Tholomyes", value: 4 },
    { source: "Favourite", target: "Tholomyes", value: 3 },
    { source: "Dahlia", target: "Tholomyes", value: 3 },
    { source: "Zephine", target: "Tholomyes", value: 3 },
    { source: "Fantine", target: "Valjean", value: 5 },
    { source: "Cosette", target: "Valjean", value: 4 },
    { source: "Javert", target: "Valjean", value: 6 },
    { source: "Fauchelevent", target: "Valjean", value: 2 },
    { source: "Bamatabois", target: "Valjean", value: 1 },
    { source: "Cosette", target: "Javert", value: 1 },
    { source: "Cosette", target: "Fantine", value: 5 }
  ];

  // Create simulation (matches Observable example)
  const simNodes = nodes.map(d => ({ ...d }));
  const simLinks = links.map(d => ({ ...d }));

  const simulation = d3.forceSimulation(simNodes)
    .force("link", d3.forceLink(simLinks).id(d => d.id))
    .force("charge", d3.forceManyBody())
    .force("center", d3.forceCenter(0, 0))
    .stop();

  // Run for fixed iterations
  const iterations = 300;
  for (let i = 0; i < iterations; i++) {
    simulation.tick();
  }

  testCases.push({
    name: "force_directed_graph",
    description: "Force-directed graph layout from Observable",
    iterations,
    node_count: simNodes.length,
    link_count: simLinks.length,
    forces: {
      link: { type: "forceLink", id: "d.id" },
      charge: { type: "forceManyBody", strength: -30 },
      center: { type: "forceCenter", x: 0, y: 0 }
    },
    nodes: simNodes.map(n => ({
      id: n.id, group: n.group,
      x: round(n.x), y: round(n.y),
      vx: round(n.vx), vy: round(n.vy)
    })),
    links: simLinks.map(l => ({
      source: typeof l.source === 'object' ? l.source.id : l.source,
      target: typeof l.target === 'object' ? l.target.id : l.target,
      value: l.value
    })),
    // Simulation state
    alpha: round(simulation.alpha()),
    alpha_min: simulation.alphaMin(),
    alpha_decay: round(simulation.alphaDecay())
  });

  writeGolden('force_directed.json', createGoldenFile(
    "https://observablehq.com/@d3/force-directed-graph", "d3-force", "force_directed", testCases));
}

// ============================================================================
// SANKEY — https://observablehq.com/@d3/sankey
// ============================================================================

function generateSankeyObservable() {
  const { sankey, sankeyLinkHorizontal, sankeyJustify } = require('d3-sankey');
  const testCases = [];

  // Energy flow data (subset of the classic Sankey example)
  const nodesData = [
    { id: "Agricultural 'Waste'" },
    { id: "Bio-conversion" },
    { id: "Liquid" },
    { id: "Losses" },
    { id: "Solid" },
    { id: "Gas" },
    { id: "Biofuel imports" },
    { id: "Biomass imports" },
    { id: "Coal imports" },
    { id: "Coal" },
    { id: "Coal reserves" },
    { id: "Electricity grid" },
    { id: "Thermal generation" },
    { id: "District heating" }
  ];

  const linksData = [
    { source: "Agricultural 'Waste'", target: "Bio-conversion", value: 124.729 },
    { source: "Bio-conversion", target: "Liquid", value: 0.597 },
    { source: "Bio-conversion", target: "Losses", value: 26.862 },
    { source: "Bio-conversion", target: "Solid", value: 280.322 },
    { source: "Bio-conversion", target: "Gas", value: 81.144 },
    { source: "Biofuel imports", target: "Liquid", value: 35.0 },
    { source: "Biomass imports", target: "Solid", value: 35.0 },
    { source: "Coal imports", target: "Coal", value: 11.606 },
    { source: "Coal reserves", target: "Coal", value: 63.965 },
    { source: "Coal", target: "Solid", value: 75.571 },
    { source: "Solid", target: "Thermal generation", value: 390.893 },
    { source: "Thermal generation", target: "Electricity grid", value: 325.239 },
    { source: "Thermal generation", target: "Losses", value: 22.505 },
    { source: "Thermal generation", target: "District heating", value: 43.149 }
  ];

  const width = 928, height = 600;
  const marginTop = 5, marginRight = 1, marginBottom = 5, marginLeft = 1;

  const graph = sankey()
    .nodeId(d => d.id)
    .nodeAlign(sankeyJustify)
    .nodeWidth(15)
    .nodePadding(10)
    .extent([[marginLeft, marginTop], [width - marginRight, height - marginBottom]])
    ({
      nodes: nodesData.map(d => ({ ...d })),
      links: linksData.map(d => ({ ...d }))
    });

  const linkPath = sankeyLinkHorizontal();

  testCases.push({
    name: "sankey_energy",
    description: "Sankey energy flow diagram from Observable",
    layout: { width, height, marginTop, marginRight, marginBottom, marginLeft },
    config: { nodeWidth: 15, nodePadding: 10, nodeAlign: "justify" },
    node_count: graph.nodes.length,
    link_count: graph.links.length,
    nodes: graph.nodes.map(n => ({
      id: n.id, index: n.index,
      x0: round(n.x0), x1: round(n.x1),
      y0: round(n.y0), y1: round(n.y1),
      value: round(n.value),
      depth: n.depth, height: n.height, layer: n.layer
    })),
    links: graph.links.map(l => ({
      source: l.source.id, target: l.target.id,
      value: l.value,
      y0: round(l.y0), y1: round(l.y1),
      width: round(l.width),
      path: linkPath(l)
    }))
  });

  writeGolden('sankey.json', createGoldenFile(
    "https://observablehq.com/@d3/sankey", "d3-sankey", "sankey", testCases));
}

// ============================================================================
// CHORD DIAGRAM — https://observablehq.com/@d3/chord-diagram
// ============================================================================

function generateChordObservable() {
  const testCases = [];

  // Phone market share matrix (from Observable chord diagram)
  const matrix = [
    [0.096899, 0.008859, 0.000554, 0.004430, 0.025471, 0.024363, 0.005537, 0.025471],
    [0.001107, 0.018272, 0.000000, 0.004983, 0.011074, 0.010520, 0.002215, 0.004983],
    [0.000554, 0.002769, 0.002215, 0.002215, 0.003876, 0.008306, 0.000554, 0.003322],
    [0.000554, 0.001107, 0.000554, 0.012182, 0.011628, 0.006645, 0.004983, 0.010520],
    [0.002215, 0.004430, 0.000000, 0.002769, 0.104097, 0.012182, 0.004983, 0.028239],
    [0.011628, 0.026024, 0.000000, 0.013843, 0.087486, 0.168328, 0.017165, 0.055925],
    [0.000554, 0.004983, 0.000000, 0.003322, 0.004430, 0.008859, 0.017719, 0.004430],
    [0.002215, 0.007198, 0.000000, 0.003322, 0.016611, 0.014950, 0.001107, 0.054264]
  ];
  const names = ["Apple", "HTC", "Huawei", "LG", "Nokia", "Samsung", "Sony", "Other"];
  const colors = ["#c4c4c4", "#69b40f", "#ec1d25", "#c8125c", "#008fc8", "#10218b", "#134b24", "#737373"];

  const width = 928, height = 928;
  const outerRadius = Math.min(width, height) * 0.5 - 60;
  const innerRadius = outerRadius - 10;

  const chord = d3.chord()
    .padAngle(10 / innerRadius)
    .sortSubgroups(d3.descending)
    .sortChords(d3.descending);

  const chords = chord(matrix);

  const arc = d3.arc().innerRadius(innerRadius).outerRadius(outerRadius);

  // Ribbon generator
  const ribbon = d3.ribbon().radius(innerRadius - 1).padAngle(1 / innerRadius);

  testCases.push({
    name: "chord_phones",
    description: "Chord diagram of phone market share from Observable",
    layout: { width, height, innerRadius: round(innerRadius), outerRadius: round(outerRadius) },
    matrix_size: matrix.length,
    names,
    pad_angle: round(10 / innerRadius),
    // Groups (arcs)
    groups: chords.groups.map((g, i) => ({
      index: g.index,
      name: names[g.index],
      value: round(g.value),
      startAngle: round(g.startAngle),
      endAngle: round(g.endAngle),
      arc_path: arc(g)
    })),
    // Chords (ribbons)
    chords: chords.map(c => ({
      source: {
        index: c.source.index,
        name: names[c.source.index],
        startAngle: round(c.source.startAngle),
        endAngle: round(c.source.endAngle),
        value: round(c.source.value)
      },
      target: {
        index: c.target.index,
        name: names[c.target.index],
        startAngle: round(c.target.startAngle),
        endAngle: round(c.target.endAngle),
        value: round(c.target.value)
      },
      ribbon_path: ribbon(c)
    })),
    chord_count: chords.length
  });

  writeGolden('chord.json', createGoldenFile(
    "https://observablehq.com/@d3/chord-diagram", "d3-chord", "chord_observable", testCases));
}

// ============================================================================
// STACKED BAR CHART — https://observablehq.com/@d3/stacked-bar-chart
// ============================================================================

function generateStackedBarObservable() {
  const testCases = [];

  // Population data by state and age group
  const ages = ["<10", "10-19", "20-29", "30-39", "40-49", "50-59", "60-69", "70+"];
  const states = [
    { name: "California", "<10": 5038, "10-19": 5170, "20-29": 5765, "30-39": 5430, "40-49": 5044, "50-59": 4835, "60-69": 3738, "70+": 2920 },
    { name: "Texas", "<10": 3983, "10-19": 3862, "20-29": 3872, "30-39": 3678, "40-49": 3360, "50-59": 3092, "60-69": 2388, "70+": 1708 },
    { name: "Florida", "<10": 2211, "10-19": 2331, "20-29": 2641, "30-39": 2574, "40-49": 2524, "50-59": 2685, "60-69": 2462, "70+": 2285 },
    { name: "New York", "<10": 2334, "10-19": 2470, "20-29": 2903, "30-39": 2700, "40-49": 2523, "50-59": 2706, "60-69": 2128, "70+": 1709 },
    { name: "Illinois", "<10": 1625, "10-19": 1710, "20-29": 1826, "30-39": 1699, "40-49": 1591, "50-59": 1688, "60-69": 1259, "70+": 953 }
  ];

  const width = 928, height = 500;
  const marginTop = 10, marginRight = 10, marginBottom = 20, marginLeft = 40;

  // Stack
  const stack = d3.stack()
    .keys(ages)
    .order(d3.stackOrderNone)
    .offset(d3.stackOffsetDiverging);

  const stackData = states.map(s => {
    const row = {};
    for (const age of ages) row[age] = s[age];
    return row;
  });
  const series = stack(stackData);

  // Scales
  const xScale = d3.scaleBand()
    .domain(states.map(s => s.name))
    .range([marginLeft, width - marginRight])
    .paddingInner(0.1);

  const yExtent = d3.extent(series.flat(2));
  const yScale = d3.scaleLinear()
    .domain(yExtent)
    .rangeRound([height - marginBottom, marginTop]);

  const color = d3.scaleOrdinal().domain(ages).range(d3.schemeTableau10);

  testCases.push({
    name: "stacked_bar",
    description: "Stacked bar chart of population by state and age",
    layout: { width, height, marginTop, marginRight, marginBottom, marginLeft },
    categories: ages,
    states: states.map(s => s.name),
    stack_config: { order: "none", offset: "diverging" },
    series: series.map(s => ({
      key: s.key,
      index: s.index,
      values: s.map(d => [round(d[0]), round(d[1])])
    })),
    x_scale: {
      type: "band",
      domain: states.map(s => s.name),
      range: [marginLeft, width - marginRight],
      padding: 0.1,
      bandwidth: round(xScale.bandwidth()),
      step: round(xScale.step()),
      positions: states.map(s => ({ state: s.name, position: round(xScale(s.name)) }))
    },
    y_scale: {
      type: "linear",
      domain: yScale.domain().map(v => round(v)),
      range: yScale.range()
    },
    color_scale: {
      type: "ordinal",
      domain: ages,
      range: ages.map(a => color(a))
    },
    // Bar rectangles for first state (full detail)
    bars_first_state: series.map(s => ({
      key: s.key,
      y0: round(yScale(s[0][0])),
      y1: round(yScale(s[0][1])),
      x: round(xScale(states[0].name)),
      width: round(xScale.bandwidth()),
      color: color(s.key)
    }))
  });

  writeGolden('stacked_bar.json', createGoldenFile(
    "https://observablehq.com/@d3/stacked-bar-chart", "d3-shape", "stacked_bar", testCases));
}

// ============================================================================
// STACKED AREA CHART — https://observablehq.com/@d3/stacked-area-chart
// ============================================================================

function generateStackedAreaObservable() {
  const testCases = [];

  // Time series with multiple categories
  const categories = ["Electronics", "Clothing", "Food", "Transport"];
  const months = 12;
  const data = [];
  for (let m = 0; m < months; m++) {
    const row = { month: m };
    for (const [ci, cat] of categories.entries()) {
      const base = 50 + ci * 20;
      row[cat] = round(base + 15 * Math.sin(m * 0.5 + ci * 1.2) + 5 * Math.cos(m * 0.8 + ci * 0.5), 2);
    }
    data.push(row);
  }

  const width = 928, height = 500;
  const marginTop = 20, marginRight = 20, marginBottom = 30, marginLeft = 40;

  const stack = d3.stack().keys(categories).order(d3.stackOrderNone).offset(d3.stackOffsetNone);
  const series = stack(data);

  const xScale = d3.scaleLinear().domain([0, months - 1]).range([marginLeft, width - marginRight]);
  const yExtent = [0, d3.max(series, s => d3.max(s, d => d[1]))];
  const yScale = d3.scaleLinear().domain(yExtent).range([height - marginBottom, marginTop]);

  const area = d3.area()
    .x((d, i) => xScale(i))
    .y0(d => yScale(d[0]))
    .y1(d => yScale(d[1]))
    .curve(d3.curveMonotoneX);

  const color = d3.scaleOrdinal().domain(categories).range(d3.schemeTableau10);

  testCases.push({
    name: "stacked_area",
    description: "Stacked area chart with monotone curve",
    layout: { width, height, marginTop, marginRight, marginBottom, marginLeft },
    categories,
    data_count: data.length,
    stack_config: { order: "none", offset: "none" },
    series: series.map(s => ({
      key: s.key, index: s.index,
      values: s.map(d => [round(d[0]), round(d[1])])
    })),
    x_scale: { type: "linear", domain: xScale.domain(), range: xScale.range() },
    y_scale: { type: "linear", domain: yScale.domain().map(v => round(v)), range: yScale.range() },
    color_scale: { domain: categories, range: categories.map(c => color(c)) },
    area_paths: series.map(s => ({ key: s.key, path: area(s) })),
    curve: "monotoneX"
  });

  writeGolden('stacked_area.json', createGoldenFile(
    "https://observablehq.com/@d3/stacked-area-chart", "d3-shape", "stacked_area", testCases));
}

// ============================================================================
// LINE CHART — https://observablehq.com/@d3/line-chart
// ============================================================================

function generateLineChartObservable() {
  const testCases = [];

  // Temperature-like time series
  const n = 30;
  const data = [];
  for (let i = 0; i < n; i++) {
    const t = i / (n - 1);
    const value = 15 + 10 * Math.sin(t * 2 * Math.PI) + 3 * Math.cos(t * 4 * Math.PI) + 2 * Math.sin(i * 1.7);
    data.push({ day: i, value: round(value, 2) });
  }

  const width = 928, height = 500;
  const marginTop = 20, marginRight = 30, marginBottom = 30, marginLeft = 40;

  const xScale = d3.scaleLinear().domain([0, n - 1]).range([marginLeft, width - marginRight]);
  const yScale = d3.scaleLinear()
    .domain(d3.extent(data, d => d.value)).nice()
    .range([height - marginBottom, marginTop]);

  // Multiple curve types for comparison
  const curves = {
    linear: d3.curveLinear,
    monotoneX: d3.curveMonotoneX,
    catmullRom: d3.curveCatmullRom,
    basis: d3.curveBasis,
    natural: d3.curveNatural,
    step: d3.curveStep,
    cardinal: d3.curveCardinal
  };

  const paths = {};
  for (const [name, curve] of Object.entries(curves)) {
    const line = d3.line()
      .x(d => xScale(d.day))
      .y(d => yScale(d.value))
      .curve(curve);
    paths[name] = line(data);
  }

  testCases.push({
    name: "line_chart",
    description: "Line chart with multiple curve types",
    layout: { width, height, marginTop, marginRight, marginBottom, marginLeft },
    data, data_count: n,
    x_scale: {
      type: "linear", domain: xScale.domain(), range: xScale.range(),
      samples: data.filter((_, i) => i % 5 === 0).map(d => ({ input: d.day, output: round(xScale(d.day)) }))
    },
    y_scale: {
      type: "linear", domain: yScale.domain().map(v => round(v)), range: yScale.range(),
      samples: data.filter((_, i) => i % 5 === 0).map(d => ({ input: d.value, output: round(yScale(d.value)) }))
    },
    line_paths: paths
  });

  writeGolden('line_chart.json', createGoldenFile(
    "https://observablehq.com/@d3/line-chart", "d3-shape", "line_chart", testCases));
}

// ============================================================================
// PIE CHART — https://observablehq.com/@d3/pie-chart
// ============================================================================

function generatePieChartObservable() {
  const testCases = [];

  const data = [
    { name: "Residential", value: 48.5 },
    { name: "Commercial", value: 18.6 },
    { name: "Industrial", value: 13.1 },
    { name: "Transportation", value: 11.3 },
    { name: "Other", value: 8.5 }
  ];

  const width = 928, height = 500;
  const radius = Math.min(width, height) / 2;

  const pie = d3.pie()
    .sort(null)
    .value(d => d.value);

  const slices = pie(data);

  const arc = d3.arc()
    .innerRadius(0)
    .outerRadius(radius - 1);

  // Label arc (for text positioning)
  const labelRadius = radius * 2 / 3;
  const labelArc = d3.arc()
    .innerRadius(labelRadius)
    .outerRadius(labelRadius);

  const color = d3.scaleOrdinal()
    .domain(data.map(d => d.name))
    .range(d3.quantize(t => d3.interpolateSpectral(t * 0.8 + 0.1), data.length).reverse());

  testCases.push({
    name: "pie_chart",
    description: "Pie chart with spectral color scheme",
    layout: { width, height, radius },
    data,
    slices: slices.map(s => ({
      name: s.data.name,
      value: s.data.value,
      index: s.index,
      startAngle: round(s.startAngle),
      endAngle: round(s.endAngle),
      padAngle: round(s.padAngle),
      arc_path: arc(s),
      label_position: labelArc.centroid(s).map(v => round(v)),
      centroid: arc.centroid(s).map(v => round(v)),
      color: color(s.data.name)
    })),
    total_value: d3.sum(data, d => d.value)
  });

  writeGolden('pie_chart.json', createGoldenFile(
    "https://observablehq.com/@d3/pie-chart", "d3-shape", "pie_chart", testCases));
}

// ============================================================================
// DONUT CHART — https://observablehq.com/@d3/donut-chart
// ============================================================================

function generateDonutChartObservable() {
  const testCases = [];

  const data = [
    { name: "JavaScript", value: 67.7 },
    { name: "Python", value: 44.1 },
    { name: "TypeScript", value: 34.8 },
    { name: "Java", value: 33.3 },
    { name: "C#", value: 27.6 },
    { name: "Rust", value: 13.0 },
    { name: "Go", value: 11.2 }
  ];

  const width = 928, height = 500;
  const radius = Math.min(width, height) / 2;
  const innerRadius = radius * 0.67;

  const pie = d3.pie()
    .padAngle(1 / radius)
    .sort(null)
    .value(d => d.value);

  const slices = pie(data);

  const arc = d3.arc()
    .innerRadius(innerRadius)
    .outerRadius(radius - 1);

  const color = d3.scaleOrdinal()
    .domain(data.map(d => d.name))
    .range(d3.quantize(t => d3.interpolateSpectral(t * 0.8 + 0.1), data.length).reverse());

  testCases.push({
    name: "donut_chart",
    description: "Donut chart with pad angle and spectral colors",
    layout: { width, height, radius, innerRadius: round(innerRadius) },
    pad_angle: round(1 / radius),
    data,
    slices: slices.map(s => ({
      name: s.data.name,
      value: s.data.value,
      index: s.index,
      startAngle: round(s.startAngle),
      endAngle: round(s.endAngle),
      padAngle: round(s.padAngle),
      arc_path: arc(s),
      centroid: arc.centroid(s).map(v => round(v)),
      color: color(s.data.name)
    })),
    total_value: d3.sum(data, d => d.value)
  });

  writeGolden('donut_chart.json', createGoldenFile(
    "https://observablehq.com/@d3/donut-chart", "d3-shape", "donut_chart", testCases));
}

// ============================================================================
// PARALLEL SETS — based on https://observablehq.com/@d3/parallel-sets
// Uses d3-sankey under the hood for flow layout
// ============================================================================

function generateParallelSetsObservable() {
  const { sankey, sankeyLinkHorizontal, sankeyJustify } = require('d3-sankey');
  const testCases = [];

  // Titanic-like categorical data: Class × Sex × Survived
  // Encoded as a Sankey flow: Class -> Sex -> Survived
  const nodesData = [
    { id: "1st Class" }, { id: "2nd Class" }, { id: "3rd Class" },
    { id: "Male" }, { id: "Female" },
    { id: "Survived" }, { id: "Died" }
  ];

  const linksData = [
    { source: "1st Class", target: "Male", value: 175 },
    { source: "1st Class", target: "Female", value: 144 },
    { source: "2nd Class", target: "Male", value: 168 },
    { source: "2nd Class", target: "Female", value: 93 },
    { source: "3rd Class", target: "Male", value: 462 },
    { source: "3rd Class", target: "Female", value: 165 },
    { source: "Male", target: "Survived", value: 161 },
    { source: "Male", target: "Died", value: 644 },
    { source: "Female", target: "Survived", value: 339 },
    { source: "Female", target: "Died", value: 63 }
  ];

  const width = 928, height = 600;

  const graph = sankey()
    .nodeId(d => d.id)
    .nodeAlign(sankeyJustify)
    .nodeWidth(15)
    .nodePadding(10)
    .extent([[1, 5], [width - 1, height - 5]])
    ({
      nodes: nodesData.map(d => ({ ...d })),
      links: linksData.map(d => ({ ...d }))
    });

  const linkPath = sankeyLinkHorizontal();

  testCases.push({
    name: "parallel_sets_titanic",
    description: "Parallel sets (Sankey-based) showing Titanic survival data",
    layout: { width, height },
    config: { nodeWidth: 15, nodePadding: 10 },
    nodes: graph.nodes.map(n => ({
      id: n.id, index: n.index,
      x0: round(n.x0), x1: round(n.x1),
      y0: round(n.y0), y1: round(n.y1),
      value: round(n.value), depth: n.depth, layer: n.layer
    })),
    links: graph.links.map(l => ({
      source: l.source.id, target: l.target.id,
      value: l.value,
      y0: round(l.y0), y1: round(l.y1),
      width: round(l.width),
      path: linkPath(l)
    }))
  });

  writeGolden('parallel_sets.json', createGoldenFile(
    "https://observablehq.com/@d3/parallel-sets", "d3-sankey", "parallel_sets", testCases));
}

// ============================================================================
// MAIN
// ============================================================================

const generators = {
  hexbin: generateHexbinObservable,
  streamgraph: generateStreamgraphObservable,
  ortho: generateOrthoToEquirectObservable,
  circle_packing: generateCirclePackingObservable,
  sunburst: generateSunburstObservable,
  versor: generateVersorDraggingObservable,
  box_plot: generateBoxPlotObservable,
  force: generateForceDirectedObservable,
  sankey: generateSankeyObservable,
  chord: generateChordObservable,
  stacked_bar: generateStackedBarObservable,
  stacked_area: generateStackedAreaObservable,
  line: generateLineChartObservable,
  pie: generatePieChartObservable,
  donut: generateDonutChartObservable,
  parallel_sets: generateParallelSetsObservable,
};

const args = process.argv.slice(2);

if (args.length === 0) {
  console.log('Generating all Observable example golden files...');
  for (const [name, fn] of Object.entries(generators)) {
    fn();
  }
  console.log('Done!');
} else {
  for (const arg of args) {
    if (generators[arg]) {
      generators[arg]();
    } else {
      console.log(`Unknown example: ${arg}. Available: ${Object.keys(generators).join(', ')}`);
    }
  }
}
