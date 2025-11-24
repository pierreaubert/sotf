use clap::Parser;
use ndarray::Array1;
use plotly::{
    Layout, Plot, Scatter,
    common::{ColorScale, ColorScalePalette, Marker, Mode, Title},
    contour::Contour,
};
use std::collections::BTreeMap;
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;

// Import environment utilities
use autoeq_env::{get_data_generated_dir, get_records_dir};

// Import the test functions and metadata
use autoeq_testfunctions::{FunctionMetadata, get_function_metadata};

// Import shared function registry
use autoeq_de::function_registry::TestFunction;

/// CLI arguments for plotting test functions
#[derive(Parser)]
#[command(name = "plot_functions")]
#[command(about = "Plot test functions using contour plots with Plotly")]
struct Args {
    /// Height of the plot in pixels
    #[arg(short = 'H', long, default_value = "800")]
    height: usize,

    /// Width of the plot in pixels
    #[arg(short = 'W', long, default_value = "800")]
    width: usize,

    /// Number of points along x-axis
    #[arg(short = 'x', long, default_value = "100")]
    xn: usize,

    /// Number of points along y-axis
    #[arg(short = 'y', long, default_value = "100")]
    yn: usize,

    /// X-axis bounds (min,max)
    #[arg(long, default_value = "-5.0,5.0")]
    x_bounds: String,

    /// Y-axis bounds (min,max)
    #[arg(long, default_value = "-5.0,5.0")]
    y_bounds: String,

    /// Output directory for HTML files
    #[arg(short, long)]
    output_dir: Option<String>,

    /// List of specific functions to plot (comma-separated), if empty plots all
    #[arg(short, long)]
    functions: Option<String>,

    /// Directory containing CSV files with optimization traces
    #[arg(long)]
    csv_dir: Option<String>,

    /// Show optimization traces from CSV files
    #[arg(long)]
    show_traces: bool,

    /// Use function metadata for bounds (overrides x_bounds and y_bounds)
    #[arg(long)]
    use_metadata: bool,

    /// Create convergence plots showing loss function vs iterations/evaluations
    #[arg(long)]
    convergence_plots: bool,
}

// TestFunction type now imported from shared function_registry

#[derive(Debug, Clone)]
struct OptimizationPoint {
    iteration: usize,
    x: Vec<f64>,
    best_result: f64,
    f_value: f64,
    is_improvement: bool,
}

#[derive(Debug, Clone)]
struct OptimizationTrace {
    function_name: String,
    points: Vec<OptimizationPoint>,
}

fn read_csv_trace(csv_path: &str) -> Result<OptimizationTrace, Box<dyn std::error::Error>> {
    let content = fs::read_to_string(csv_path)?;
    let lines: Vec<&str> = content.trim().split('\n').collect();

    if lines.len() < 2 {
        return Err("CSV file must have at least header and one data row".into());
    }

    let header = lines[0];

    // Determine CSV format based on header
    let is_new_format = header.starts_with("eval_id,generation,");
    let is_old_format = header.starts_with("iteration,");

    if !is_new_format && !is_old_format {
        return Err(format!(
            "Invalid CSV header format. Expected to start with 'eval_id,generation,' or 'iteration,', got: {}",
            header
        )
        .into());
    }

    // Extract function name from filename
    let raw_name = Path::new(csv_path)
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown");

    // Clean function name by removing _block_XXXX suffix if present
    let function_name = if raw_name.contains("_block_") {
        raw_name
            .split("_block_")
            .next()
            .unwrap_or(raw_name)
            .to_string()
    } else {
        raw_name.to_string()
    };

    let mut points = Vec::new();

    for (line_idx, line) in lines.iter().skip(1).enumerate() {
        let parts: Vec<&str> = line.split(',').collect();

        if parts.len() < 4 {
            return Err(format!("Line {}: insufficient columns", line_idx + 2).into());
        }

        let point = if is_new_format {
            // New format: eval_id,generation,x0,x1,f_value,best_so_far,is_improvement
            if parts.len() < 7 {
                return Err(format!(
                    "Line {}: insufficient columns for new format (expected 7+)",
                    line_idx + 2
                )
                .into());
            }

            let eval_id: usize = parts[0]
                .parse()
                .map_err(|_| format!("Line {}: invalid eval_id", line_idx + 2))?;

            let _generation: usize = parts[1]
                .parse()
                .map_err(|_| format!("Line {}: invalid generation", line_idx + 2))?;

            // Parse x coordinates (between generation and f_value/best_so_far/is_improvement)
            let x_end = parts.len() - 3; // f_value, best_so_far, is_improvement
            let mut x = Vec::new();
            for i in 2..x_end {
                let coord: f64 = parts[i].parse().map_err(|_| {
                    format!(
                        "Line {}: invalid x coordinate at column {}",
                        line_idx + 2,
                        i
                    )
                })?;
                x.push(coord);
            }

            if x.len() != 2 {
                return Err(format!(
                    "Line {}: expected 2D coordinates, got {}D",
                    line_idx + 2,
                    x.len()
                )
                .into());
            }

            let f_value: f64 = parts[x_end]
                .parse()
                .map_err(|_| format!("Line {}: invalid f_value", line_idx + 2))?;

            let best_so_far: f64 = parts[x_end + 1]
                .parse()
                .map_err(|_| format!("Line {}: invalid best_so_far", line_idx + 2))?;

            let is_improvement: bool = parts[x_end + 2]
                .parse()
                .map_err(|_| format!("Line {}: invalid is_improvement", line_idx + 2))?;

            OptimizationPoint {
                iteration: eval_id,
                x,
                best_result: best_so_far,
                f_value,
                is_improvement,
            }
        } else {
            // Old format: iteration,x0,x1,best_result,convergence,is_improvement
            let iteration: usize = parts[0]
                .parse()
                .map_err(|_| format!("Line {}: invalid iteration number", line_idx + 2))?;

            // Parse x coordinates (all columns between iteration and last 3 columns)
            let x_columns_end = parts.len() - 3; // best_result, convergence, is_improvement
            let mut x = Vec::new();

            for i in 1..x_columns_end {
                let coord: f64 = parts[i].parse().map_err(|_| {
                    format!(
                        "Line {}: invalid x coordinate at column {}",
                        line_idx + 2,
                        i
                    )
                })?;
                x.push(coord);
            }

            if x.len() != 2 {
                return Err(format!(
                    "Line {}: expected 2D coordinates, got {}D",
                    line_idx + 2,
                    x.len()
                )
                .into());
            }

            let best_result: f64 = parts[x_columns_end]
                .parse()
                .map_err(|_| format!("Line {}: invalid best_result", line_idx + 2))?;

            let is_improvement: bool = parts[x_columns_end + 2]
                .parse()
                .map_err(|_| format!("Line {}: invalid is_improvement", line_idx + 2))?;

            OptimizationPoint {
                iteration,
                x,
                best_result,
                f_value: best_result, // In old format, we don't have separate f_value
                is_improvement,
            }
        };

        points.push(point);
    }

    Ok(OptimizationTrace {
        function_name,
        points,
    })
}

fn find_csv_for_function(csv_dir: &str, function_name: &str) -> Vec<String> {
    autoeq_de::function_registry::find_csv_files_for_function(csv_dir, function_name)
}

fn add_optimization_trace(
    plot: &mut Plot,
    trace: &OptimizationTrace,
    x_bounds: (f64, f64),
    y_bounds: (f64, f64),
) {
    if trace.points.is_empty() {
        return;
    }

    // Filter points to only those within bounds
    let valid_points: Vec<&OptimizationPoint> = trace
        .points
        .iter()
        .filter(|point| {
            point.x.len() >= 2
                && point.x[0] >= x_bounds.0
                && point.x[0] <= x_bounds.1
                && point.x[1] >= y_bounds.0
                && point.x[1] <= y_bounds.1
        })
        .collect();

    eprintln!("  Found {} valid points", valid_points.len());

    if valid_points.is_empty() {
        return;
    }

    // Split points into improvements and non-improvements
    let improvements: Vec<&OptimizationPoint> = valid_points
        .iter()
        .filter(|point| point.is_improvement)
        .copied()
        .collect();

    let non_improvements: Vec<&OptimizationPoint> = valid_points
        .iter()
        .filter(|point| !point.is_improvement)
        .copied()
        .collect();

    // Plot all evaluation points (gray) - decimate if more than 1000 points
    if !non_improvements.is_empty() {
        const MAX_POINTS: usize = 1000;
        let step_size = std::cmp::max(1, non_improvements.len() / MAX_POINTS);

        let decimated_points: Vec<&OptimizationPoint> = if non_improvements.len() > MAX_POINTS {
            eprintln!(
                "  Decimating non-improvement points: {} -> {} (step: {})",
                non_improvements.len(),
                non_improvements.len() / step_size,
                step_size
            );
            non_improvements
                .iter()
                .step_by(step_size)
                .copied()
                .collect()
        } else {
            non_improvements
        };

        let x_coords: Vec<f64> = decimated_points.iter().map(|p| p.x[0]).collect();
        let y_coords: Vec<f64> = decimated_points.iter().map(|p| p.x[1]).collect();

        let trace_all = Scatter::new(x_coords, y_coords)
            .mode(Mode::Markers)
            .name("Evaluations")
            .marker(
                Marker::new()
                    .color("rgba(128, 128, 128, 0.6)") // Gray with transparency
                    .size(4)
                    .symbol(plotly::common::MarkerSymbol::Circle),
            );
        plot.add_trace(trace_all);
    }

    // Plot improvement points (bright colors on Viridis-friendly colors) - decimate if more than 1000 points
    if !improvements.is_empty() {
        const MAX_POINTS: usize = 1000;
        let step_size = std::cmp::max(1, improvements.len() / MAX_POINTS);

        let decimated_improvements: Vec<&OptimizationPoint> = if improvements.len() > MAX_POINTS {
            eprintln!(
                "  Decimating improvement points: {} -> {} (step: {})",
                improvements.len(),
                improvements.len() / step_size,
                step_size
            );
            improvements.iter().step_by(step_size).copied().collect()
        } else {
            improvements.clone()
        };

        let x_coords: Vec<f64> = decimated_improvements.iter().map(|p| p.x[0]).collect();
        let y_coords: Vec<f64> = decimated_improvements.iter().map(|p| p.x[1]).collect();

        let trace_improvements = Scatter::new(x_coords, y_coords)
            .mode(Mode::Markers)
            .name("Improvements")
            .marker(
                Marker::new()
                    .color("rgba(255, 255, 0, 0.9)") // Bright yellow - highly visible on Viridis
                    .size(8)
                    .line(
                        plotly::common::Line::new()
                            .color("rgba(255, 140, 0, 1.0)") // Orange border
                            .width(2.0),
                    )
                    .symbol(plotly::common::MarkerSymbol::Circle),
            );
        plot.add_trace(trace_improvements);
    }

    // Plot the optimization path (connecting improvements) - decimate if more than 1000 points
    if improvements.len() > 1 {
        const MAX_PATH_POINTS: usize = 1000;
        let step_size = std::cmp::max(1, improvements.len() / MAX_PATH_POINTS);

        let path_points: Vec<&OptimizationPoint> = if improvements.len() > MAX_PATH_POINTS {
            eprintln!(
                "  Decimating path points: {} -> {} (step: {})",
                improvements.len(),
                improvements.len() / step_size,
                step_size
            );
            improvements.iter().step_by(step_size).copied().collect()
        } else {
            improvements.clone()
        };

        let x_coords: Vec<f64> = path_points.iter().map(|p| p.x[0]).collect();
        let y_coords: Vec<f64> = path_points.iter().map(|p| p.x[1]).collect();

        let path_trace = Scatter::new(x_coords, y_coords)
            .mode(Mode::Lines)
            .name("Optimization Path")
            .line(
                plotly::common::Line::new()
                    .color("rgba(255, 140, 0, 0.8)") // Orange line
                    .width(2.0)
                    .dash(plotly::common::DashType::Dash),
            );
        plot.add_trace(path_trace);
    }

    // Highlight the best point (final solution)
    if let Some(best_point) = improvements.last() {
        let best_trace = Scatter::new(vec![best_point.x[0]], vec![best_point.x[1]])
            .mode(Mode::Markers)
            .name("Best Solution")
            .marker(
                Marker::new()
                    .color("rgba(255, 0, 0, 1.0)") // Bright red - stands out on any background
                    .size(12)
                    .line(
                        plotly::common::Line::new()
                            .color("rgba(255, 255, 255, 1.0)") // White border
                            .width(3.0),
                    )
                    .symbol(plotly::common::MarkerSymbol::Star),
            );
        plot.add_trace(best_trace);
    }
}

/// Create a convergence plot showing loss function over iterations/evaluations
fn plot_convergence(trace: &OptimizationTrace, output_dir: &str, width: usize, height: usize) {
    if trace.points.is_empty() {
        eprintln!("  Warning: No data points for convergence plot");
        return;
    }

    let iterations: Vec<usize> = trace.points.iter().map(|p| p.iteration).collect();
    let best_results: Vec<f64> = trace.points.iter().map(|p| p.best_result).collect();
    let f_values: Vec<f64> = trace.points.iter().map(|p| p.f_value).collect();

    // Create best-so-far trace (shows the convergence of the optimization)
    let best_trace = Scatter::new(iterations.clone(), best_results)
        .mode(Mode::Lines)
        .name("Best So Far")
        .line(
            plotly::common::Line::new()
                .color("rgba(0, 100, 200, 0.8)") // Blue
                .width(3.0),
        );

    // Create function evaluation trace (shows all individual evaluations)
    // Only plot every nth point to avoid overcrowding for large datasets
    let step_size = std::cmp::max(1, trace.points.len() / 1000); // Limit to ~1000 points max
    let sampled_iterations: Vec<usize> = iterations.iter().step_by(step_size).copied().collect();
    let sampled_f_values: Vec<f64> = f_values.iter().step_by(step_size).copied().collect();

    let eval_trace = Scatter::new(sampled_iterations, sampled_f_values)
        .mode(Mode::Markers)
        .name("Function Evaluations")
        .marker(
            Marker::new()
                .color("rgba(200, 200, 200, 0.4)") // Light gray with transparency
                .size(2)
                .symbol(plotly::common::MarkerSymbol::Circle),
        );

    let layout = Layout::new()
        .title(Title::with_text(format!(
            "Convergence: {},",
            trace.function_name
        )))
        .width(width)
        .height(height)
        .x_axis(
            plotly::layout::Axis::new()
                .title(Title::with_text("Iteration/Evaluation"))
                .type_(plotly::layout::AxisType::Linear),
        )
        .y_axis(
            plotly::layout::Axis::new()
                .title(Title::with_text("Function Value"))
                .type_(plotly::layout::AxisType::Log), // Use log scale for better visualization
        )
        .legend(plotly::layout::Legend::new().x(0.7).y(0.9));

    let mut plot = Plot::new();
    plot.add_trace(eval_trace);
    plot.add_trace(best_trace);
    plot.set_layout(layout);

    // Use a clean function name for the convergence plot (remove _block_XXXX suffix if present)
    let clean_name = if trace.function_name.contains("_block_") {
        trace
            .function_name
            .split("_block_")
            .next()
            .unwrap_or(&trace.function_name)
    } else {
        &trace.function_name
    };
    let filename = format!(
        "{}/{}_convergence.html",
        output_dir,
        clean_name.replace(' ', "_")
    );
    plot.write_html(&filename);
    println!("  Created convergence plot: {}", filename);
}

fn main() {
    let args = Args::parse();

    // Set default directories if not provided, using environment-based paths
    let output_dir = args
        .output_dir
        .unwrap_or_else(|| match get_data_generated_dir() {
            Ok(data_dir) => {
                let mut path = data_dir;
                path.push("plot_autoeq_de");
                path.to_string_lossy().to_string()
            }
            Err(e) => {
                eprintln!("Error accessing data directory: {}", e);
                eprintln!(
                    "Please set AUTOEQ_DIR environment variable to your AutoEQ project root."
                );
                std::process::exit(1);
            }
        });

    let csv_dir = args.csv_dir.unwrap_or_else(|| match get_records_dir() {
        Ok(records_dir) => records_dir.to_string_lossy().to_string(),
        Err(e) => {
            eprintln!("Error accessing records directory: {}", e);
            eprintln!("Please set AUTOEQ_DIR environment variable to your AutoEQ project root.");
            std::process::exit(1);
        }
    });

    // Parse bounds
    let x_bounds = parse_bounds(&args.x_bounds).expect("Invalid x_bounds format");
    let y_bounds = parse_bounds(&args.y_bounds).expect("Invalid y_bounds format");

    // Create output directory
    std::fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    // Get all test functions and metadata
    let functions = get_test_functions();
    let metadata = get_function_metadata();

    // Filter functions if specific ones are requested
    let functions_to_plot = if let Some(func_names) = &args.functions {
        let requested: Vec<&str> = func_names.split(',').map(|s| s.trim()).collect();
        functions
            .into_iter()
            .filter(|(name, _)| requested.contains(&name.as_str()))
            .collect()
    } else {
        functions
    };

    println!(
        "Generating interactive HTML with JSON files for {} functions with {}x{} grid",
        functions_to_plot.len(),
        args.xn,
        args.yn
    );

    // Generate JSON file for each function
    let mut processed_functions = BTreeMap::new();

    for (name, func) in functions_to_plot {
        println!("Processing function: {}", name);

        // Check if function requires more than 2D (skip if so)
        if let Some(meta) = metadata.get(&name) {
            if meta.bounds.len() > 2 {
                println!(
                    "  Skipping '{}': requires {}D input, plotting only supports 2D",
                    name,
                    meta.bounds.len()
                );
                continue;
            }
        }

        // Determine bounds to use
        let (plot_x_bounds, plot_y_bounds) = if args.use_metadata {
            if let Some(meta) = metadata.get(&name) {
                // Use metadata bounds if available
                if meta.bounds.len() >= 2 {
                    (meta.bounds[0], meta.bounds[1])
                } else {
                    // Fallback to CLI bounds if metadata doesn't have enough dimensions
                    eprintln!(
                        "  Warning: Function '{}' metadata has insufficient bounds, using CLI bounds",
                        name
                    );
                    (x_bounds, y_bounds)
                }
            } else {
                eprintln!(
                    "  Warning: No metadata found for function '{}', using CLI bounds",
                    name
                );
                (x_bounds, y_bounds)
            }
        } else {
            // Use CLI-provided bounds
            (x_bounds, y_bounds)
        };

        println!(
            "  Using bounds: x=({}, {}), y=({}, {})",
            plot_x_bounds.0, plot_x_bounds.1, plot_y_bounds.0, plot_y_bounds.1
        );

        // Load optimization trace if requested
        let trace = if args.show_traces || args.convergence_plots {
            let csv_files = find_csv_for_function(&csv_dir, &name);
            if !csv_files.is_empty() {
                // Read and combine all block files
                let mut combined_trace = OptimizationTrace {
                    function_name: name.clone(),
                    points: Vec::new(),
                };

                for csv_path in &csv_files {
                    match read_csv_trace(csv_path) {
                        Ok(mut trace) => {
                            combined_trace.points.append(&mut trace.points);
                        }
                        Err(e) => {
                            eprintln!("  Warning: Failed to load trace from {}: {}", csv_path, e);
                        }
                    }
                }

                if !combined_trace.points.is_empty() {
                    println!(
                        "  Loaded optimization trace with {} points from {} file(s)",
                        combined_trace.points.len(),
                        csv_files.len()
                    );
                    Some(combined_trace)
                } else {
                    println!("  No valid trace data found in {} file(s)", csv_files.len());
                    None
                }
            } else {
                println!("  No trace file found for function '{}'", name);
                None
            }
        } else {
            None
        };

        // Create the plot
        let plot = create_plot(
            &name,
            func,
            plot_x_bounds,
            plot_y_bounds,
            args.xn,
            args.yn,
            args.width,
            args.height,
            if args.show_traces {
                trace.as_ref()
            } else {
                None
            },
            metadata.get(&name),
        );

        // Save plot as JSON file
        if let Err(e) = save_plot_as_json(&plot, &output_dir, &name) {
            eprintln!(
                "  Warning: Failed to save JSON for function '{}': {}",
                name, e
            );
        } else {
            println!("  Saved JSON file for function '{}'", name);

            // Add to processed functions for the grouped display
            let first_char = name.chars().next().unwrap_or('_').to_ascii_uppercase();
            let key = if first_char.is_ascii_alphabetic() {
                first_char
            } else {
                '#'
            };
            processed_functions
                .entry(key)
                .or_insert_with(Vec::new)
                .push((name.clone(), func));
        }

        // Create convergence plot if requested and trace is available
        if args.convergence_plots {
            if let Some(ref trace) = trace {
                plot_convergence(trace, &output_dir, args.width, args.height);
            }
        }
    }

    // Generate the interactive HTML file
    generate_interactive_html(&output_dir, &processed_functions);

    println!(
        "Interactive HTML with alphabetical navigation saved to directory: {}",
        output_dir
    );
    println!(
        "Generated {} JSON plot files",
        processed_functions.values().map(|v| v.len()).sum::<usize>()
    );
}

fn parse_bounds(bounds_str: &str) -> Result<(f64, f64), Box<dyn std::error::Error>> {
    // Remove possible surrounding single or double quotes
    let cleaned = bounds_str.trim_matches(|c| c == '\'' || c == '"');

    // Try splitting by comma or whitespace
    let parts: Vec<&str> = if cleaned.contains(',') {
        cleaned.split(',').collect()
    } else {
        cleaned.split_whitespace().collect()
    };

    if parts.len() != 2 {
        return Err("Bounds must be in format 'min,max' or 'min max'".into());
    }

    eprintln!("  bounds {}  {}", parts[0].trim(), parts[1].trim());
    let min = parts[0].trim().parse::<f64>()?;
    let max = parts[1].trim().parse::<f64>()?;
    Ok((min, max))
}

fn create_plot(
    name: &str,
    func: TestFunction,
    x_bounds: (f64, f64),
    y_bounds: (f64, f64),
    xn: usize,
    yn: usize,
    width: usize,
    height: usize,
    trace: Option<&OptimizationTrace>,
    metadata: Option<&FunctionMetadata>,
) -> Plot {
    // Create coordinate grids
    let x_vals: Vec<f64> = (0..xn)
        .map(|i| x_bounds.0 + (x_bounds.1 - x_bounds.0) * i as f64 / (xn - 1) as f64)
        .collect();

    let y_vals: Vec<f64> = (0..yn)
        .map(|i| y_bounds.0 + (y_bounds.1 - y_bounds.0) * i as f64 / (yn - 1) as f64)
        .collect();

    // Evaluate function on grid
    let mut z_vals = Vec::with_capacity(yn);
    for &y in &y_vals {
        let mut row = Vec::with_capacity(xn);
        for &x in &x_vals {
            let input = Array1::from(vec![x, y]);
            let z = func(&input);
            row.push(z);
        }
        z_vals.push(row);
    }

    // Create contour plot with custom colorbar configuration
    let contour = Contour::new(x_vals.clone(), y_vals.clone(), z_vals.clone())
        .color_scale(ColorScale::Palette(ColorScalePalette::Viridis))
        .color_bar(
            plotly::common::ColorBar::new()
                .len_mode(plotly::common::ThicknessMode::Pixels)
                .len(60 * height / 100)
                .y_anchor(plotly::common::Anchor::Bottom)
                .y(0.0),
        );

    // Create layout
    let layout = Layout::new()
        .title(Title::with_text(format!("Function: {}", name)))
        .width(width)
        .height(height)
        .x_axis(plotly::layout::Axis::new().title(Title::with_text("X")))
        .y_axis(plotly::layout::Axis::new().title(Title::with_text("Y")));

    // Create plot and add contour
    let mut plot = Plot::new();
    plot.add_trace(contour);

    // Add optimization trace if available
    if let Some(trace) = trace {
        add_optimization_trace(&mut plot, trace, x_bounds, y_bounds);
    }

    // Add global minima if metadata is available
    if let Some(meta) = metadata {
        add_global_minima(&mut plot, meta, x_bounds, y_bounds);

        // Add constraint boundaries if present
        if !meta.inequality_constraints.is_empty() {
            add_constraint_boundaries(&mut plot, meta, x_bounds, y_bounds, &x_vals, &y_vals);
        }
    }

    plot.set_layout(layout);
    plot
}

/// Save a plot to a JSON file
fn save_plot_as_json(
    plot: &Plot,
    output_dir: &str,
    function_name: &str,
) -> Result<(), std::io::Error> {
    let json_path = format!("{}/{}.json", output_dir, function_name.replace(' ', "_"));
    let plot_json = plot.to_json();

    let mut file = File::create(&json_path)?;
    file.write_all(plot_json.as_bytes())?;

    Ok(())
}

/// Generate interactive HTML with alphabetical navigation
fn generate_interactive_html(
    output_dir: &str,
    grouped_functions: &BTreeMap<char, Vec<(String, TestFunction)>>,
) {
    let file_path = format!("{}/interactive_plots.html", output_dir);
    let mut file = File::create(&file_path).expect("Failed to create HTML file");

    // Generate letter navigation buttons
    let letter_buttons: String = grouped_functions
        .keys()
        .map(|letter| {
            format!(
                "<button class=\"letter-btn\" onclick=\"showLetter('{}')\">{}</button>",
                letter, letter
            )
        })
        .collect::<Vec<String>>()
        .join("");

    // Generate function lists for each letter
    let mut function_lists = String::new();
    for (letter, functions) in grouped_functions {
        function_lists.push_str(&format!(
            "<div id=\"functions-{}\" class=\"function-list\" style=\"display: none;\">\n",
            letter
        ));

        for (name, _) in functions {
            function_lists.push_str(&format!(
                "  <div class=\"function-item\" onclick=\"loadFunction('{}')\">{}</div>\n",
                name.replace(' ', "_"),
                name
            ));
        }

        function_lists.push_str("</div>\n");
    }

    let html_content = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>Interactive AutoEQ DE Optimization Plots</title>
    <script src="https://cdn.plot.ly/plotly-3.1.0.min.js"></script>
    <style>
        body {{
            font-family: 'Segoe UI', Tahoma, Geneva, Verdana, sans-serif;
            margin: 0;
            padding: 20px;
            background-color: #f5f5f5;
        }}
        .container {{
            max-width: 1200px;
            margin: 0 auto;
            background-color: white;
            border-radius: 8px;
            box-shadow: 0 2px 10px rgba(0,0,0,0.1);
            overflow: hidden;
        }}
        .header {{
            background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
            color: white;
            padding: 20px;
            text-align: center;
        }}
        .navigation {{
            display: flex;
            justify-content: space-between;
            padding: 20px;
            background-color: #fafafa;
            border-bottom: 1px solid #eee;
        }}
        .letter-navigation {{
            flex: 1;
        }}
        .letter-btn {{
            padding: 8px 12px;
            margin: 2px;
            border: none;
            background-color: #e9ecef;
            color: #495057;
            cursor: pointer;
            border-radius: 4px;
            transition: all 0.3s ease;
        }}
        .letter-btn:hover, .letter-btn.active {{
            background-color: #007bff;
            color: white;
        }}
        .function-panel {{
            width: 300px;
            padding: 20px;
            border-right: 1px solid #eee;
        }}
        .function-list {{
            max-height: 400px;
            overflow-y: auto;
        }}
        .function-item {{
            padding: 10px;
            margin: 5px 0;
            background-color: #f8f9fa;
            border-radius: 4px;
            cursor: pointer;
            transition: all 0.3s ease;
        }}
        .function-item:hover {{
            background-color: #e9ecef;
        }}
        .function-item.active {{
            background-color: #007bff;
            color: white;
        }}
        .main-content {{
            display: flex;
        }}
        .plot-container {{
            flex: 1;
            padding: 20px;
            min-height: 600px;
        }}
        .loading {{
            text-align: center;
            padding: 50px;
            color: #6c757d;
        }}
        .error {{
            text-align: center;
            padding: 50px;
            color: #dc3545;
        }}
    </style>
</head>
<body>
    <div class="container">
        <div class="header">
            <h1>Interactive AutoEQ DE Optimization Plots</h1>
            <p>Click on a letter to browse functions, then click on a function name to view its plot</p>
        </div>

        <div class="navigation">
            <div class="letter-navigation">
                {letter_buttons}
            </div>
        </div>

        <div class="main-content">
            <div class="function-panel">
                {function_lists}
            </div>

            <div class="plot-container">
                <div id="plot-display" class="loading">
                    <h3>Select a function to view its plot</h3>
                    <p>Choose a letter from the navigation above, then click on a function name.</p>
                </div>
            </div>
        </div>
    </div>

    <script>
        let currentLetter = null;
        let currentFunction = null;

        function showLetter(letter) {{
            /* Hide all function lists */
            const allLists = document.querySelectorAll('.function-list');
            allLists.forEach(list => list.style.display = 'none');

            /* Show the selected letter's function list */
            const targetList = document.getElementById('functions-' + letter);
            if (targetList) {{
                targetList.style.display = 'block';
            }}

            /* Update button states */
            document.querySelectorAll('.letter-btn').forEach(btn => btn.classList.remove('active'));
            event.target.classList.add('active');

            /* Clear the plot when changing letters */
            const plotDisplay = document.getElementById('plot-display');
            plotDisplay.innerHTML = '<div class="loading"><h3>Select a function to view its plot</h3><p>Choose a function from the list below.</p></div>';

            /* Clear function item selection states */
            document.querySelectorAll('.function-item').forEach(item => item.classList.remove('active'));

            currentLetter = letter;
            currentFunction = null;
        }}

        async function loadFunction(functionName) {{
            const plotDisplay = document.getElementById('plot-display');

            /* Show loading state */
            plotDisplay.innerHTML = '<div class="loading"><h3>Loading plot...</h3></div>';

            /* Update function item states */
            document.querySelectorAll('.function-item').forEach(item => item.classList.remove('active'));
            event.target.classList.add('active');

            try {{
                /* Load the JSON file for this function */
                const response = await fetch(functionName + '.json');

                if (!response.ok) {{
                    throw new Error('Failed to load plot data: ' + response.statusText);
                }}

                const plotData = await response.text();
                const plot = JSON.parse(plotData);

                /* Clear the plot display and render the new plot */
                plotDisplay.innerHTML = '';
                await Plotly.newPlot('plot-display', plot.data, plot.layout);

                currentFunction = functionName;

            }} catch (error) {{
                console.error('Error loading function plot:', error);
                plotDisplay.innerHTML = '<div class="error"><h3>Error Loading Plot</h3><p>Failed to load plot for function: ' + functionName.replace('_', ' ') + '</p><p>Error: ' + error.message + '</p></div>';
            }}
        }}

        /* Initialize by showing the first letter */
        const firstLetter = document.querySelector('.letter-btn');
        if (firstLetter) {{
            firstLetter.click();
        }}
    </script>
</body>
</html>
"#,
        letter_buttons = letter_buttons,
        function_lists = function_lists
    );

    file.write_all(html_content.as_bytes())
        .expect("Failed to write to HTML file");
}

/// Automatically get all test functions using the shared registry
fn get_test_functions() -> Vec<(String, TestFunction)> {
    let registry = autoeq_de::function_registry::FunctionRegistry::new();
    let metadata = get_function_metadata();
    let mut functions = Vec::new();

    // Build function list from registry and metadata
    for (name, _meta) in metadata.iter() {
        if let Some(func) = registry.get(name) {
            functions.push((name.clone(), func));
        } else {
            eprintln!(
                "Warning: Function '{}' found in metadata but not in registry",
                name
            );
        }
    }

    // Sort functions alphabetically for consistent ordering
    functions.sort_by(|a, b| a.0.cmp(&b.0));

    eprintln!(
        "Discovered {} plottable functions from registry",
        functions.len()
    );
    functions
}

// Function mapping now handled by shared FunctionRegistry

/// Add global minima markers to the plot
fn add_global_minima(
    plot: &mut Plot,
    metadata: &FunctionMetadata,
    x_bounds: (f64, f64),
    y_bounds: (f64, f64),
) {
    let valid_minima: Vec<&(Vec<f64>, f64)> = metadata
        .global_minima
        .iter()
        .filter(|(coords, _)| {
            coords.len() >= 2
                && coords[0] >= x_bounds.0
                && coords[0] <= x_bounds.1
                && coords[1] >= y_bounds.0
                && coords[1] <= y_bounds.1
        })
        .collect();

    if !valid_minima.is_empty() {
        let x_coords: Vec<f64> = valid_minima.iter().map(|(coords, _)| coords[0]).collect();
        let y_coords: Vec<f64> = valid_minima.iter().map(|(coords, _)| coords[1]).collect();

        let global_minima_trace = Scatter::new(x_coords, y_coords)
            .mode(Mode::Markers)
            .name("Global Minima")
            .marker(
                Marker::new()
                    .color("rgba(255, 255, 255, 1.0)") // White center
                    .size(10)
                    .line(
                        plotly::common::Line::new()
                            .color("rgba(255, 0, 255, 1.0)") // Magenta border
                            .width(3.0),
                    )
                    .symbol(plotly::common::MarkerSymbol::Diamond),
            );
        plot.add_trace(global_minima_trace);
    }
}

/// Add constraint boundary visualization to the plot
fn add_constraint_boundaries(
    plot: &mut Plot,
    metadata: &FunctionMetadata,
    _x_bounds: (f64, f64),
    _y_bounds: (f64, f64),
    x_vals: &[f64],
    y_vals: &[f64],
) {
    // Create a contour for each constraint showing feasible/infeasible regions
    for (i, constraint_fn) in metadata.inequality_constraints.iter().enumerate() {
        let mut constraint_vals = Vec::with_capacity(y_vals.len());

        for &y in y_vals {
            let mut row = Vec::with_capacity(x_vals.len());
            for &x in x_vals {
                let input = Array1::from(vec![x, y]);
                let constraint_value = constraint_fn(&input);
                row.push(constraint_value);
            }
            constraint_vals.push(row);
        }

        // Add contour line at constraint_value = 0 (boundary)
        let constraint_contour = Contour::new(x_vals.to_vec(), y_vals.to_vec(), constraint_vals)
            .show_scale(false) // Don't show colorbar for constraints
            .contours(
                plotly::contour::Contours::new()
                    .start(0.0)
                    .end(0.0)
                    .size(1.0), // Only show the boundary line
            )
            .line(
                plotly::common::Line::new()
                    .color("rgba(255, 0, 0, 0.8)".to_string()) // Red constraint boundary
                    .width(3.0)
                    .dash(plotly::common::DashType::Dash),
            )
            .name(&format!("Constraint {}", i + 1))
            .hover_info(plotly::common::HoverInfo::Skip); // Don't show hover info for constraints

        plot.add_trace(constraint_contour);
    }
}
