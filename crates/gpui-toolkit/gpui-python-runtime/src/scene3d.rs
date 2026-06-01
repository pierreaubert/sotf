use crate::error::Scene3DError;
use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Point3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl Point3 {
    pub const ZERO: Self = Self {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };

    #[must_use]
    pub const fn new(x: f32, y: f32, z: f32) -> Self {
        Self { x, y, z }
    }

    pub(crate) fn validate(&self, field: &'static str) -> Result<(), Scene3DError> {
        if !self.x.is_finite() || !self.y.is_finite() || !self.z.is_finite() {
            return Err(Scene3DError::InvalidData {
                field,
                reason: "contains NaN or Infinity",
            });
        }
        Ok(())
    }

    pub(crate) fn hash_into(&self, h: &mut impl Hasher) {
        self.x.to_bits().hash(h);
        self.y.to_bits().hash(h);
        self.z.to_bits().hash(h);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ViewportSize {
    pub width: f32,
    pub height: f32,
}

impl ViewportSize {
    #[must_use]
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }

    pub(crate) fn validate(&self) -> Result<(), Scene3DError> {
        if !self.width.is_finite() || self.width <= 0.0 {
            return Err(Scene3DError::InvalidData {
                field: "width",
                reason: "must be positive and finite",
            });
        }
        if !self.height.is_finite() || self.height <= 0.0 {
            return Err(Scene3DError::InvalidData {
                field: "height",
                reason: "must be positive and finite",
            });
        }
        Ok(())
    }

    pub(crate) fn hash_into(&self, h: &mut impl Hasher) {
        self.width.to_bits().hash(h);
        self.height.to_bits().hash(h);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ScalarRange {
    pub min: f64,
    pub max: f64,
}

impl ScalarRange {
    #[must_use]
    pub const fn new(min: f64, max: f64) -> Self {
        Self { min, max }
    }

    pub(crate) fn validate(&self, field: &'static str) -> Result<(), Scene3DError> {
        if !self.min.is_finite() || !self.max.is_finite() {
            return Err(Scene3DError::InvalidData {
                field,
                reason: "range contains NaN or Infinity",
            });
        }
        if !matches!(
            self.min.partial_cmp(&self.max),
            Some(std::cmp::Ordering::Less)
        ) {
            return Err(Scene3DError::InvalidData {
                field,
                reason: "range min must be less than max",
            });
        }
        Ok(())
    }

    pub(crate) fn validate_positive(&self, field: &'static str) -> Result<(), Scene3DError> {
        self.validate(field)?;
        if self.min <= 0.0 || self.max <= 0.0 {
            return Err(Scene3DError::InvalidData {
                field,
                reason: "log range must be strictly positive",
            });
        }
        Ok(())
    }

    pub(crate) fn hash_into(&self, h: &mut impl Hasher) {
        self.min.to_bits().hash(h);
        self.max.to_bits().hash(h);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ColorRgba {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Default for ColorRgba {
    fn default() -> Self {
        Self::from_rgb_u8(255, 255, 255)
    }
}

impl ColorRgba {
    #[must_use]
    pub const fn new(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    #[must_use]
    pub const fn from_rgb_u8(r: u8, g: u8, b: u8) -> Self {
        Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        }
    }

    pub fn from_hex(value: &str) -> Result<Self, Scene3DError> {
        let trimmed = value.trim();
        let Some(hex) = trimmed.strip_prefix('#') else {
            return Err(Scene3DError::InvalidColor {
                value: value.to_string(),
            });
        };
        if hex.len() != 6 && hex.len() != 8 {
            return Err(Scene3DError::InvalidColor {
                value: value.to_string(),
            });
        }

        let rgba = u32::from_str_radix(hex, 16).map_err(|_| Scene3DError::InvalidColor {
            value: value.to_string(),
        })?;

        let (r, g, b, a) = if hex.len() == 6 {
            (
                ((rgba >> 16) & 0xff) as u8,
                ((rgba >> 8) & 0xff) as u8,
                (rgba & 0xff) as u8,
                255,
            )
        } else {
            (
                ((rgba >> 24) & 0xff) as u8,
                ((rgba >> 16) & 0xff) as u8,
                ((rgba >> 8) & 0xff) as u8,
                (rgba & 0xff) as u8,
            )
        };

        Ok(Self {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: a as f32 / 255.0,
        })
    }

    pub(crate) fn validate(&self, field: &'static str) -> Result<(), Scene3DError> {
        if !self.r.is_finite() || !self.g.is_finite() || !self.b.is_finite() || !self.a.is_finite()
        {
            return Err(Scene3DError::InvalidData {
                field,
                reason: "contains NaN or Infinity",
            });
        }
        if !(0.0..=1.0).contains(&self.r)
            || !(0.0..=1.0).contains(&self.g)
            || !(0.0..=1.0).contains(&self.b)
            || !(0.0..=1.0).contains(&self.a)
        {
            return Err(Scene3DError::InvalidData {
                field,
                reason: "channels must be in 0..=1",
            });
        }
        Ok(())
    }

    pub(crate) fn hash_into(&self, h: &mut impl Hasher) {
        self.r.to_bits().hash(h);
        self.g.to_bits().hash(h);
        self.b.to_bits().hash(h);
        self.a.to_bits().hash(h);
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ColormapSpec {
    #[default]
    Viridis,
    Plasma,
    Inferno,
    Turbo,
    CoolWarm,
}

impl ColormapSpec {
    pub fn parse(value: &str) -> Result<Self, Scene3DError> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "viridis" => Ok(Self::Viridis),
            "plasma" => Ok(Self::Plasma),
            "inferno" => Ok(Self::Inferno),
            "turbo" => Ok(Self::Turbo),
            "coolwarm" | "cool_warm" => Ok(Self::CoolWarm),
            _ => Err(Scene3DError::UnknownColormap {
                name: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InteractionMode {
    Orbit,
    Pan,
    Zoom,
    Reset,
    HitTest,
}

impl InteractionMode {
    pub fn parse(value: &str) -> Result<Self, Scene3DError> {
        match value.trim().to_ascii_lowercase().replace('-', "_").as_str() {
            "orbit" => Ok(Self::Orbit),
            "pan" => Ok(Self::Pan),
            "zoom" => Ok(Self::Zoom),
            "reset" => Ok(Self::Reset),
            "hit_test" | "hittest" => Ok(Self::HitTest),
            _ => Err(Scene3DError::UnknownInteraction {
                name: value.to_string(),
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CameraSpec {
    Orbit(OrbitCameraSpec),
    Perspective(PerspectiveCameraSpec),
}

impl Default for CameraSpec {
    fn default() -> Self {
        Self::Orbit(OrbitCameraSpec::default())
    }
}

impl CameraSpec {
    pub(crate) fn validate(&self) -> Result<(), Scene3DError> {
        match self {
            Self::Orbit(camera) => camera.validate(),
            Self::Perspective(camera) => camera.validate(),
        }
    }

    pub(crate) fn hash_into(&self, h: &mut impl Hasher) {
        match self {
            Self::Orbit(camera) => {
                0_u8.hash(h);
                camera.hash_into(h);
            }
            Self::Perspective(camera) => {
                1_u8.hash(h);
                camera.hash_into(h);
            }
        }
    }

    #[must_use]
    pub fn as_orbit(&self) -> Option<&OrbitCameraSpec> {
        match self {
            Self::Orbit(camera) => Some(camera),
            Self::Perspective(_) => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OrbitCameraSpec {
    pub distance: f32,
    pub azimuth_deg: f32,
    pub elevation_deg: f32,
    pub target: Point3,
    pub fov_y_deg: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for OrbitCameraSpec {
    fn default() -> Self {
        Self {
            distance: 3.5,
            azimuth_deg: 45.0,
            elevation_deg: 30.0,
            target: Point3::ZERO,
            fov_y_deg: 45.0,
            near: 0.1,
            far: 100.0,
        }
    }
}

impl OrbitCameraSpec {
    #[must_use]
    pub fn new(distance: f32, azimuth_deg: f32, elevation_deg: f32) -> Self {
        Self {
            distance,
            azimuth_deg,
            elevation_deg,
            ..Self::default()
        }
    }

    fn validate(&self) -> Result<(), Scene3DError> {
        validate_positive_f32(self.distance, "camera.distance")?;
        validate_finite_f32(self.azimuth_deg, "camera.azimuth_deg")?;
        validate_finite_f32(self.elevation_deg, "camera.elevation_deg")?;
        validate_positive_f32(self.fov_y_deg, "camera.fov_y_deg")?;
        validate_positive_f32(self.near, "camera.near")?;
        validate_positive_f32(self.far, "camera.far")?;
        if self.near >= self.far {
            return Err(Scene3DError::InvalidData {
                field: "camera.far",
                reason: "must be greater than near",
            });
        }
        self.target.validate("camera.target")
    }

    fn hash_into(&self, h: &mut impl Hasher) {
        self.distance.to_bits().hash(h);
        self.azimuth_deg.to_bits().hash(h);
        self.elevation_deg.to_bits().hash(h);
        self.target.hash_into(h);
        self.fov_y_deg.to_bits().hash(h);
        self.near.to_bits().hash(h);
        self.far.to_bits().hash(h);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PerspectiveCameraSpec {
    pub position: Point3,
    pub target: Point3,
    pub up: Point3,
    pub fov_y_deg: f32,
    pub near: f32,
    pub far: f32,
}

impl Default for PerspectiveCameraSpec {
    fn default() -> Self {
        Self {
            position: Point3::new(2.0, 2.0, 2.0),
            target: Point3::ZERO,
            up: Point3::new(0.0, 1.0, 0.0),
            fov_y_deg: 45.0,
            near: 0.1,
            far: 100.0,
        }
    }
}

impl PerspectiveCameraSpec {
    fn validate(&self) -> Result<(), Scene3DError> {
        self.position.validate("camera.position")?;
        self.target.validate("camera.target")?;
        self.up.validate("camera.up")?;
        validate_positive_f32(self.fov_y_deg, "camera.fov_y_deg")?;
        validate_positive_f32(self.near, "camera.near")?;
        validate_positive_f32(self.far, "camera.far")?;
        if self.near >= self.far {
            return Err(Scene3DError::InvalidData {
                field: "camera.far",
                reason: "must be greater than near",
            });
        }
        Ok(())
    }

    fn hash_into(&self, h: &mut impl Hasher) {
        self.position.hash_into(h);
        self.target.hash_into(h);
        self.up.hash_into(h);
        self.fov_y_deg.to_bits().hash(h);
        self.near.to_bits().hash(h);
        self.far.to_bits().hash(h);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GridData {
    pub values: Vec<f64>,
    pub width: usize,
    pub height: usize,
}

impl GridData {
    #[must_use]
    pub fn from_flat(values: Vec<f64>, width: usize, height: usize) -> Self {
        Self {
            values,
            width,
            height,
        }
    }

    pub fn from_rows(rows: Vec<Vec<f64>>) -> Result<Self, Scene3DError> {
        if rows.is_empty() {
            return Err(Scene3DError::EmptyData { field: "z" });
        }
        let height = rows.len();
        let width = rows[0].len();
        if width == 0 {
            return Err(Scene3DError::EmptyData { field: "z" });
        }
        let mut values = Vec::with_capacity(width * height);
        for row in rows {
            if row.len() != width {
                return Err(Scene3DError::GridDimensionMismatch {
                    z_len: values.len() + row.len(),
                    width,
                    height,
                    expected: width * height,
                });
            }
            values.extend(row);
        }
        Ok(Self {
            values,
            width,
            height,
        })
    }

    pub fn validate(&self) -> Result<(), Scene3DError> {
        if self.width == 0 || self.height == 0 || self.values.is_empty() {
            return Err(Scene3DError::EmptyData { field: "z" });
        }
        let expected = self.width * self.height;
        if self.values.len() != expected {
            return Err(Scene3DError::GridDimensionMismatch {
                z_len: self.values.len(),
                width: self.width,
                height: self.height,
                expected,
            });
        }
        validate_finite_f64_slice(&self.values, "z")
    }

    #[must_use]
    pub fn rows(&self) -> Vec<Vec<f64>> {
        self.values
            .chunks(self.width)
            .map(<[f64]>::to_vec)
            .collect()
    }

    pub(crate) fn hash_into(&self, h: &mut impl Hasher) {
        self.width.hash(h);
        self.height.hash(h);
        hash_f64_slice(&self.values, h);
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct AxisLabels {
    pub x: Option<String>,
    pub y: Option<String>,
    pub z: Option<String>,
    pub title: Option<String>,
}

impl AxisLabels {
    pub(crate) fn hash_into(&self, h: &mut impl Hasher) {
        self.x.hash(h);
        self.y.hash(h);
        self.z.hash(h);
        self.title.hash(h);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SurfaceSpec {
    pub id: String,
    pub z: GridData,
    pub x: Option<Vec<f64>>,
    pub y: Option<Vec<f64>>,
    #[serde(default)]
    pub colormap: ColormapSpec,
    #[serde(default)]
    pub wireframe: bool,
    #[serde(default)]
    pub x_log: bool,
    #[serde(default)]
    pub y_log: bool,
    #[serde(default)]
    pub z_log: bool,
    pub z_range: Option<ScalarRange>,
    #[serde(default)]
    pub labels: AxisLabels,
    pub camera: Option<CameraSpec>,
    #[serde(default)]
    pub interactions: Vec<InteractionMode>,
    pub size: Option<ViewportSize>,
}

impl SurfaceSpec {
    #[must_use]
    pub fn from_flat(id: impl Into<String>, z: Vec<f64>, width: usize, height: usize) -> Self {
        Self {
            id: id.into(),
            z: GridData::from_flat(z, width, height),
            x: None,
            y: None,
            colormap: ColormapSpec::default(),
            wireframe: false,
            x_log: false,
            y_log: false,
            z_log: false,
            z_range: None,
            labels: AxisLabels::default(),
            camera: None,
            interactions: Vec::new(),
            size: None,
        }
    }

    pub fn from_rows(id: impl Into<String>, rows: Vec<Vec<f64>>) -> Result<Self, Scene3DError> {
        Ok(Self {
            z: GridData::from_rows(rows)?,
            ..Self::from_flat(id, Vec::new(), 0, 0)
        })
    }

    pub fn validate(&self) -> Result<(), Scene3DError> {
        validate_id(&self.id, "surface.id")?;
        self.z.validate()?;
        if let Some(x) = &self.x {
            validate_axis(x, self.z.width, "x", "grid_width", self.x_log)?;
        } else if self.x_log {
            return Err(Scene3DError::InvalidData {
                field: "x",
                reason: "log axis requires explicit positive values",
            });
        }
        if let Some(y) = &self.y {
            validate_axis(y, self.z.height, "y", "grid_height", self.y_log)?;
        } else if self.y_log {
            return Err(Scene3DError::InvalidData {
                field: "y",
                reason: "log axis requires explicit positive values",
            });
        }
        if self.z_log {
            validate_positive_f64_slice(&self.z.values, "z")?;
        }
        if let Some(range) = &self.z_range {
            if self.z_log {
                range.validate_positive("z_range")?;
            } else {
                range.validate("z_range")?;
            }
        }
        if let Some(camera) = &self.camera {
            camera.validate()?;
        }
        if let Some(size) = &self.size {
            size.validate()?;
        }
        Ok(())
    }

    #[must_use]
    pub fn x_values(&self) -> Vec<f64> {
        self.x
            .clone()
            .unwrap_or_else(|| (0..self.z.width).map(|value| value as f64).collect())
    }

    #[must_use]
    pub fn y_values(&self) -> Vec<f64> {
        self.y
            .clone()
            .unwrap_or_else(|| (0..self.z.height).map(|value| value as f64).collect())
    }

    pub(crate) fn fingerprints(&self) -> SceneFingerprints {
        let mut geometry = DefaultHasher::new();
        self.id.hash(&mut geometry);
        self.z.hash_into(&mut geometry);
        hash_optional_f64_slice(&self.x, &mut geometry);
        hash_optional_f64_slice(&self.y, &mut geometry);
        self.x_log.hash(&mut geometry);
        self.y_log.hash(&mut geometry);
        self.z_log.hash(&mut geometry);
        if let Some(range) = &self.z_range {
            range.hash_into(&mut geometry);
        }

        let mut material = DefaultHasher::new();
        self.colormap.hash(&mut material);
        self.wireframe.hash(&mut material);
        self.labels.hash_into(&mut material);
        if let Some(size) = &self.size {
            size.hash_into(&mut material);
        }

        let mut camera = DefaultHasher::new();
        let default_camera = CameraSpec::default();
        self.camera
            .as_ref()
            .unwrap_or(&default_camera)
            .hash_into(&mut camera);
        self.interactions.hash(&mut camera);

        SceneFingerprints {
            geometry: geometry.finish(),
            material: material.finish(),
            camera: camera.finish(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MaterialSpec {
    #[serde(default)]
    pub color: ColorRgba,
    #[serde(default = "default_opacity")]
    pub opacity: f32,
}

impl Default for MaterialSpec {
    fn default() -> Self {
        Self {
            color: ColorRgba::default(),
            opacity: 1.0,
        }
    }
}

impl MaterialSpec {
    pub fn validate(&self) -> Result<(), Scene3DError> {
        self.color.validate("material.color")?;
        if !self.opacity.is_finite() || !(0.0..=1.0).contains(&self.opacity) {
            return Err(Scene3DError::InvalidData {
                field: "material.opacity",
                reason: "must be in 0..=1",
            });
        }
        Ok(())
    }

    pub(crate) fn hash_into(&self, h: &mut impl Hasher) {
        self.color.hash_into(h);
        self.opacity.to_bits().hash(h);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineSegmentSpec {
    pub from: Point3,
    pub to: Point3,
    #[serde(default)]
    pub color: ColorRgba,
    #[serde(default = "default_line_width")]
    pub width: f32,
}

impl LineSegmentSpec {
    pub fn validate(&self) -> Result<(), Scene3DError> {
        self.from.validate("line.from")?;
        self.to.validate("line.to")?;
        self.color.validate("line.color")?;
        validate_positive_f32(self.width, "line.width")
    }

    pub(crate) fn hash_into(&self, h: &mut impl Hasher) {
        self.from.hash_into(h);
        self.to.hash_into(h);
        self.color.hash_into(h);
        self.width.to_bits().hash(h);
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineStripSpec {
    pub id: String,
    pub points: Vec<Point3>,
    #[serde(default)]
    pub color: ColorRgba,
    #[serde(default = "default_line_width")]
    pub width: f32,
}

impl LineStripSpec {
    pub fn validate(&self) -> Result<(), Scene3DError> {
        validate_id(&self.id, "line_strip.id")?;
        if self.points.len() < 2 {
            return Err(Scene3DError::EmptyData {
                field: "line_strip.points",
            });
        }
        for point in &self.points {
            point.validate("line_strip.points")?;
        }
        self.color.validate("line_strip.color")?;
        validate_positive_f32(self.width, "line_strip.width")
    }

    #[must_use]
    pub fn to_segments(&self) -> Vec<LineSegmentSpec> {
        self.points
            .windows(2)
            .map(|pair| LineSegmentSpec {
                from: pair[0],
                to: pair[1],
                color: self.color,
                width: self.width,
            })
            .collect()
    }

    pub(crate) fn hash_into(&self, h: &mut impl Hasher) {
        self.id.hash(h);
        for point in &self.points {
            point.hash_into(h);
        }
        self.color.hash_into(h);
        self.width.to_bits().hash(h);
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct LinesSpec {
    pub id: String,
    #[serde(default)]
    pub strips: Vec<LineStripSpec>,
    #[serde(default)]
    pub segments: Vec<LineSegmentSpec>,
    pub background: Option<ColorRgba>,
    pub camera: Option<CameraSpec>,
    #[serde(default)]
    pub interactions: Vec<InteractionMode>,
    pub size: Option<ViewportSize>,
}

impl LinesSpec {
    pub fn validate(&self) -> Result<(), Scene3DError> {
        validate_id(&self.id, "lines.id")?;
        if self.strips.is_empty() && self.segments.is_empty() {
            return Err(Scene3DError::EmptyData { field: "lines" });
        }
        for strip in &self.strips {
            strip.validate()?;
        }
        for segment in &self.segments {
            segment.validate()?;
        }
        if let Some(background) = &self.background {
            background.validate("lines.background")?;
        }
        if let Some(camera) = &self.camera {
            camera.validate()?;
        }
        if let Some(size) = &self.size {
            size.validate()?;
        }
        Ok(())
    }

    #[must_use]
    pub fn flattened_segments(&self) -> Vec<LineSegmentSpec> {
        let mut segments = self.segments.clone();
        segments.extend(self.strips.iter().flat_map(LineStripSpec::to_segments));
        segments
    }

    pub(crate) fn fingerprints(&self) -> SceneFingerprints {
        let mut geometry = DefaultHasher::new();
        self.id.hash(&mut geometry);
        for strip in &self.strips {
            strip.hash_into(&mut geometry);
        }
        for segment in &self.segments {
            segment.hash_into(&mut geometry);
        }

        let mut material = DefaultHasher::new();
        if let Some(background) = &self.background {
            background.hash_into(&mut material);
        }
        if let Some(size) = &self.size {
            size.hash_into(&mut material);
        }

        let mut camera = DefaultHasher::new();
        let default_camera = CameraSpec::default();
        self.camera
            .as_ref()
            .unwrap_or(&default_camera)
            .hash_into(&mut camera);
        self.interactions.hash(&mut camera);

        SceneFingerprints {
            geometry: geometry.finish(),
            material: material.finish(),
            camera: camera.finish(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MeshSpec {
    pub id: String,
    pub vertices: Vec<Point3>,
    pub indices: Vec<u32>,
    #[serde(default)]
    pub material: MaterialSpec,
}

impl MeshSpec {
    pub fn validate(&self) -> Result<(), Scene3DError> {
        validate_id(&self.id, "mesh.id")?;
        if self.vertices.is_empty() {
            return Err(Scene3DError::EmptyData {
                field: "mesh.vertices",
            });
        }
        if self.indices.is_empty() {
            return Err(Scene3DError::EmptyData {
                field: "mesh.indices",
            });
        }
        if !self.indices.len().is_multiple_of(3) {
            return Err(Scene3DError::InvalidData {
                field: "mesh.indices",
                reason: "triangle indices must be a multiple of 3",
            });
        }
        for vertex in &self.vertices {
            vertex.validate("mesh.vertices")?;
        }
        for (position, index) in self.indices.iter().copied().enumerate() {
            if index as usize >= self.vertices.len() {
                return Err(Scene3DError::InvalidMeshIndex {
                    position,
                    index,
                    vertex_count: self.vertices.len(),
                });
            }
        }
        self.material.validate()
    }

    pub(crate) fn fingerprints(&self) -> SceneFingerprints {
        let mut geometry = DefaultHasher::new();
        self.id.hash(&mut geometry);
        for vertex in &self.vertices {
            vertex.hash_into(&mut geometry);
        }
        self.indices.hash(&mut geometry);

        let mut material = DefaultHasher::new();
        self.material.hash_into(&mut material);

        SceneFingerprints {
            geometry: geometry.finish(),
            material: material.finish(),
            camera: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LightSpec {
    pub id: String,
    pub direction: Point3,
    #[serde(default = "default_light_intensity")]
    pub intensity: f32,
    #[serde(default)]
    pub color: ColorRgba,
}

impl LightSpec {
    pub fn validate(&self) -> Result<(), Scene3DError> {
        validate_id(&self.id, "light.id")?;
        self.direction.validate("light.direction")?;
        validate_positive_f32(self.intensity, "light.intensity")?;
        self.color.validate("light.color")
    }

    pub(crate) fn fingerprints(&self) -> SceneFingerprints {
        let mut material = DefaultHasher::new();
        self.id.hash(&mut material);
        self.direction.hash_into(&mut material);
        self.intensity.to_bits().hash(&mut material);
        self.color.hash_into(&mut material);

        SceneFingerprints {
            geometry: 0,
            material: material.finish(),
            camera: 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SceneNode {
    Surface(SurfaceSpec),
    Lines(LinesSpec),
    Mesh(MeshSpec),
    Light(LightSpec),
}

impl SceneNode {
    pub fn id(&self) -> &str {
        match self {
            Self::Surface(spec) => &spec.id,
            Self::Lines(spec) => &spec.id,
            Self::Mesh(spec) => &spec.id,
            Self::Light(spec) => &spec.id,
        }
    }

    pub fn validate(&self) -> Result<(), Scene3DError> {
        match self {
            Self::Surface(spec) => spec.validate(),
            Self::Lines(spec) => spec.validate(),
            Self::Mesh(spec) => spec.validate(),
            Self::Light(spec) => spec.validate(),
        }
    }

    pub(crate) fn fingerprints(&self) -> SceneFingerprints {
        match self {
            Self::Surface(spec) => spec.fingerprints(),
            Self::Lines(spec) => spec.fingerprints(),
            Self::Mesh(spec) => spec.fingerprints(),
            Self::Light(spec) => spec.fingerprints(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SceneSpec {
    pub id: String,
    #[serde(default)]
    pub camera: CameraSpec,
    #[serde(default)]
    pub children: Vec<SceneNode>,
    #[serde(default)]
    pub interactions: Vec<InteractionMode>,
    pub background: Option<ColorRgba>,
    pub size: Option<ViewportSize>,
}

impl SceneSpec {
    pub fn validate(&self) -> Result<(), Scene3DError> {
        validate_id(&self.id, "scene.id")?;
        self.camera.validate()?;
        if self.children.is_empty() {
            return Err(Scene3DError::EmptyData {
                field: "scene.children",
            });
        }
        for child in &self.children {
            child.validate()?;
        }
        if let Some(background) = &self.background {
            background.validate("scene.background")?;
        }
        if let Some(size) = &self.size {
            size.validate()?;
        }
        Ok(())
    }

    pub(crate) fn fingerprints(&self) -> SceneFingerprints {
        let mut geometry = DefaultHasher::new();
        self.id.hash(&mut geometry);
        for child in &self.children {
            child.id().hash(&mut geometry);
            child.fingerprints().geometry.hash(&mut geometry);
        }

        let mut material = DefaultHasher::new();
        if let Some(background) = &self.background {
            background.hash_into(&mut material);
        }
        if let Some(size) = &self.size {
            size.hash_into(&mut material);
        }
        for child in &self.children {
            child.fingerprints().material.hash(&mut material);
        }

        let mut camera = DefaultHasher::new();
        self.camera.hash_into(&mut camera);
        self.interactions.hash(&mut camera);
        for child in &self.children {
            child.fingerprints().camera.hash(&mut camera);
        }

        SceneFingerprints {
            geometry: geometry.finish(),
            material: material.finish(),
            camera: camera.finish(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SceneFingerprints {
    pub geometry: u64,
    pub material: u64,
    pub camera: u64,
}

fn default_opacity() -> f32 {
    1.0
}

fn default_line_width() -> f32 {
    1.5
}

fn default_light_intensity() -> f32 {
    1.0
}

fn validate_id(id: &str, field: &'static str) -> Result<(), Scene3DError> {
    if id.trim().is_empty() {
        return Err(Scene3DError::InvalidData {
            field,
            reason: "must not be empty",
        });
    }
    Ok(())
}

fn validate_axis(
    values: &[f64],
    expected_len: usize,
    field: &'static str,
    expected_field: &'static str,
    log: bool,
) -> Result<(), Scene3DError> {
    if values.len() != expected_len {
        return Err(Scene3DError::DataLengthMismatch {
            x_field: field,
            y_field: expected_field,
            x_len: values.len(),
            y_len: expected_len,
        });
    }
    validate_finite_f64_slice(values, field)?;
    validate_monotonic(values, field)?;
    if log {
        validate_positive_f64_slice(values, field)?;
    }
    Ok(())
}

fn validate_finite_f32(value: f32, field: &'static str) -> Result<(), Scene3DError> {
    if !value.is_finite() {
        return Err(Scene3DError::InvalidData {
            field,
            reason: "contains NaN or Infinity",
        });
    }
    Ok(())
}

fn validate_positive_f32(value: f32, field: &'static str) -> Result<(), Scene3DError> {
    validate_finite_f32(value, field)?;
    if value <= 0.0 {
        return Err(Scene3DError::InvalidData {
            field,
            reason: "must be positive",
        });
    }
    Ok(())
}

fn validate_finite_f64_slice(values: &[f64], field: &'static str) -> Result<(), Scene3DError> {
    if values.is_empty() {
        return Err(Scene3DError::EmptyData { field });
    }
    if values.iter().any(|value| !value.is_finite()) {
        return Err(Scene3DError::InvalidData {
            field,
            reason: "contains NaN or Infinity",
        });
    }
    Ok(())
}

fn validate_positive_f64_slice(values: &[f64], field: &'static str) -> Result<(), Scene3DError> {
    if values.iter().any(|value| *value <= 0.0) {
        return Err(Scene3DError::InvalidData {
            field,
            reason: "contains non-positive values for log scale",
        });
    }
    Ok(())
}

fn validate_monotonic(values: &[f64], field: &'static str) -> Result<(), Scene3DError> {
    for window in values.windows(2) {
        if window[1] <= window[0] {
            return Err(Scene3DError::InvalidData {
                field,
                reason: "must be strictly monotonically increasing",
            });
        }
    }
    Ok(())
}

fn hash_f64_slice(values: &[f64], h: &mut impl Hasher) {
    values.len().hash(h);
    for value in values {
        value.to_bits().hash(h);
    }
}

fn hash_optional_f64_slice(values: &Option<Vec<f64>>, h: &mut impl Hasher) {
    values.is_some().hash(h);
    if let Some(values) = values {
        hash_f64_slice(values, h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_validates_grid_dimensions() {
        let spec = SurfaceSpec::from_flat("surface", vec![1.0, 2.0, 3.0], 2, 2);
        assert!(matches!(
            spec.validate(),
            Err(Scene3DError::GridDimensionMismatch {
                z_len: 3,
                width: 2,
                height: 2,
                expected: 4
            })
        ));
    }

    #[test]
    fn surface_validates_log_axis_positivity() {
        let mut spec = SurfaceSpec::from_flat("surface", vec![1.0, 2.0, 3.0, 4.0], 2, 2);
        spec.x = Some(vec![0.0, 10.0]);
        spec.x_log = true;

        assert!(matches!(
            spec.validate(),
            Err(Scene3DError::InvalidData {
                field: "x",
                reason: "contains non-positive values for log scale"
            })
        ));
    }

    #[test]
    fn surface_requires_monotonic_axes() {
        let mut spec = SurfaceSpec::from_flat("surface", vec![1.0, 2.0, 3.0, 4.0], 2, 2);
        spec.y = Some(vec![2.0, 1.0]);

        assert!(matches!(
            spec.validate(),
            Err(Scene3DError::InvalidData {
                field: "y",
                reason: "must be strictly monotonically increasing"
            })
        ));
    }

    #[test]
    fn surface_from_rows_flattens_row_major_data() {
        let spec = SurfaceSpec::from_rows("surface", vec![vec![1.0, 2.0], vec![3.0, 4.0]])
            .expect("valid surface");

        assert_eq!(spec.z.width, 2);
        assert_eq!(spec.z.height, 2);
        assert_eq!(spec.z.values, vec![1.0, 2.0, 3.0, 4.0]);
        assert_eq!(spec.z.rows(), vec![vec![1.0, 2.0], vec![3.0, 4.0]]);
    }

    #[test]
    fn mesh_rejects_invalid_indices() {
        let spec = MeshSpec {
            id: "mesh".to_string(),
            vertices: vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(0.0, 1.0, 0.0),
            ],
            indices: vec![0, 1, 3],
            material: MaterialSpec::default(),
        };

        assert!(matches!(
            spec.validate(),
            Err(Scene3DError::InvalidMeshIndex {
                position: 2,
                index: 3,
                vertex_count: 3
            })
        ));
    }

    #[test]
    fn line_strip_expands_to_segments() {
        let strip = LineStripSpec {
            id: "path".to_string(),
            points: vec![
                Point3::new(0.0, 0.0, 0.0),
                Point3::new(1.0, 0.0, 0.0),
                Point3::new(1.0, 1.0, 0.0),
            ],
            color: ColorRgba::from_rgb_u8(255, 255, 255),
            width: 2.0,
        };

        let segments = strip.to_segments();
        assert_eq!(segments.len(), 2);
        assert_eq!(segments[0].from, strip.points[0]);
        assert_eq!(segments[1].to, strip.points[2]);
    }

    #[test]
    fn serde_uses_snake_case_tags() {
        let camera = CameraSpec::Orbit(OrbitCameraSpec::new(3.5, 60.0, 25.0));
        let value = serde_json::to_value(camera).expect("camera json");

        assert_eq!(value["kind"], "orbit");
        assert_eq!(value["distance"], 3.5);
    }

    #[test]
    fn color_hex_parses_alpha() {
        let color = ColorRgba::from_hex("#33669980").expect("color");

        assert!((color.r - 0x33 as f32 / 255.0).abs() < f32::EPSILON);
        assert!((color.a - 0x80 as f32 / 255.0).abs() < f32::EPSILON);
    }
}
