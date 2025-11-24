use clap::Parser;
use ndarray::Array1;
use plotly::common::{ColorScale, ColorScalePalette, Marker, Mode, Title};
use plotly::contour::Contour;
use plotly::{Layout, Plot, Scatter};
use std::collections::BTreeMap;
use std::fs::File;
use std::io::Write;

// Import environment utilities
use autoeq_env::get_data_generated_dir;

// Import the test functions and metadata
use autoeq_testfunctions::{FunctionMetadata, functions, get_function_metadata};

type TestFunction = fn(&Array1<f64>) -> f64;

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

    /// Use function metadata for bounds (overrides x_bounds and y_bounds)
    #[arg(long, default_value = "true")]
    use_metadata: bool,
}

fn main() {
    let args = Args::parse();

    // Set default directories if not provided, using environment-based paths
    let output_dir = args
        .output_dir
        .unwrap_or_else(|| match get_data_generated_dir() {
            Ok(data_dir) => {
                let functions_dir = data_dir.join("functions");
                functions_dir.to_string_lossy().to_string()
            }
            Err(e) => {
                eprintln!("Error accessing data directory: {}", e);
                eprintln!(
                    "Please set AUTOEQ_DIR environment variable to your AutoEQ project root."
                );
                std::process::exit(1);
            }
        });

    // Parse bounds
    let x_bounds = parse_bounds(&args.x_bounds).expect("Invalid x_bounds format");
    let y_bounds = parse_bounds(&args.y_bounds).expect("Invalid y_bounds format");

    // Create output directory
    std::fs::create_dir_all(&output_dir).expect("Failed to create output directory");

    // Get all test functions and metadata
    let functions = get_all_functions();
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

    // Group functions by letter for navigation
    let _grouped_functions = group_functions_by_letter(&functions_to_plot);

    // Generate JSON file for each function
    let mut processed_functions = BTreeMap::new();
    for (name, func) in functions_to_plot {
        println!("Processing function: {}", name);

        // First check: Skip known multidimensional functions by name
        if name.contains("3d")
            || name.contains("4d")
            || name.contains("6d")
            || name.contains("hartman")
        {
            println!(
                "  Skipping '{}': likely requires > 2D input, plotting only supports 2D",
                name
            );
            continue;
        }

        // Second check: Skip based on metadata bounds
        if let Some(meta) = metadata.get(&name) {
            if meta.bounds.len() > 2 {
                println!(
                    "  Skipping '{}': requires {}D input, plotting only supports 2D",
                    name,
                    meta.bounds.len()
                );
                continue;
            }
            // Skip functions with no metadata (likely problematic)
            if meta.description.contains("no metadata found") {
                println!(
                    "  Skipping '{}': no metadata available, may require > 2D input",
                    name
                );
                continue;
            }
        }

        let (plot_x_bounds, plot_y_bounds) = if args.use_metadata {
            if let Some(meta) = metadata.get(&name) {
                if meta.bounds.len() >= 2 {
                    (meta.bounds[0], meta.bounds[1])
                } else {
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
            (x_bounds, y_bounds)
        };

        println!(
            "  Using bounds: x=({}, {}), y=({}, {})",
            plot_x_bounds.0, plot_x_bounds.1, plot_y_bounds.0, plot_y_bounds.1
        );

        let plot = create_plot(
            &name,
            func,
            plot_x_bounds,
            plot_y_bounds,
            args.xn,
            args.yn,
            args.width,
            args.height,
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
    let cleaned = bounds_str.trim_matches(|c| c == '\'' || c == '"');
    let parts: Vec<&str> = if cleaned.contains(',') {
        cleaned.split(',').collect()
    } else {
        cleaned.split_whitespace().collect()
    };

    if parts.len() != 2 {
        return Err("Bounds must be in format 'min,max' or 'min max'".into());
    }

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
    metadata: Option<&FunctionMetadata>,
) -> Plot {
    let x_vals: Vec<f64> = (0..xn)
        .map(|i| x_bounds.0 + (x_bounds.1 - x_bounds.0) * i as f64 / (xn - 1) as f64)
        .collect();

    let y_vals: Vec<f64> = (0..yn)
        .map(|i| y_bounds.0 + (y_bounds.1 - y_bounds.0) * i as f64 / (yn - 1) as f64)
        .collect();

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

    let contour = Contour::new(x_vals.clone(), y_vals.clone(), z_vals.clone())
        .color_scale(ColorScale::Palette(ColorScalePalette::Viridis))
        .color_bar(
            plotly::common::ColorBar::new()
                .len_mode(plotly::common::ThicknessMode::Pixels)
                .len(60 * height / 100)
                .y_anchor(plotly::common::Anchor::Bottom)
                .y(0.0),
        );

    let layout = Layout::new()
        .title(Title::with_text(format!("Function: {}", name)))
        .width(width)
        .height(height)
        .x_axis(plotly::layout::Axis::new().title(Title::with_text("X")))
        .y_axis(plotly::layout::Axis::new().title(Title::with_text("Y")));

    let mut plot = Plot::new();
    plot.add_trace(contour);

    if let Some(meta) = metadata {
        add_global_minima(&mut plot, meta, x_bounds, y_bounds);
        if !meta.inequality_constraints.is_empty() {
            add_constraint_boundaries(&mut plot, meta, &x_vals, &y_vals);
        }
    }

    plot.set_layout(layout);
    plot
}

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
                    .color("rgba(255, 255, 255, 1.0)")
                    .size(10)
                    .line(
                        plotly::common::Line::new()
                            .color("rgba(255, 0, 255, 1.0)")
                            .width(3.0),
                    )
                    .symbol(plotly::common::MarkerSymbol::Diamond),
            );
        plot.add_trace(global_minima_trace);
    }
}

fn add_constraint_boundaries(
    plot: &mut Plot,
    metadata: &FunctionMetadata,
    x_vals: &[f64],
    y_vals: &[f64],
) {
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

        let constraint_contour = Contour::new(x_vals.to_vec(), y_vals.to_vec(), constraint_vals)
            .show_scale(false)
            .contours(
                plotly::contour::Contours::new()
                    .start(0.0)
                    .end(0.0)
                    .size(1.0),
            )
            .line(
                plotly::common::Line::new()
                    .color("rgba(255, 0, 0, 0.8)".to_string())
                    .width(3.0)
                    .dash(plotly::common::DashType::Dash),
            )
            .name(&format!("Constraint {}", i + 1))
            .hover_info(plotly::common::HoverInfo::Skip);

        plot.add_trace(constraint_contour);
    }
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
    <title>Interactive Test Function Plots</title>
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
            <h1>Interactive Test Function Plots</h1>
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

/// Group test functions by their first letter for alphabetical navigation
fn group_functions_by_letter(
    functions: &[(String, TestFunction)],
) -> BTreeMap<char, Vec<(String, TestFunction)>> {
    let mut groups = BTreeMap::new();

    for (name, func) in functions {
        let first_char = name.chars().next().unwrap_or('_').to_ascii_uppercase();

        // Handle non-alphabetic characters
        let key = if first_char.is_ascii_alphabetic() {
            first_char
        } else {
            '#' // Use '#' for functions starting with non-alphabetic characters
        };

        groups
            .entry(key)
            .or_insert_with(Vec::new)
            .push((name.clone(), *func));
    }

    // Sort functions within each group
    for group in groups.values_mut() {
        group.sort_by(|a, b| a.0.cmp(&b.0));
    }

    groups
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

fn get_all_functions() -> Vec<(String, TestFunction)> {
    vec![
        ("ackley".to_string(), functions::ackley),
        ("ackley_n2".to_string(), functions::ackley_n2),
        ("ackley_n3".to_string(), functions::ackley_n3),
        ("alpine_n1".to_string(), functions::alpine_n1),
        ("alpine_n2".to_string(), functions::alpine_n2),
        ("beale".to_string(), functions::beale),
        ("bent_cigar".to_string(), functions::bent_cigar),
        ("bent_cigar_alt".to_string(), functions::bent_cigar_alt),
        (
            "binh_korn_constraint1".to_string(),
            functions::binh_korn_constraint1,
        ),
        (
            "binh_korn_constraint2".to_string(),
            functions::binh_korn_constraint2,
        ),
        (
            "binh_korn_weighted".to_string(),
            functions::binh_korn_weighted,
        ),
        ("bird".to_string(), functions::bird),
        ("bohachevsky1".to_string(), functions::bohachevsky1),
        ("bohachevsky2".to_string(), functions::bohachevsky2),
        ("bohachevsky3".to_string(), functions::bohachevsky3),
        ("booth".to_string(), functions::booth),
        ("branin".to_string(), functions::branin),
        ("brown".to_string(), functions::brown),
        ("bukin_n6".to_string(), functions::bukin_n6),
        ("chung_reynolds".to_string(), functions::chung_reynolds),
        ("cigar".to_string(), functions::cigar),
        ("colville".to_string(), functions::colville),
        ("cosine_mixture".to_string(), functions::cosine_mixture),
        ("cross_in_tray".to_string(), functions::cross_in_tray),
        ("de_jong_step2".to_string(), functions::de_jong_step2),
        (
            "dejong_f5_foxholes".to_string(),
            functions::dejong_f5_foxholes,
        ),
        ("different_powers".to_string(), functions::different_powers),
        ("discus".to_string(), functions::discus),
        ("dixons_price".to_string(), functions::dixons_price),
        ("drop_wave".to_string(), functions::drop_wave),
        ("easom".to_string(), functions::easom),
        ("eggholder".to_string(), functions::eggholder),
        ("elliptic".to_string(), functions::elliptic),
        (
            "epistatic_michalewicz".to_string(),
            functions::epistatic_michalewicz,
        ),
        (
            "expanded_griewank_rosenbrock".to_string(),
            functions::expanded_griewank_rosenbrock,
        ),
        ("exponential".to_string(), functions::exponential),
        ("forrester_2008".to_string(), functions::forrester_2008),
        (
            "freudenstein_roth".to_string(),
            functions::freudenstein_roth,
        ),
        ("goldstein_price".to_string(), functions::goldstein_price),
        ("gramacy_lee_2012".to_string(), functions::gramacy_lee_2012),
        (
            "gramacy_lee_function".to_string(),
            functions::gramacy_lee_function,
        ),
        ("griewank".to_string(), functions::griewank),
        ("griewank2".to_string(), functions::griewank2),
        ("happy_cat".to_string(), functions::happy_cat),
        ("happycat".to_string(), functions::happycat),
        ("hartman_3d".to_string(), functions::hartman_3d),
        ("hartman_4d".to_string(), functions::hartman_4d),
        ("hartman_6d".to_string(), functions::hartman_6d),
        ("himmelblau".to_string(), functions::himmelblau),
        ("holder_table".to_string(), functions::holder_table),
        ("katsuura".to_string(), functions::katsuura),
        (
            "keanes_bump_constraint1".to_string(),
            functions::keanes_bump_constraint1,
        ),
        (
            "keanes_bump_constraint2".to_string(),
            functions::keanes_bump_constraint2,
        ),
        (
            "keanes_bump_objective".to_string(),
            functions::keanes_bump_objective,
        ),
        (
            "lampinen_simplified".to_string(),
            functions::lampinen_simplified,
        ),
        ("langermann".to_string(), functions::langermann),
        ("levi13".to_string(), functions::levi13),
        ("levy".to_string(), functions::levy),
        ("levy_n13".to_string(), functions::levy_n13),
        ("matyas".to_string(), functions::matyas),
        ("mccormick".to_string(), functions::mccormick),
        ("michalewicz".to_string(), functions::michalewicz),
        (
            "mishras_bird_constraint".to_string(),
            functions::mishras_bird_constraint,
        ),
        (
            "mishras_bird_objective".to_string(),
            functions::mishras_bird_objective,
        ),
        ("periodic".to_string(), functions::periodic),
        ("perm_0_d_beta".to_string(), functions::perm_0_d_beta),
        ("perm_d_beta".to_string(), functions::perm_d_beta),
        ("pinter".to_string(), functions::pinter),
        ("powell".to_string(), functions::powell),
        ("power_sum".to_string(), functions::power_sum),
        ("qing".to_string(), functions::qing),
        ("quadratic".to_string(), functions::quadratic),
        ("quartic".to_string(), functions::quartic),
        ("rastrigin".to_string(), functions::rastrigin),
        ("ridge".to_string(), functions::ridge),
        ("rosenbrock".to_string(), functions::rosenbrock),
        (
            "rosenbrock_disk_constraint".to_string(),
            functions::rosenbrock_disk_constraint,
        ),
        (
            "rosenbrock_objective".to_string(),
            functions::rosenbrock_objective,
        ),
        (
            "rotated_hyper_ellipsoid".to_string(),
            functions::rotated_hyper_ellipsoid,
        ),
        ("salomon".to_string(), functions::salomon),
        (
            "salomon_corrected".to_string(),
            functions::salomon_corrected,
        ),
        ("schaffer_n2".to_string(), functions::schaffer_n2),
        ("schaffer_n4".to_string(), functions::schaffer_n4),
        ("schwefel".to_string(), functions::schwefel),
        ("schwefel2".to_string(), functions::schwefel2),
        ("sharp_ridge".to_string(), functions::sharp_ridge),
        ("shekel".to_string(), functions::shekel),
        ("shubert".to_string(), functions::shubert),
        ("six_hump_camel".to_string(), functions::six_hump_camel),
        ("sphere".to_string(), functions::sphere),
        ("step".to_string(), functions::step),
        ("styblinski_tang2".to_string(), functions::styblinski_tang2),
        (
            "sum_of_different_powers".to_string(),
            functions::sum_of_different_powers,
        ),
        ("sum_squares".to_string(), functions::sum_squares),
        ("tablet".to_string(), functions::tablet),
        ("three_hump_camel".to_string(), functions::three_hump_camel),
        ("trid".to_string(), functions::trid),
        ("vincent".to_string(), functions::vincent),
        ("whitley".to_string(), functions::whitley),
        ("xin_she_yang_n1".to_string(), functions::xin_she_yang_n1),
        ("xin_she_yang_n2".to_string(), functions::xin_she_yang_n2),
        ("xin_she_yang_n3".to_string(), functions::xin_she_yang_n3),
        ("xin_she_yang_n4".to_string(), functions::xin_she_yang_n4),
        ("zakharov".to_string(), functions::zakharov),
        ("zakharov2".to_string(), functions::zakharov2),
    ]
}
