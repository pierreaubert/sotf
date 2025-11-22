//
//  HeadScannerBridge.swift
//  HeadScanner iOS
//
//  Swift wrapper for Rust head-scanner FFI
//

import Foundation
import simd

/// Swift wrapper for the Rust head-scanner library
public class HeadScanner {
    private var scanner: OpaquePointer?

    public init() {
        scanner = scanner_new()
    }

    deinit {
        if let scanner = scanner {
            scanner_free(scanner)
        }
    }

    /// Process a camera frame
    public func processFrame(
        rgb: Data,
        depth: [Float],
        width: UInt32,
        height: UInt32,
        position: SIMD3<Float>,
        rotation: simd_quatf
    ) -> Bool {
        guard let scanner = scanner else { return false }

        let pose = CameraPose(
            position: Point3D(x: position.x, y: position.y, z: position.z),
            rotation: Quaternion(
                x: rotation.vector.x,
                y: rotation.vector.y,
                z: rotation.vector.z,
                w: rotation.vector.w
            )
        )

        return rgb.withUnsafeBytes { rgbPtr in
            depth.withUnsafeBufferPointer { depthPtr in
                var poseCopy = pose
                let result = scanner_process_frame(
                    scanner,
                    rgbPtr.baseAddress?.assumingMemoryBound(to: UInt8.self),
                    depthPtr.baseAddress,
                    width,
                    height,
                    &poseCopy
                )
                return result == SCANNER_OK
            }
        }
    }

    /// Get the reconstructed mesh
    public func getMesh() -> HeadMesh? {
        guard let scanner = scanner else { return nil }
        guard let mesh = scanner_get_mesh(scanner) else { return nil }
        return HeadMesh(mesh: mesh)
    }

    /// Get scan guidance
    public func getGuidance() -> ScanGuidanceWrapper? {
        guard let scanner = scanner else { return nil }
        guard let guidance = scanner_get_guidance(scanner) else { return nil }
        return ScanGuidanceWrapper(guidance: guidance)
    }

    /// Get last error message
    public static func lastError() -> String? {
        guard let cStr = scanner_last_error() else { return nil }
        return String(cString: cStr)
    }
}

/// Swift wrapper for scan guidance
public class ScanGuidanceWrapper {
    private var guidance: OpaquePointer?

    init(guidance: OpaquePointer) {
        self.guidance = guidance
    }

    deinit {
        if let guidance = guidance {
            guidance_free(guidance)
        }
    }

    /// Update with new camera pose
    public func updatePose(position: SIMD3<Float>, rotation: simd_quatf) {
        guard let guidance = guidance else { return }

        var pose = CameraPose(
            position: Point3D(x: position.x, y: position.y, z: position.z),
            rotation: Quaternion(
                x: rotation.vector.x,
                y: rotation.vector.y,
                z: rotation.vector.z,
                w: rotation.vector.w
            )
        )

        guidance_update_pose(guidance, &pose)
    }

    /// Get quality metrics
    public func getMetrics() -> QualityMetrics {
        guard let guidance = guidance else {
            return QualityMetrics(coverage: 0, angularCoverage: 0, pointDensity: 0, blurScore: 0)
        }

        let metrics = guidance_get_metrics(guidance)
        return QualityMetrics(
            coverage: metrics.coverage,
            angularCoverage: metrics.angular_coverage,
            pointDensity: metrics.point_density,
            blurScore: metrics.blur_score
        )
    }

    /// Check if region is covered
    public func isRegionCovered(_ region: HeadRegionEnum) -> Bool {
        guard let guidance = guidance else { return false }
        return guidance_is_region_covered(guidance, region.toCRegion())
    }

    /// Get next region to scan
    public func getNextRegion() -> HeadRegionEnum {
        guard let guidance = guidance else { return .front }
        return HeadRegionEnum.fromCRegion(guidance_get_next_region(guidance))
    }
}

/// Swift wrapper for mesh
public class HeadMesh {
    private var mesh: OpaquePointer?

    init(mesh: OpaquePointer) {
        self.mesh = mesh
    }

    deinit {
        if let mesh = mesh {
            mesh_free(mesh)
        }
    }

    /// Get vertex count
    public var vertexCount: UInt32 {
        guard let mesh = mesh else { return 0 }
        return mesh_vertex_count(mesh)
    }

    /// Get triangle count
    public var triangleCount: UInt32 {
        guard let mesh = mesh else { return 0 }
        return mesh_triangle_count(mesh)
    }

    /// Export mesh to OBJ file
    public func exportOBJ(to path: String) -> Bool {
        guard let mesh = mesh else { return false }
        return path.withCString { cPath in
            mesh_export_obj(mesh, cPath) == SCANNER_OK
        }
    }

    /// Generate SOFA file for HRTF
    public func generateSOFA(
        to path: String,
        sampleRate: Float = 44100.0,
        azimuthResolution: UInt32 = 360,
        elevationResolution: UInt32 = 180,
        distance: Float = 1.0
    ) -> Bool {
        guard let mesh = mesh else { return false }
        return path.withCString { cPath in
            scanner_generate_sofa(
                mesh,
                cPath,
                sampleRate,
                azimuthResolution,
                elevationResolution,
                distance
            ) == SCANNER_OK
        }
    }
}

/// Quality metrics
public struct QualityMetrics {
    public let coverage: Float
    public let angularCoverage: Float
    public let pointDensity: Float
    public let blurScore: Float

    /// Overall quality score (0-100)
    public var overallScore: Float {
        return (coverage * 40 + angularCoverage * 30 + pointDensity * 20 + blurScore * 10)
    }

    /// Check if scan is complete (>90% coverage)
    public var isComplete: Bool {
        return coverage > 0.9
    }
}

/// Head regions
public enum HeadRegionEnum: CaseIterable {
    case front
    case left
    case right
    case back
    case top
    case bottom
    case frontLeft
    case frontRight
    case backLeft
    case backRight
    case topFront

    func toCRegion() -> HeadRegion {
        switch self {
        case .front: return REGION_FRONT
        case .left: return REGION_LEFT
        case .right: return REGION_RIGHT
        case .back: return REGION_BACK
        case .top: return REGION_TOP
        case .bottom: return REGION_BOTTOM
        case .frontLeft: return REGION_FRONT_LEFT
        case .frontRight: return REGION_FRONT_RIGHT
        case .backLeft: return REGION_BACK_LEFT
        case .backRight: return REGION_BACK_RIGHT
        case .topFront: return REGION_TOP_FRONT
        }
    }

    static func fromCRegion(_ region: HeadRegion) -> HeadRegionEnum {
        switch region {
        case REGION_FRONT: return .front
        case REGION_LEFT: return .left
        case REGION_RIGHT: return .right
        case REGION_BACK: return .back
        case REGION_TOP: return .top
        case REGION_BOTTOM: return .bottom
        case REGION_FRONT_LEFT: return .frontLeft
        case REGION_FRONT_RIGHT: return .frontRight
        case REGION_BACK_LEFT: return .backLeft
        case REGION_BACK_RIGHT: return .backRight
        case REGION_TOP_FRONT: return .topFront
        default: return .front
        }
    }

    /// Human-readable name
    public var displayName: String {
        switch self {
        case .front: return "Front"
        case .left: return "Left"
        case .right: return "Right"
        case .back: return "Back"
        case .top: return "Top"
        case .bottom: return "Bottom"
        case .frontLeft: return "Front Left"
        case .frontRight: return "Front Right"
        case .backLeft: return "Back Left"
        case .backRight: return "Back Right"
        case .topFront: return "Top Front"
        }
    }
}
