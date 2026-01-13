//! EQ Visualization for Audio Unit plugin
//!
//! Renders parametric EQ curves and control points.

use crate::renderer::Renderer2D;

/// EQ band filter types
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum FilterType {
    Peak,
    LowShelf,
    HighShelf,
    LowPass,
    HighPass,
}

/// Single EQ band
#[derive(Clone, Debug)]
pub struct EQBand {
    pub filter_type: FilterType,
    pub frequency: f32,
    pub gain_db: f32,
    pub q: f32,
    pub enabled: bool,
}

impl Default for EQBand {
    fn default() -> Self {
        Self {
            filter_type: FilterType::Peak,
            frequency: 1000.0,
            gain_db: 0.0,
            q: 1.0,
            enabled: true,
        }
    }
}

/// Theme colors for the EQ view
pub struct EQTheme {
    pub background: [f32; 4],
    pub grid_major: [f32; 4],
    pub grid_minor: [f32; 4],
    pub curve: [f32; 4],
    pub curve_fill: [f32; 4],
    pub handle: [f32; 4],
    pub handle_selected: [f32; 4],
    pub text: [f32; 4],
}

impl Default for EQTheme {
    fn default() -> Self {
        Self {
            background: [0.1, 0.1, 0.12, 1.0],
            grid_major: [0.3, 0.3, 0.35, 1.0],
            grid_minor: [0.2, 0.2, 0.22, 0.5],
            curve: [0.4, 0.7, 1.0, 1.0],
            curve_fill: [0.4, 0.7, 1.0, 0.2],
            handle: [0.9, 0.9, 0.9, 1.0],
            handle_selected: [1.0, 0.6, 0.2, 1.0],
            text: [0.8, 0.8, 0.8, 1.0],
        }
    }
}

/// EQ visualization view
pub struct EQView {
    /// EQ bands
    pub bands: Vec<EQBand>,
    /// Selected band index (for editing)
    pub selected_band: Option<usize>,
    /// Theme
    pub theme: EQTheme,
    /// Display range
    pub min_freq: f32,
    pub max_freq: f32,
    pub min_db: f32,
    pub max_db: f32,
    /// Cached response curve points
    response_cache: Vec<[f32; 2]>,
    /// Cache valid flag
    cache_valid: bool,
}

impl Default for EQView {
    fn default() -> Self {
        Self {
            bands: Vec::new(),
            selected_band: None,
            theme: EQTheme::default(),
            min_freq: 20.0,
            max_freq: 20000.0,
            min_db: -24.0,
            max_db: 24.0,
            response_cache: Vec::new(),
            cache_valid: false,
        }
    }
}

impl EQView {
    /// Create a new EQ view with default bands
    pub fn new() -> Self {
        let mut view = Self::default();
        // Add some default bands
        view.bands = vec![
            EQBand {
                filter_type: FilterType::HighPass,
                frequency: 30.0,
                gain_db: 0.0,
                q: 0.7,
                enabled: true,
            },
            EQBand {
                filter_type: FilterType::Peak,
                frequency: 100.0,
                gain_db: 3.0,
                q: 1.5,
                enabled: true,
            },
            EQBand {
                filter_type: FilterType::Peak,
                frequency: 1000.0,
                gain_db: -2.0,
                q: 2.0,
                enabled: true,
            },
            EQBand {
                filter_type: FilterType::Peak,
                frequency: 4000.0,
                gain_db: 1.5,
                q: 1.0,
                enabled: true,
            },
            EQBand {
                filter_type: FilterType::LowPass,
                frequency: 18000.0,
                gain_db: 0.0,
                q: 0.7,
                enabled: true,
            },
        ];
        view
    }

    /// Invalidate the response cache
    pub fn invalidate_cache(&mut self) {
        self.cache_valid = false;
    }

    /// Set bands from external source
    pub fn set_bands(&mut self, bands: Vec<EQBand>) {
        self.bands = bands;
        self.invalidate_cache();
    }

    /// Convert frequency to X position
    fn freq_to_x(&self, freq: f32, width: f32) -> f32 {
        let log_min = self.min_freq.ln();
        let log_max = self.max_freq.ln();
        let log_freq = freq.clamp(self.min_freq, self.max_freq).ln();
        ((log_freq - log_min) / (log_max - log_min)) * width
    }

    /// Convert X position to frequency
    fn x_to_freq(&self, x: f32, width: f32) -> f32 {
        let log_min = self.min_freq.ln();
        let log_max = self.max_freq.ln();
        let t = x / width;
        (log_min + t * (log_max - log_min)).exp()
    }

    /// Convert dB to Y position
    fn db_to_y(&self, db: f32, height: f32) -> f32 {
        let t = (db - self.max_db) / (self.min_db - self.max_db);
        t * height
    }

    /// Convert Y position to dB
    fn y_to_db(&self, y: f32, height: f32) -> f32 {
        let t = y / height;
        self.max_db + t * (self.min_db - self.max_db)
    }

    /// Calculate the magnitude response of a single band at a given frequency
    fn band_magnitude(&self, band: &EQBand, freq: f32, sample_rate: f32) -> f32 {
        if !band.enabled {
            return 0.0;
        }

        // Simplified biquad magnitude calculation
        // For a proper implementation, use the actual biquad coefficients
        let w0 = 2.0 * std::f32::consts::PI * band.frequency / sample_rate;
        let w = 2.0 * std::f32::consts::PI * freq / sample_rate;

        match band.filter_type {
            FilterType::Peak => {
                // Peaking EQ approximation
                let bw = w0 / band.q;
                let delta = (w - w0).abs();
                let response = band.gain_db * (-delta * delta / (bw * bw)).exp();
                response
            }
            FilterType::LowShelf => {
                // Low shelf approximation
                let ratio = freq / band.frequency;
                if ratio < 1.0 {
                    band.gain_db
                } else {
                    band.gain_db * (1.0 / ratio).powf(1.0 / band.q)
                }
            }
            FilterType::HighShelf => {
                // High shelf approximation
                let ratio = freq / band.frequency;
                if ratio > 1.0 {
                    band.gain_db
                } else {
                    band.gain_db * ratio.powf(1.0 / band.q)
                }
            }
            FilterType::LowPass => {
                // Low pass approximation (6dB/octave slope visualization)
                let ratio = freq / band.frequency;
                if ratio <= 1.0 {
                    0.0
                } else {
                    -12.0 * (ratio.ln() / 2.0_f32.ln())
                }
            }
            FilterType::HighPass => {
                // High pass approximation
                let ratio = freq / band.frequency;
                if ratio >= 1.0 {
                    0.0
                } else {
                    -12.0 * ((1.0 / ratio).ln() / 2.0_f32.ln())
                }
            }
        }
    }

    /// Calculate total magnitude response at a frequency
    fn total_magnitude(&self, freq: f32, sample_rate: f32) -> f32 {
        self.bands
            .iter()
            .map(|band| self.band_magnitude(band, freq, sample_rate))
            .sum()
    }

    /// Update the response cache
    fn update_cache(&mut self, width: f32, height: f32) {
        if self.cache_valid {
            return;
        }

        let sample_rate = 48000.0;
        let num_points = (width as usize).max(100);
        self.response_cache.clear();

        for i in 0..num_points {
            let x = i as f32 / (num_points - 1) as f32 * width;
            let freq = self.x_to_freq(x, width);
            let db = self.total_magnitude(freq, sample_rate);
            let y = self.db_to_y(db, height);
            self.response_cache.push([x, y]);
        }

        self.cache_valid = true;
    }

    /// Render the EQ view
    pub fn render(&mut self, renderer: &mut Renderer2D, width: f32, height: f32) {
        let padding = 40.0;
        let graph_width = width - padding * 2.0;
        let graph_height = height - padding * 2.0;

        // Draw grid
        self.draw_grid(renderer, padding, padding, graph_width, graph_height);

        // Update and draw response curve
        self.update_cache(graph_width, graph_height);
        self.draw_response_curve(renderer, padding, padding, graph_width, graph_height);

        // Draw band handles
        self.draw_handles(renderer, padding, padding, graph_width, graph_height);
    }

    /// Draw the frequency/dB grid
    fn draw_grid(&self, renderer: &mut Renderer2D, x: f32, y: f32, width: f32, height: f32) {
        // Draw border
        renderer.draw_rect(x, y, width, height, [0.15, 0.15, 0.17, 1.0]);

        // Frequency grid lines (logarithmic)
        let freq_lines = [
            20.0, 50.0, 100.0, 200.0, 500.0, 1000.0, 2000.0, 5000.0, 10000.0, 20000.0,
        ];
        for &freq in &freq_lines {
            let line_x = x + self.freq_to_x(freq, width);
            let is_major = freq == 100.0 || freq == 1000.0 || freq == 10000.0;
            let color = if is_major {
                self.theme.grid_major
            } else {
                self.theme.grid_minor
            };
            renderer.draw_line(line_x, y, line_x, y + height, color, 1.0);
        }

        // dB grid lines
        let db_lines = [-24.0, -18.0, -12.0, -6.0, 0.0, 6.0, 12.0, 18.0, 24.0];
        for &db in &db_lines {
            let line_y = y + self.db_to_y(db, height);
            let is_major = db == 0.0;
            let color = if is_major {
                self.theme.grid_major
            } else {
                self.theme.grid_minor
            };
            renderer.draw_line(
                x,
                line_y,
                x + width,
                line_y,
                color,
                if is_major { 2.0 } else { 1.0 },
            );
        }
    }

    /// Draw the response curve
    fn draw_response_curve(
        &self,
        renderer: &mut Renderer2D,
        x: f32,
        y: f32,
        _width: f32,
        height: f32,
    ) {
        if self.response_cache.len() < 2 {
            return;
        }

        // Offset points by graph position
        let points: Vec<[f32; 2]> = self
            .response_cache
            .iter()
            .map(|p| [p[0] + x, p[1] + y])
            .collect();

        // Draw fill (from curve to center line)
        let center_y = y + height / 2.0;
        for i in 0..points.len().saturating_sub(1) {
            let p1 = points[i];
            let p2 = points[i + 1];

            // Draw quad from curve to center
            renderer.draw_rect(
                p1[0].min(p2[0]),
                p1[1].min(center_y),
                (p2[0] - p1[0]).abs().max(1.0),
                (center_y - p1[1]).abs(),
                self.theme.curve_fill,
            );
        }

        // Draw curve line
        renderer.draw_polyline(&points, self.theme.curve, 2.0);
    }

    /// Draw band control handles
    fn draw_handles(&self, renderer: &mut Renderer2D, x: f32, y: f32, width: f32, height: f32) {
        for (i, band) in self.bands.iter().enumerate() {
            if !band.enabled {
                continue;
            }

            let handle_x = x + self.freq_to_x(band.frequency, width);
            let handle_y = y + self.db_to_y(band.gain_db, height);

            let color = if self.selected_band == Some(i) {
                self.theme.handle_selected
            } else {
                self.theme.handle
            };

            // Draw handle
            renderer.draw_circle(handle_x, handle_y, 8.0, color, 16);

            // Draw inner circle for filter type indication
            let inner_color = match band.filter_type {
                FilterType::Peak => [0.4, 0.7, 1.0, 1.0],
                FilterType::LowShelf => [0.4, 1.0, 0.7, 1.0],
                FilterType::HighShelf => [1.0, 0.7, 0.4, 1.0],
                FilterType::LowPass => [1.0, 0.4, 0.4, 1.0],
                FilterType::HighPass => [0.7, 0.4, 1.0, 1.0],
            };
            renderer.draw_circle(handle_x, handle_y, 4.0, inner_color, 12);
        }
    }

    /// Handle mouse down event
    /// Returns the band index if a handle was hit
    pub fn handle_mouse_down(
        &mut self,
        mouse_x: f32,
        mouse_y: f32,
        width: f32,
        height: f32,
    ) -> Option<usize> {
        let padding = 40.0;
        let graph_width = width - padding * 2.0;
        let graph_height = height - padding * 2.0;

        // Check each band handle
        for (i, band) in self.bands.iter().enumerate() {
            if !band.enabled {
                continue;
            }

            let handle_x = padding + self.freq_to_x(band.frequency, graph_width);
            let handle_y = padding + self.db_to_y(band.gain_db, graph_height);

            let dx = mouse_x - handle_x;
            let dy = mouse_y - handle_y;
            let dist = (dx * dx + dy * dy).sqrt();

            if dist <= 12.0 {
                self.selected_band = Some(i);
                return Some(i);
            }
        }

        self.selected_band = None;
        None
    }

    /// Handle mouse drag event
    pub fn handle_mouse_drag(&mut self, mouse_x: f32, mouse_y: f32, width: f32, height: f32) {
        if let Some(idx) = self.selected_band {
            let padding = 40.0;
            let graph_width = width - padding * 2.0;
            let graph_height = height - padding * 2.0;

            let local_x = (mouse_x - padding).clamp(0.0, graph_width);
            let local_y = (mouse_y - padding).clamp(0.0, graph_height);

            let new_freq = self.x_to_freq(local_x, graph_width);
            let new_db = self.y_to_db(local_y, graph_height);

            if let Some(band) = self.bands.get_mut(idx) {
                band.frequency = new_freq.clamp(self.min_freq, self.max_freq);
                band.gain_db = new_db.clamp(self.min_db, self.max_db);
                self.invalidate_cache();
            }
        }
    }

    /// Handle mouse up event
    pub fn handle_mouse_up(&mut self) {
        // Keep selection for now (could clear if desired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_freq_conversion() {
        let view = EQView::default();
        let width = 800.0;

        // Test roundtrip
        let freq = 1000.0;
        let x = view.freq_to_x(freq, width);
        let back = view.x_to_freq(x, width);
        assert!((freq - back).abs() < 1.0);
    }

    #[test]
    fn test_db_conversion() {
        let view = EQView::default();
        let height = 400.0;

        // Test roundtrip
        let db = 6.0;
        let y = view.db_to_y(db, height);
        let back = view.y_to_db(y, height);
        assert!((db - back).abs() < 0.1);
    }
}
