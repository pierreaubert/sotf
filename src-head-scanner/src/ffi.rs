//! FFI bindings for iOS/Swift integration
//!
//! This module provides C-compatible FFI functions that can be called from Swift.

use crate::guidance::{HeadRegion, QualityMetrics, ScanGuidance};
use crate::mesh::Mesh;
use crate::scanner::Scanner;
use crate::ScannerResult;
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
            coverage: metrics.coverage,
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
    Bottom = 5,
    FrontLeft = 6,
    FrontRight = 7,
    BackLeft = 8,
    BackRight = 9,
    TopFront = 10,
}

impl From<HeadRegion> for HeadRegionC {
    fn from(region: HeadRegion) -> Self {
        match region {
            HeadRegion::Front => HeadRegionC::Front,
            HeadRegion::Left => HeadRegionC::Left,
            HeadRegion::Right => HeadRegionC::Right,
            HeadRegion::Back => HeadRegionC::Back,
            HeadRegion::Top => HeadRegionC::Top,
            HeadRegion::Bottom => HeadRegionC::Bottom,
            HeadRegion::FrontLeft => HeadRegionC::FrontLeft,
            HeadRegion::FrontRight => HeadRegionC::FrontRight,
            HeadRegion::BackLeft => HeadRegionC::BackLeft,
            HeadRegion::BackRight => HeadRegionC::BackRight,
            HeadRegion::TopFront => HeadRegionC::TopFront,
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
            HeadRegionC::Bottom => HeadRegion::Bottom,
            HeadRegionC::FrontLeft => HeadRegion::FrontLeft,
            HeadRegionC::FrontRight => HeadRegion::FrontRight,
            HeadRegionC::BackLeft => HeadRegion::BackLeft,
            HeadRegionC::BackRight => HeadRegion::BackRight,
            HeadRegionC::TopFront => HeadRegion::TopFront,
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

#[no_mangle]
pub extern "C" fn scanner_new() -> *mut Scanner {
    Box::into_raw(Box::new(Scanner::new()))
}

#[no_mangle]
pub extern "C" fn scanner_free(scanner: *mut Scanner) {
    if !scanner.is_null() {
        unsafe {
            drop(Box::from_raw(scanner));
        }
    }
}

/// Frame processing

#[no_mangle]
pub extern "C" fn scanner_process_frame(
    scanner: *mut Scanner,
    rgb_data: *const u8,
    depth_data: *const f32,
    width: u32,
    height: u32,
    pose: *const CameraPose,
) -> ScannerResultCode {
    if scanner.is_null() || rgb_data.is_null() || depth_data.is_null() || pose.is_null() {
        set_last_error("Null pointer provided".to_string());
        return ScannerResultCode::InvalidInput;
    }

    let scanner = unsafe { &mut *scanner };
    let pose = unsafe { &*pose };

    // Convert RGB data
    let rgb_size = (width * height * 3) as usize;
    let rgb = unsafe { std::slice::from_raw_parts(rgb_data, rgb_size) };

    // Convert depth data
    let depth_size = (width * height) as usize;
    let depth = unsafe { std::slice::from_raw_parts(depth_data, depth_size) };

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

#[no_mangle]
pub extern "C" fn scanner_get_guidance(scanner: *mut Scanner) -> *mut ScanGuidance {
    if scanner.is_null() {
        return ptr::null_mut();
    }

    let scanner = unsafe { &mut *scanner };
    Box::into_raw(Box::new(ScanGuidance::new()))
}

#[no_mangle]
pub extern "C" fn guidance_free(guidance: *mut ScanGuidance) {
    if !guidance.is_null() {
        unsafe {
            drop(Box::from_raw(guidance));
        }
    }
}

#[no_mangle]
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

#[no_mangle]
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
    QualityMetricsC::from(&guidance.quality_metrics)
}

#[no_mangle]
pub extern "C" fn guidance_is_region_covered(
    guidance: *const ScanGuidance,
    region: HeadRegionC,
) -> bool {
    if guidance.is_null() {
        return false;
    }

    let guidance = unsafe { &*guidance };
    guidance.covered_regions.contains(&HeadRegion::from(region))
}

#[no_mangle]
pub extern "C" fn guidance_get_next_region(guidance: *const ScanGuidance) -> HeadRegionC {
    if guidance.is_null() {
        return HeadRegionC::Front;
    }

    let guidance = unsafe { &*guidance };
    HeadRegionC::from(guidance.get_next_region())
}

/// Mesh reconstruction

#[no_mangle]
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

#[no_mangle]
pub extern "C" fn mesh_free(mesh: *mut Mesh) {
    if !mesh.is_null() {
        unsafe {
            drop(Box::from_raw(mesh));
        }
    }
}

#[no_mangle]
pub extern "C" fn mesh_vertex_count(mesh: *const Mesh) -> u32 {
    if mesh.is_null() {
        return 0;
    }

    let mesh = unsafe { &*mesh };
    mesh.vertex_count() as u32
}

#[no_mangle]
pub extern "C" fn mesh_triangle_count(mesh: *const Mesh) -> u32 {
    if mesh.is_null() {
        return 0;
    }

    let mesh = unsafe & *mesh };
    mesh.triangle_count() as u32
}

#[no_mangle]
pub extern "C" fn mesh_export_obj(mesh: *const Mesh, path: *const c_char) -> ScannerResultCode {
    if mesh.is_null() || path.is_null() {
        return ScannerResultCode::InvalidInput;
    }

    let mesh = unsafe { &*mesh };
    let path = unsafe {
        match CStr::from_ptr(path).to_str() {
            Ok(s) => s,
            Err(_) => {
                set_last_error("Invalid UTF-8 in path".to_string());
                return ScannerResultCode::InvalidInput;
            }
        }
    };

    match mesh.export_obj(path) {
        Ok(_) => ScannerResultCode::Ok,
        Err(e) => {
            set_last_error(format!("{:?}", e));
            ScannerResultCode::IoError
        }
    }
}

/// SOFA generation

#[no_mangle]
pub extern "C" fn scanner_generate_sofa(
    mesh: *const Mesh,
    output_path: *const c_char,
    sample_rate: f32,
    azimuth_resolution: u32,
    elevation_resolution: u32,
    distance: f32,
) -> ScannerResultCode {
    if mesh.is_null() || output_path.is_null() {
        return ScannerResultCode::InvalidInput;
    }

    let mesh = unsafe { &*mesh };
    let path = unsafe {
        match CStr::from_ptr(output_path).to_str() {
            Ok(s) => s,
            Err(_) => {
                set_last_error("Invalid UTF-8 in path".to_string());
                return ScannerResultCode::InvalidInput;
            }
        }
    };

    #[cfg(feature = "sofa")]
    {
        match crate::acoustics::generate_sofa_analytical(
            mesh,
            path,
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

#[no_mangle]
pub extern "C" fn scanner_last_error() -> *const c_char {
    LAST_ERROR.with(|e| {
        e.borrow()
            .as_ref()
            .map(|s| s.as_ptr())
            .unwrap_or(ptr::null())
    })
}

// Note: Scanner::process_frame method needs to be added to scanner.rs
// This is a simplified interface for the FFI
impl Scanner {
    pub fn process_frame(
        &mut self,
        _rgb: &[u8],
        _depth: &[f32],
        _width: u32,
        _height: u32,
        _position: Point3<f32>,
        _rotation: UnitQuaternion<f32>,
    ) -> ScannerResult<()> {
        // This would integrate with the existing scanner pipeline
        // For now, return Ok as placeholder
        Ok(())
    }

    pub fn get_mesh(&self) -> ScannerResult<Mesh> {
        // This would return the reconstructed mesh
        // For now, return a placeholder
        Ok(Mesh::default())
    }
}
