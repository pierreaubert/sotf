use gpui::*;

pub fn render(_app: &ShowcaseApp) -> Div {
    div()
        .flex()
        .flex_col()
        .gap_6()
        .child(
            div()
                .text_2xl()
                .font_weight(FontWeight::BOLD)
                .child("Welcome to d3rs"),
        )
        .child(
            div()
                .text_base()
                .text_color(rgb(0x666666))
                .max_w(px(600.0))
                .child("d3rs is a D3.js-inspired plotting library for GPUI. It brings familiar D3 concepts like scales, axes, and shape generators to Rust applications built with GPUI."),
        )
        .child(
            div()
                .mt_4()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .child("Demos"),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .ml_4()
                .child(feature_item("Scales", "Linear, logarithmic, ordinal, and band scales"))
                .child(feature_item("Axes", "Customizable axes with tick formatting"))
                .child(feature_item("Bar Charts", "Simple and grouped bar charts"))
                .child(feature_item("Line Charts", "Line charts with points and curves"))
                .child(feature_item("Scatter Plots", "Scatter plots with symbols"))
                .child(feature_item("Surface Plots", "2D surface and heatmap visualizations"))
                .child(feature_item("QuadTree", "Spatial indexing and nearest neighbor search"))
                .child(feature_item("Contours", "Contour lines and density estimation"))
                .child(feature_item("Transitions", "Animated transitions and easing"))
                .child(feature_item("Geo", "Geographic projections and maps"))
                .child(feature_item("Colors", "Color schemes and interpolation")),
        )
        .child(
            div()
                .mt_4()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .child("D3 Observable Examples"),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .ml_4()
                .child(feature_item("Volcano", "Volcano contour visualization"))
                .child(feature_item("KDE", "Kernel density estimation"))
                .child(feature_item("Treemap", "Hierarchical treemap layout"))
                .child(feature_item("Stacked Bars", "Stacked and grouped bar charts"))
                .child(feature_item("Versor Dragging", "Orthographic globe with projection switching"))
                .child(feature_item("Histogram", "Binned value distribution"))
                .child(feature_item("Revenue Stream", "Music industry stacked area trends"))
                .child(feature_item("Horizon Chart", "Multi-band realtime horizon chart"))
                .child(feature_item("Choropleth", "World map with colored regions"))
                .child(feature_item("Sankey", "Energy flow Sankey diagram"))
                .child(feature_item("Calendar", "Calendar heatmap (52x7 grid)"))
                .child(feature_item("Radial Line", "Polar coordinate temperature chart"))
                .child(feature_item("Parallel Coord", "Multi-axis car dataset comparison")),
        )
        .child(
            div()
                .mt_4()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .child("Observable Examples (Golden-Tested)"),
        )
        .child(
            div()
                .text_sm()
                .text_color(rgb(0x888888))
                .ml_4()
                .mb_2()
                .child("These use d3rs::examples::* compute modules — validated against D3.js golden files."),
        )
        .child(
            div()
                .flex()
                .flex_col()
                .gap_2()
                .ml_4()
                .child(feature_item("Hexbin", "Log scales + hexagonal binning (diamonds data)"))
                .child(feature_item("Pie Chart", "Pie layout with arc path generation"))
                .child(feature_item("Donut Chart", "Pie with inner radius and pad angle"))
                .child(feature_item("Line Chart", "7 curve types (linear, basis, cardinal, etc.)"))
                .child(feature_item("Streamgraph", "Stack with Wiggle offset + InsideOut order"))
                .child(feature_item("Stacked Bar", "BandScale + Stack with diverging offset"))
                .child(feature_item("Stacked Area", "Stack + area paths with monotoneX curve"))
                .child(feature_item("Box Plot", "Quartiles, whiskers, outliers + BandScale"))
                .child(feature_item("Chord Diagram", "Chord layout from adjacency matrix"))
                .child(feature_item("Force Directed", "Force simulation with charge + center")),
        )
        .child(
            div()
                .mt_6()
                .p_4()
                .bg(rgb(0xf5f5f5))
                .rounded_md()
                .child(
                    div()
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .child("Use the sidebar to explore different demos"),
                ),
        )
}

pub fn feature_item(title: &str, desc: &str) -> Div {
    div()
        .flex()
        .gap_2()
        .child(
            div()
                .text_color(rgb(0x007acc))
                .font_weight(FontWeight::SEMIBOLD)
                .child(format!("{title}:")),
        )
        .child(div().text_color(rgb(0x666666)).child(desc.to_string()))
}

use super::ShowcaseApp;
