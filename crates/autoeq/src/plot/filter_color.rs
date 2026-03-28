/// Get a color from the Plotly qualitative color palette
///
/// # Arguments
/// * `index` - Index of the color to retrieve (cycles through 10 colors)
///
/// # Returns
/// * Hex color code as a static string
///
/// # Details
/// Uses a predefined set of 10 colors from Plotly's qualitative palette.
/// Cycles through the colors when index exceeds the palette size.
pub fn filter_color(index: usize) -> &'static str {
    // Plotly qualitative color palette (10 colors)
    // Matches expectations in tests: index 0 -> #1f77b4, index 3 -> #d62728, index 9 -> #17becf
    const COLORS: [&str; 10] = [
        "#1f77b4", "#ff7f0e", "#2ca02c", "#d62728", "#9467bd", "#8c564b", "#e377c2", "#7f7f7f",
        "#bcbd22", "#17becf",
    ];
    COLORS[index % COLORS.len()]
}

#[cfg(test)]
mod tests {
    use super::filter_color;

    #[test]
    fn color_palette_cycles() {
        assert_eq!(filter_color(0), "#1f77b4");
        assert_eq!(filter_color(3), "#d62728");
        assert_eq!(filter_color(9), "#17becf");
        // Cycle wraps around
        assert_eq!(filter_color(10), "#1f77b4");
        assert_eq!(filter_color(13), "#d62728");
    }
}
