//! FFI bindings for iOS/Swift integration
//!
//! This module provides C-compatible FFI functions that can be called from Swift.

use crate::ScannerResult;
use crate::guidance::{HeadRegion, QualityMetrics, ScanGuidance};
use crate::mesh::Mesh;
use crate::scanner::Scanner;
use nalgebra::{Point3, Quaternion as NalgebraQuaternion, UnitQuaternion};
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;

/// Result codes for FFI functions
#[repr(C)]
pub enum ScannerResultCode {
    Ok = 0,
    Error = 1,
    InvalidInput = 2,
    IoError = 3,
}

/// 3D point
#[repr(C)]
pub struct Point3D {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

impl From<Point3D> for Point3<f32> {
    fn from(p: Point3D) -> Self {
        Point3::new(p.x, p.y, p.z)
    }
}

/// Quaternion for rotation
#[repr(C)]
pub struct Quaternion {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub w: f32,
}

/// Camera pose (position + orientation)
#[repr(C)]
pub struct CameraPose {
    pub position: Point3D,
    pub rotation: Quaternion,
}

/// Quality metrics (C-compatible)
#[repr(C)]
pub struct QualityMetricsC {
    pub coverage: f32,
    pub angular_coverage: f32,
    pub point_density: f32,
    pub blur_score: f32,
}

impl From<&QualityMetrics> for QualityMetricsC {
    fn from(metrics: &QualityMetrics) -> Self {
        QualityMetricsC {
            coverage: metrics.coverage_percentage,
            angular_coverage: metrics.angular_coverage,
            point_density: metrics.point_density,
            blur_score: metrics.blur_score,
        }
    }
}

/// Head region enum (C-compatible)
#[repr(C)]
pub enum HeadRegionC {
    Front = 0,
    Left = 1,
    Right = 2,
    Back = 3,
    Top = 4,
    FrontLeft = 5,
    FrontRight = 6,
    BackLeft = 7,
    BackRight = 8,
    TopLeft = 9,
    TopRight = 10,
}

impl From<HeadRegion> for HeadRegionC {
    fn from(region: HeadRegion) -> Self {
        match region {
            HeadRegion::Front => HeadRegionC::Front,
            HeadRegion::Left => HeadRegionC::Left,
            HeadRegion::Right => HeadRegionC::Right,
            HeadRegion::Back => HeadRegionC::Back,
            HeadRegion::Top => HeadRegionC::Top,
            HeadRegion::FrontLeft => HeadRegionC::FrontLeft,
            HeadRegion::FrontRight => HeadRegionC::FrontRight,
            HeadRegion::BackLeft => HeadRegionC::BackLeft,
            HeadRegion::BackRight => HeadRegionC::BackRight,
            HeadRegion::TopLeft => HeadRegionC::TopLeft,
            HeadRegion::TopRight => HeadRegionC::TopRight,
        }
    }
}

impl From<HeadRegionC> for HeadRegion {
    fn from(region: HeadRegionC) -> Self {
        match region {
            HeadRegionC::Front => HeadRegion::Front,
            HeadRegionC::Left => HeadRegion::Left,
            HeadRegionC::Right => HeadRegion::Right,
            HeadRegionC::Back => HeadRegion::Back,
            HeadRegionC::Top => HeadRegion::Top,
            HeadRegionC::FrontLeft => HeadRegion::FrontLeft,
            HeadRegionC::FrontRight => HeadRegion::FrontRight,
            HeadRegionC::BackLeft => HeadRegion::BackLeft,
            HeadRegionC::BackRight => HeadRegion::BackRight,
            HeadRegionC::TopLeft => HeadRegion::TopLeft,
            HeadRegionC::TopRight => HeadRegion::TopRight,
        }
    }
}

thread_local! {
    static LAST_ERROR: std::cell::RefCell<Option<CString>> = std::cell::RefCell::new(None);
}

fn set_last_error(err: String) {
    LAST_ERROR.with(|e| {
        *e.borrow_mut() = CString::new(err).ok();
    });
}

/// Scanner lifecycle

#[unsafe(no_mangle)]
pub extern "C" fn scanner_new() -> *mut Scanner {
    Box::into_raw(Box::new(Scanner::new()))
}

#[unsafe(no_mangle)]
pub extern "C" fn scanner_free(scanner: *mut Scanner) {
    if !scanner.is_null() {
        unsafe {
            drop(Box::from_raw(scanner));
        }
    }
}

/// Frame processing

#[unsafe(no_mangle)]
pub extern "C" fn scanner_process_frame(
    scanner: *mut Scanner,
    rgb_data: *const u8,
    depth_data: *const f32,
    width: u32,
    height: u32,
    pose: *const CameraPose,
) -> ScannerResultCode {
    // Validate pointers
    if scanner.is_null() || rgb_data.is_null() || depth_data.is_null() || pose.is_null() {
        set_last_error("Null pointer provided".to_string());
        return ScannerResultCode::InvalidInput;
    }

    // Validate dimensions to prevent integer overflow and excessive memory access
    const MAX_DIMENSION: u32 = 16384; // 16K resolution max
    const MIN_DIMENSION: u32 = 64; // Minimum reasonable resolution

    if width < MIN_DIMENSION || width > MAX_DIMENSION {
        set_last_error(format!(
            "Invalid width: {} (must be {}-{})",
            width, MIN_DIMENSION, MAX_DIMENSION
        ));
        return ScannerResultCode::InvalidInput;
    }

    if height < MIN_DIMENSION || height > MAX_DIMENSION {
        set_last_error(format!(
            "Invalid height: {} (must be {}-{})",
            height, MIN_DIMENSION, MAX_DIMENSION
        ));
        return ScannerResultCode::InvalidInput;
    }

    // Check for potential integer overflow in buffer size calculation
    let pixel_count = match (width as u64).checked_mul(height as u64) {
        Some(count) if count <= (usize::MAX as u64) => count as usize,
        _ => {
            set_last_error(format!(
                "Dimensions too large: {}x{} would overflow",
                width, height
            ));
            return ScannerResultCode::InvalidInput;
        }
    };

    let rgb_size = match pixel_count.checked_mul(3) {
        Some(size) => size,
        None => {
            set_last_error("RGB buffer size would overflow".to_string());
            return ScannerResultCode::InvalidInput;
        }
    };

    let scanner = unsafe { &mut *scanner };
    let pose = unsafe { &*pose };

    // Validate pose values are not NaN or infinity
    if !pose.position.x.is_finite() || !pose.position.y.is_finite() || !pose.position.z.is_finite()
    {
        set_last_error("Invalid position: contains NaN or infinity".to_string());
        return ScannerResultCode::InvalidInput;
    }

    if !pose.rotation.x.is_finite()
        || !pose.rotation.y.is_finite()
        || !pose.rotation.z.is_finite()
        || !pose.rotation.w.is_finite()
    {
        set_last_error("Invalid rotation: contains NaN or infinity".to_string());
        return ScannerResultCode::InvalidInput;
    }

    // Check quaternion is normalized (within tolerance)
    let quat_len_sq = pose.rotation.x * pose.rotation.x
        + pose.rotation.y * pose.rotation.y
        + pose.rotation.z * pose.rotation.z
        + pose.rotation.w * pose.rotation.w;
    if (quat_len_sq - 1.0).abs() > 0.01 {
        set_last_error(format!(
            "Quaternion not normalized: length^2 = {}",
            quat_len_sq
        ));
        return ScannerResultCode::InvalidInput;
    }

    // Convert RGB data (now with validated size)
    let rgb = unsafe { std::slice::from_raw_parts(rgb_data, rgb_size) };

    // Convert depth data
    let depth = unsafe { std::slice::from_raw_parts(depth_data, pixel_count) };

    // Validate depth data (quick spot check to prevent garbage data)
    const MAX_DEPTH: f32 = 100.0; // 100 meters maximum
    let mut valid_depth_count = 0;
    for (i, &d) in depth.iter().enumerate().take(100) {
        // Check first 100 samples
        if d.is_finite() && d >= 0.0 && d <= MAX_DEPTH {
            valid_depth_count += 1;
        }
    }
    if valid_depth_count == 0 {
        set_last_error("Depth data appears invalid (all samples out of range or NaN)".to_string());
        return ScannerResultCode::InvalidInput;
    }

    // Convert pose
    let position = Point3::new(pose.position.x, pose.position.y, pose.position.z);
    let rotation = UnitQuaternion::from_quaternion(NalgebraQuaternion::new(
        pose.rotation.w,
        pose.rotation.x,
        pose.rotation.y,
        pose.rotation.z,
    ));

    // Process frame (simplified - actual implementation would use the scanner's methods)
    match scanner.process_frame(rgb, depth, width, height, position, rotation) {
        Ok(_) => ScannerResultCode::Ok,
        Err(e) => {
            set_last_error(format!("{:?}", e));
            ScannerResultCode::Error
        }
    }
}

/// Scan guidance

#[unsafe(no_mangle)]
pub extern "C" fn scanner_get_guidance(scanner: *mut Scanner) -> *mut ScanGuidance {
    if scanner.is_null() {
        return ptr::null_mut();
    }

    let scanner = unsafe { &mut *scanner };
    Box::into_raw(Box::new(ScanGuidance::new()))
}

#[unsafe(no_mangle)]
pub extern "C" fn guidance_free(guidance: *mut ScanGuidance) {
    if !guidance.is_null() {
        unsafe {
            drop(Box::from_raw(guidance));
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn guidance_update_pose(
    guidance: *mut ScanGuidance,
    pose: *const CameraPose,
) -> ScannerResultCode {
    if guidance.is_null() || pose.is_null() {
        return ScannerResultCode::InvalidInput;
    }

    let guidance = unsafe { &mut *guidance };
    let pose = unsafe { &*pose };

    let position = Point3::new(pose.position.x, pose.position.y, pose.position.z);
    let rotation = UnitQuaternion::from_quaternion(NalgebraQuaternion::new(
        pose.rotation.w,
        pose.rotation.x,
        pose.rotation.y,
        pose.rotation.z,
    ));

    guidance.update_pose(position, rotation);
    ScannerResultCode::Ok
}

#[unsafe(no_mangle)]
pub extern "C" fn guidance_get_metrics(guidance: *const ScanGuidance) -> QualityMetricsC {
    if guidance.is_null() {
        return QualityMetricsC {
            coverage: 0.0,
            angular_coverage: 0.0,
            point_density: 0.0,
            blur_score: 0.0,
        };
    }

    let guidance = unsafe { &*guidance };
    QualityMetricsC::from(guidance.get_quality_metrics())
}

#[unsafe(no_mangle)]
pub extern "C" fn guidance_is_region_covered(
    guidance: *const ScanGuidance,
    region: HeadRegionC,
) -> bool {
    if guidance.is_null() {
        return false;
    }

    let guidance = unsafe { &*guidance };
    guidance
        .get_covered_regions()
        .contains(&HeadRegion::from(region))
}

#[unsafe(no_mangle)]
pub extern "C" fn guidance_get_next_region(guidance: *const ScanGuidance) -> HeadRegionC {
    if guidance.is_null() {
        return HeadRegionC::Front;
    }

    let guidance = unsafe { &*guidance };
    HeadRegionC::from(guidance.get_next_region())
}

/// Mesh reconstruction

#[unsafe(no_mangle)]
pub extern "C" fn scanner_get_mesh(scanner: *mut Scanner) -> *mut Mesh {
    if scanner.is_null() {
        return ptr::null_mut();
    }

    let scanner = unsafe { &mut *scanner };
    match scanner.get_mesh() {
        Ok(mesh) => Box::into_raw(Box::new(mesh)),
        Err(e) => {
            set_last_error(format!("{:?}", e));
            ptr::null_mut()
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mesh_free(mesh: *mut Mesh) {
    if !mesh.is_null() {
        unsafe {
            drop(Box::from_raw(mesh));
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn mesh_vertex_count(mesh: *const Mesh) -> u32 {
    if mesh.is_null() {
        return 0;
    }

    let mesh = unsafe { &*mesh };
    mesh.vertex_count() as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn mesh_triangle_count(mesh: *const Mesh) -> u32 {
    if mesh.is_null() {
        return 0;
    }

    let mesh = unsafe { &*mesh };
    mesh.triangle_count() as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn mesh_export_obj(mesh: *const Mesh, path: *const c_char) -> ScannerResultCode {
    if mesh.is_null() || path.is_null() {
        set_last_error("Null pointer provided to mesh_export_obj".to_string());
        return ScannerResultCode::InvalidInput;
    }

    let mesh = unsafe { &*mesh };
    let path_str = unsafe {
        match CStr::from_ptr(path).to_str() {
            Ok(s) => s,
            Err(_) => {
                set_last_error("Invalid UTF-8 in path".to_string());
                return ScannerResultCode::InvalidInput;
            }
        }
    };

    // Validate path length
    const MAX_PATH_LENGTH: usize = 4096;
    if path_str.len() > MAX_PATH_LENGTH {
        set_last_error(format!(
            "Path too long: {} bytes (max {})",
            path_str.len(),
            MAX_PATH_LENGTH
        ));
        return ScannerResultCode::InvalidInput;
    }

    // Check for null bytes in path (security)
    if path_str.contains('\0') {
        set_last_error("Null byte in path".to_string());
        return ScannerResultCode::InvalidInput;
    }

    match mesh.export_obj(path_str) {
        Ok(_) => ScannerResultCode::Ok,
        Err(e) => {
            set_last_error(format!("{:?}", e));
            ScannerResultCode::IoError
        }
    }
}

/// SOFA generation

#[unsafe(no_mangle)]
pub extern "C" fn scanner_generate_sofa(
    mesh: *const Mesh,
    output_path: *const c_char,
    sample_rate: f32,
    azimuth_resolution: u32,
    elevation_resolution: u32,
    distance: f32,
) -> ScannerResultCode {
    // Validate pointers
    if mesh.is_null() || output_path.is_null() {
        set_last_error("Null pointer provided to scanner_generate_sofa".to_string());
        return ScannerResultCode::InvalidInput;
    }

    let mesh = unsafe { &*mesh };
    let path_str = unsafe {
        match CStr::from_ptr(output_path).to_str() {
            Ok(s) => s,
            Err(_) => {
                set_last_error("Invalid UTF-8 in output path".to_string());
                return ScannerResultCode::InvalidInput;
            }
        }
    };

    // Validate path
    const MAX_PATH_LENGTH: usize = 4096;
    if path_str.len() > MAX_PATH_LENGTH {
        set_last_error(format!(
            "Path too long: {} bytes (max {})",
            path_str.len(),
            MAX_PATH_LENGTH
        ));
        return ScannerResultCode::InvalidInput;
    }

    if path_str.contains('\0') {
        set_last_error("Null byte in path".to_string());
        return ScannerResultCode::InvalidInput;
    }

    // Validate sample rate
    if !sample_rate.is_finite() || sample_rate <= 0.0 {
        set_last_error(format!("Invalid sample rate: {}", sample_rate));
        return ScannerResultCode::InvalidInput;
    }

    const MIN_SAMPLE_RATE: f32 = 8000.0; // 8 kHz minimum
    const MAX_SAMPLE_RATE: f32 = 192000.0; // 192 kHz maximum

    if sample_rate < MIN_SAMPLE_RATE || sample_rate > MAX_SAMPLE_RATE {
        set_last_error(format!(
            "Sample rate out of range: {} (must be {}-{})",
            sample_rate, MIN_SAMPLE_RATE, MAX_SAMPLE_RATE
        ));
        return ScannerResultCode::InvalidInput;
    }

    // Validate resolutions
    const MIN_RESOLUTION: u32 = 1;
    const MAX_RESOLUTION: u32 = 3600; // 0.1 degree resolution max

    if azimuth_resolution < MIN_RESOLUTION || azimuth_resolution > MAX_RESOLUTION {
        set_last_error(format!(
            "Azimuth resolution out of range: {} (must be {}-{})",
            azimuth_resolution, MIN_RESOLUTION, MAX_RESOLUTION
        ));
        return ScannerResultCode::InvalidInput;
    }

    if elevation_resolution < MIN_RESOLUTION || elevation_resolution > MAX_RESOLUTION {
        set_last_error(format!(
            "Elevation resolution out of range: {} (must be {}-{})",
            elevation_resolution, MIN_RESOLUTION, MAX_RESOLUTION
        ));
        return ScannerResultCode::InvalidInput;
    }

    // Validate distance
    if !distance.is_finite() || distance <= 0.0 {
        set_last_error(format!("Invalid distance: {}", distance));
        return ScannerResultCode::InvalidInput;
    }

    const MIN_DISTANCE: f32 = 0.1; // 10 cm minimum
    const MAX_DISTANCE: f32 = 100.0; // 100 m maximum

    if distance < MIN_DISTANCE || distance > MAX_DISTANCE {
        set_last_error(format!(
            "Distance out of range: {} (must be {}-{})",
            distance, MIN_DISTANCE, MAX_DISTANCE
        ));
        return ScannerResultCode::InvalidInput;
    }

    #[cfg(feature = "sofa")]
    {
        match crate::acoustics::generate_sofa_analytical(
            mesh,
            path_str,
            sample_rate,
            azimuth_resolution as usize,
            elevation_resolution as usize,
            distance,
        ) {
            Ok(_) => ScannerResultCode::Ok,
            Err(e) => {
                set_last_error(format!("{:?}", e));
                ScannerResultCode::Error
            }
        }
    }

    #[cfg(not(feature = "sofa"))]
    {
        set_last_error("SOFA support not enabled".to_string());
        ScannerResultCode::Error
    }
}

/// Error handling

#[unsafe(no_mangle)]
pub extern "C" fn scanner_last_error() -> *const c_char {
    LAST_ERROR.with(|e| {
        e.borrow()
            .as_ref()
            .map(|s| s.as_ptr())
            .unwrap_or(ptr::null())
    })
}

// Scanner::process_frame and Scanner::get_mesh are now implemented in scanner.rs
