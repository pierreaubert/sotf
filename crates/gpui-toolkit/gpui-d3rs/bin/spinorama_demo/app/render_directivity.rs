impl SpinoramaApp {
    fn render_directivity_plot(&mut self, plane: &str, cx: &mut Context<Self>) -> Div {
        let theme = cx.theme();
        let ds = cx.design();
        // Create a viridis-like color palette for directivity
        let viridis_colors = vec![
            D3Color::from_hex(0x440154), // Dark purple
            D3Color::from_hex(0x414487), // Purple-blue
            D3Color::from_hex(0x2a788e), // Teal
            D3Color::from_hex(0x22a884), // Green-teal
            D3Color::from_hex(0x7ad151), // Light green
            D3Color::from_hex(0xfde725), // Yellow
        ];

        let s = self.font_scale();
        let Some(ref directivity) = self.directivity_data else {
            return div().flex().items_center().justify_center().h_full().child(
                div()
                    .text_size(px(ds.typography.base_size * s))
                    .text_color(theme.text_secondary)
                    .child("No directivity data available for this speaker."),
            );
        };

        let curves = if plane == "horizontal" {
            &directivity.horizontal
        } else {
            &directivity.vertical
        };

        if curves.is_empty() {
            return div().flex().items_center().justify_center().h_full().child(
                div()
                    .text_size(px(ds.typography.base_size * s))
                    .text_color(theme.text_secondary)
                    .child(format!("No {} directivity data available.", plane)),
            );
        }

        let chart_width = self.content_width;
        let chart_height = (chart_width * 0.5).min(self.content_height * 0.6);

        // Generate colors for different angles and build PlotCurve list
        let num_curves = curves.len();
        let plot_curves: Vec<PlotCurve> = curves
            .iter()
            .enumerate()
            .map(|(i, curve)| {
                let t = i as f32 / (num_curves.max(1) - 1).max(1) as f32;
                let color = d3rs::color::interpolate_colors(&viridis_colors, t);

                let points: Vec<LinePoint> = curve
                    .freq
                    .iter()
                    .zip(curve.spl.iter())
                    .filter(|&(&f, _)| (20.0..=20000.0).contains(&f))
                    .map(|(&f, &spl)| LinePoint::new(f, spl))
                    .collect();

                PlotCurve::new(points, color).stroke_width(1.5)
            })
            .collect();

        // Get angle range for legend
        let angle_min = curves.first().map(|c| c.angle).unwrap_or(-60.0);
        let angle_max = curves.last().map(|c| c.angle).unwrap_or(60.0);

        // Create the chart
        let chart = render_freq_spl_plot(
            plot_curves,
            self.freq_spl_zoom.x_domain(),
            self.freq_spl_zoom.y_domain(),
            None, // No secondary axis for directivity plots
            chart_width,
            chart_height,
            self.freq_spl_brush
                .current_selection()
                .map(|sel| BrushOverlay { selection: sel }),
            &theme,
        );

        // Wrap with interactive handlers
        let interactive_chart = self.wrap_freq_spl_chart_interactive(
            chart,
            ChartId::FreqSpl,
            chart_width,
            chart_height,
            cx,
        );

        div()
            .flex()
            .flex_col()
            .gap(px(ds.spacing.section_gap * 1.5))
            .child(
                div()
                    .text_size(px(ds.typography.large_size * s))
                    .font_weight(FontWeight::BOLD)
                    .text_color(theme.text_primary)
                    .child(format!(
                        "{} SPL - {}",
                        if plane == "horizontal" {
                            "Horizontal"
                        } else {
                            "Vertical"
                        },
                        self.selected_speaker.as_deref().unwrap_or("Unknown")
                    )),
            )
            .child(interactive_chart)
            // Zoom status indicator
            .when(self.freq_spl_zoom.is_zoomed(), |el| {
                el.child(
                    div()
                        .text_size(px(ds.typography.small_size * s))
                        .text_color(theme.text_secondary)
                        .child("Zoomed (double-click to reset)"),
                )
            })
            // Angle legend
            .child({
                let font_config = GlyphTextConfig::horizontal((10.0 * s).round(), Hsla::from(theme.text_primary));

                div()
                    .flex()
                    .items_center()
                    .gap(px(ds.spacing.control_gap))
                    .p(px(ds.spacing.card_padding))
                    .bg(theme.muted)
                    .rounded(px(ds.corners.md))
                    .child(render_glyph_text(
                        &format!("{:.0}°", angle_min),
                        &font_config,
                    ))
                    // Simplified gradient legend (using color strip segments)
                    .children((0..6).map(|i| {
                        let color =
                            d3rs::color::interpolate_colors(&viridis_colors, i as f32 / 5.0);
                        let (r, g, b) = (
                            (color.r * 255.0) as u32,
                            (color.g * 255.0) as u32,
                            (color.b * 255.0) as u32,
                        );
                        div().flex_1().h(px(16.0)).bg(rgb((r << 16) | (g << 8) | b))
                    }))
                    .child(render_glyph_text(
                        &format!("{:.0}°", angle_max),
                        &font_config,
                    ))
            })
    }
}
