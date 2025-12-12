//! Configuration for 3D surface rendering

use glam::Vec3;

/// Available colormaps for surface visualization
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Colormap {
    /// Viridis (perceptually uniform, colorblind-friendly)
    #[default]
    Viridis,
    /// Plasma (perceptually uniform, warm colors)
    Plasma,
    /// Inferno (perceptually uniform, hot colors)
    Inferno,
    /// Turbo (Google's improved rainbow)
    Turbo,
    /// Cool-Warm diverging colormap
    CoolWarm,
}

impl Colormap {
    /// Get colormap index for shader
    pub fn shader_index(&self) -> u32 {
        match self {
            Colormap::Viridis => 0,
            Colormap::Plasma => 1,
            Colormap::Inferno => 2,
            Colormap::Turbo => 3,
            Colormap::CoolWarm => 4,
        }
    }
}

/// Type of 3D surface plot
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SurfacePlotType {
    /// Standard Cartesian plot (X, Y, Z)
    #[default]
    Cartesian,
    /// Spherical plot (Globe/Balloon)
    Spherical,
}

/// Configuration for 3D surface rendering
#[derive(Debug, Clone)]
pub struct Surface3DConfig {
    /// Colormap for surface coloring
    pub colormap: Colormap,
    /// Show wireframe overlay
    pub wireframe: bool,
    /// Wireframe color (RGB)
    pub wireframe_color: [f32; 3],
    /// Background color (RGB)
    pub background_color: [f32; 3],
    /// Ambient lighting intensity (0.0 - 1.0)
    pub ambient: f32,
    /// Diffuse lighting intensity (0.0 - 1.0)
    pub diffuse: f32,
    /// Light direction (will be normalized)
    pub light_direction: Vec3,
    /// Enable anti-aliasing (MSAA)
    pub msaa_samples: u32,
    /// Initial camera distance
    pub camera_distance: f32,
    /// Initial camera azimuth (degrees)
    pub camera_azimuth: f32,
    /// Initial camera elevation (degrees)
    pub camera_elevation: f32,
    /// Show axis labels
    pub show_axes: bool,
    /// Show colorbar legend
    pub show_colorbar: bool,
    /// Surface opacity (0.0 - 1.0)
    pub opacity: f32,
    /// Show isolines projection
    pub isolines: bool,
    /// Isoline step size (normalized 0-1)
    pub isoline_step: f32,
    /// Plot type (Cartesian or Spherical)
    pub plot_type: SurfacePlotType,
    /// Show grid/bounding box
    pub show_grid: bool,
}

impl Default for Surface3DConfig {
    fn default() -> Self {
        Self {
            colormap: Colormap::Viridis,
            wireframe: false,
            wireframe_color: [0.2, 0.2, 0.2],
            background_color: [0.1, 0.1, 0.12],
            ambient: 0.3,
            diffuse: 0.7,
            light_direction: Vec3::new(1.0, 1.0, 1.0),
            msaa_samples: 4,
            camera_distance: 3.5,
            camera_azimuth: 45.0,
            camera_elevation: 30.0,
            show_axes: true,
            show_colorbar: true,
            opacity: 1.0,
            isolines: false,
            isoline_step: 0.05,
            plot_type: SurfacePlotType::Cartesian,
            show_grid: true,
        }
    }
}

impl Surface3DConfig {
    /// Create a new configuration with default settings
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the colormap
    pub fn colormap(mut self, colormap: Colormap) -> Self {
        self.colormap = colormap;
        self
    }

    /// Enable or disable wireframe overlay
    pub fn wireframe(mut self, enabled: bool) -> Self {
        self.wireframe = enabled;
        self
    }

    /// Set wireframe color
    pub fn wireframe_color(mut self, r: f32, g: f32, b: f32) -> Self {
        self.wireframe_color = [r, g, b];
        self
    }

    /// Set background color
    pub fn background_color(mut self, r: f32, g: f32, b: f32) -> Self {
        self.background_color = [r, g, b];
        self
    }

    /// Set ambient lighting (0.0 - 1.0)
    pub fn ambient(mut self, ambient: f32) -> Self {
        self.ambient = ambient.clamp(0.0, 1.0);
        self
    }

    /// Set diffuse lighting (0.0 - 1.0)
    pub fn diffuse(mut self, diffuse: f32) -> Self {
        self.diffuse = diffuse.clamp(0.0, 1.0);
        self
    }

    /// Set light direction
    pub fn light_direction(mut self, x: f32, y: f32, z: f32) -> Self {
        self.light_direction = Vec3::new(x, y, z).normalize();
        self
    }

    /// Set MSAA sample count (1, 2, 4, or 8)
    pub fn msaa_samples(mut self, samples: u32) -> Self {
        self.msaa_samples = match samples {
            1 | 2 | 4 | 8 => samples,
            _ => 4,
        };
        self
    }

    /// Set initial camera position
    pub fn camera_position(mut self, distance: f32, azimuth_deg: f32, elevation_deg: f32) -> Self {
        self.camera_distance = distance;
        self.camera_azimuth = azimuth_deg;
        self.camera_elevation = elevation_deg;
        self
    }

    /// Enable or disable axis display
    pub fn show_axes(mut self, enabled: bool) -> Self {
        self.show_axes = enabled;
        self
    }

    /// Show colorbar legend
    pub fn show_colorbar(mut self, enabled: bool) -> Self {
        self.show_colorbar = enabled;
        self
    }

    /// Set surface opacity (0.0 - 1.0)
    pub fn opacity(mut self, opacity: f32) -> Self {
        self.opacity = opacity.clamp(0.0, 1.0);
        self
    }

    /// Enable or disable isolines
    pub fn isolines(mut self, enabled: bool) -> Self {
        self.isolines = enabled;
        self
    }

    /// Set isoline step size (in normalized 0-1 range)
    pub fn isoline_step(mut self, step: f32) -> Self {
        self.isoline_step = step.clamp(0.001, 1.0);
        self
    }

    /// Set plot type
    pub fn plot_type(mut self, plot_type: SurfacePlotType) -> Self {
        self.plot_type = plot_type;
        self
    }

    /// Enable or disable grid/bounding box display
    pub fn show_grid(mut self, enabled: bool) -> Self {
        self.show_grid = enabled;
        self
    }

    /// Get normalized light direction
    pub fn normalized_light_direction(&self) -> Vec3 {
        self.light_direction.normalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Surface3DConfig::default();
        assert_eq!(config.colormap, Colormap::Viridis);
        assert!(!config.wireframe);
        assert!(config.ambient > 0.0);
        assert!(config.diffuse > 0.0);
    }

    #[test]
    fn test_builder_pattern() {
        let config = Surface3DConfig::new()
            .colormap(Colormap::Plasma)
            .wireframe(true)
            .ambient(0.5)
            .diffuse(0.5)
            .camera_position(5.0, 60.0, 45.0);

        assert_eq!(config.colormap, Colormap::Plasma);
        assert!(config.wireframe);
        assert!((config.ambient - 0.5).abs() < 0.01);
        assert!((config.camera_distance - 5.0).abs() < 0.01);
    }

    #[test]
    fn test_ambient_clamping() {
        let config = Surface3DConfig::new().ambient(1.5);
        assert!((config.ambient - 1.0).abs() < 0.01);

        let config = Surface3DConfig::new().ambient(-0.5);
        assert!((config.ambient - 0.0).abs() < 0.01);
    }

    #[test]
    fn test_msaa_validation() {
        let config = Surface3DConfig::new().msaa_samples(3);
        assert_eq!(config.msaa_samples, 4); // Should default to 4

        let config = Surface3DConfig::new().msaa_samples(8);
        assert_eq!(config.msaa_samples, 8);
    }
}
