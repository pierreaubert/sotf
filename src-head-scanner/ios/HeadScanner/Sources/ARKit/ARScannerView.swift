//
//  ARScannerView.swift
//  HeadScanner iOS
//
//  ARKit integration for 3D scanning
//

import SwiftUI
import ARKit
import RealityKit

struct ARScannerView: UIViewRepresentable {
    @ObservedObject var viewModel: ScannerViewModel

    func makeUIView(context: Context) -> ARView {
        let arView = ARView(frame: .zero)

        // Configure AR session
        let configuration = ARWorldTrackingConfiguration()

        // Enable people occlusion and depth if available
        if ARWorldTrackingConfiguration.supportsFrameSemantics(.sceneDepth) {
            configuration.frameSemantics.insert(.sceneDepth)
        }

        if ARWorldTrackingConfiguration.supportsFrameSemantics(.smoothedSceneDepth) {
            configuration.frameSemantics.insert(.smoothedSceneDepth)
        }

        // Enable face tracking if available (for better head detection)
        configuration.isAutoFocusEnabled = true

        // Set delegate
        context.coordinator.arView = arView
        arView.session.delegate = context.coordinator

        // Start session
        arView.session.run(configuration)

        return arView
    }

    func updateUIView(_ uiView: ARView, context: Context) {
        // Update view when needed
    }

    func makeCoordinator() -> Coordinator {
        Coordinator(viewModel: viewModel)
    }

    class Coordinator: NSObject, ARSessionDelegate {
        let viewModel: ScannerViewModel
        weak var arView: ARView?

        init(viewModel: ScannerViewModel) {
            self.viewModel = viewModel
        }

        func session(_ session: ARSession, didUpdate frame: ARFrame) {
            guard viewModel.isScanning else { return }

            // Extract camera pose
            let transform = frame.camera.transform
            let position = SIMD3<Float>(transform.columns.3.x, transform.columns.3.y, transform.columns.3.z)
            let rotation = simd_quatf(transform)

            // Get RGB image
            let pixelBuffer = frame.capturedImage
            guard let rgbData = extractRGBData(from: pixelBuffer) else { return }

            // Get depth data if available
            var depthData: [Float]?
            if let depthMap = frame.sceneDepth?.depthMap {
                depthData = extractDepthData(from: depthMap)
            } else if let smoothedDepth = frame.smoothedSceneDepth?.depthMap {
                depthData = extractDepthData(from: smoothedDepth)
            }

            // Fallback: use estimated depth if no depth sensor
            if depthData == nil {
                depthData = estimateDepthFromFeatures(frame: frame)
            }

            guard let depth = depthData else { return }

            // Process frame through Rust scanner
            viewModel.processFrame(
                rgb: rgbData,
                depth: depth,
                width: UInt32(CVPixelBufferGetWidth(pixelBuffer)),
                height: UInt32(CVPixelBufferGetHeight(pixelBuffer)),
                position: position,
                rotation: rotation
            )
        }

        func session(_ session: ARSession, didFailWithError error: Error) {
            viewModel.handleError("AR Session failed: \(error.localizedDescription)")
        }

        // MARK: - Data Extraction

        private func extractRGBData(from pixelBuffer: CVPixelBuffer) -> Data? {
            CVPixelBufferLockBaseAddress(pixelBuffer, .readOnly)
            defer { CVPixelBufferUnlockBaseAddress(pixelBuffer, .readOnly) }

            let width = CVPixelBufferGetWidth(pixelBuffer)
            let height = CVPixelBufferGetHeight(pixelBuffer)
            let bytesPerRow = CVPixelBufferGetBytesPerRow(pixelBuffer)

            guard let baseAddress = CVPixelBufferGetBaseAddress(pixelBuffer) else {
                return nil
            }

            // Convert YCbCr to RGB if needed
            let pixelFormat = CVPixelBufferGetPixelFormatType(pixelBuffer)

            if pixelFormat == kCVPixelFormatType_420YpCbCr8BiPlanarFullRange ||
               pixelFormat == kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange {
                return convertYCbCrToRGB(pixelBuffer: pixelBuffer, width: width, height: height)
            } else if pixelFormat == kCVPixelFormatType_32BGRA {
                return convertBGRAToRGB(baseAddress: baseAddress, width: width, height: height, bytesPerRow: bytesPerRow)
            }

            return nil
        }

        private func convertYCbCrToRGB(pixelBuffer: CVPixelBuffer, width: Int, height: Int) -> Data? {
            // YCbCr to RGB conversion
            var rgbData = Data(count: width * height * 3)

            CVPixelBufferLockBaseAddress(pixelBuffer, .readOnly)
            defer { CVPixelBufferUnlockBaseAddress(pixelBuffer, .readOnly) }

            guard let yPlane = CVPixelBufferGetBaseAddressOfPlane(pixelBuffer, 0),
                  let cbcrPlane = CVPixelBufferGetBaseAddressOfPlane(pixelBuffer, 1) else {
                return nil
            }

            let yBuffer = yPlane.assumingMemoryBound(to: UInt8.self)
            let cbcrBuffer = cbcrPlane.assumingMemoryBound(to: UInt8.self)

            rgbData.withUnsafeMutableBytes { rgbPtr in
                guard let rgb = rgbPtr.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return }

                for y in 0..<height {
                    for x in 0..<width {
                        let yIndex = y * width + x
                        let uvIndex = (y / 2) * (width / 2) + (x / 2)

                        let yValue = Float(yBuffer[yIndex])
                        let cbValue = Float(cbcrBuffer[uvIndex * 2]) - 128.0
                        let crValue = Float(cbcrBuffer[uvIndex * 2 + 1]) - 128.0

                        // YCbCr to RGB conversion
                        var r = yValue + 1.402 * crValue
                        var g = yValue - 0.344 * cbValue - 0.714 * crValue
                        var b = yValue + 1.772 * cbValue

                        r = max(0, min(255, r))
                        g = max(0, min(255, g))
                        b = max(0, min(255, b))

                        let rgbIndex = yIndex * 3
                        rgb[rgbIndex] = UInt8(r)
                        rgb[rgbIndex + 1] = UInt8(g)
                        rgb[rgbIndex + 2] = UInt8(b)
                    }
                }
            }

            return rgbData
        }

        private func convertBGRAToRGB(baseAddress: UnsafeMutableRawPointer, width: Int, height: Int, bytesPerRow: Int) -> Data {
            var rgbData = Data(count: width * height * 3)

            rgbData.withUnsafeMutableBytes { rgbPtr in
                guard let rgb = rgbPtr.baseAddress?.assumingMemoryBound(to: UInt8.self) else { return }

                for y in 0..<height {
                    let rowStart = baseAddress.advanced(by: y * bytesPerRow).assumingMemoryBound(to: UInt8.self)

                    for x in 0..<width {
                        let pixelIndex = x * 4
                        let outputIndex = (y * width + x) * 3

                        // BGRA -> RGB
                        rgb[outputIndex] = rowStart[pixelIndex + 2]     // R
                        rgb[outputIndex + 1] = rowStart[pixelIndex + 1] // G
                        rgb[outputIndex + 2] = rowStart[pixelIndex]     // B
                    }
                }
            }

            return rgbData
        }

        private func extractDepthData(from depthMap: CVPixelBuffer) -> [Float]? {
            CVPixelBufferLockBaseAddress(depthMap, .readOnly)
            defer { CVPixelBufferUnlockBaseAddress(depthMap, .readOnly) }

            let width = CVPixelBufferGetWidth(depthMap)
            let height = CVPixelBufferGetHeight(depthMap)

            guard let baseAddress = CVPixelBufferGetBaseAddress(depthMap) else {
                return nil
            }

            let depthBuffer = baseAddress.assumingMemoryBound(to: Float32.self)
            var depthArray = [Float](repeating: 0, count: width * height)

            for i in 0..<(width * height) {
                depthArray[i] = depthBuffer[i]
            }

            return depthArray
        }

        private func estimateDepthFromFeatures(frame: ARFrame) -> [Float]? {
            // Fallback: estimate depth from feature points and camera transform
            let width = CVPixelBufferGetWidth(frame.capturedImage)
            let height = CVPixelBufferGetHeight(frame.capturedImage)

            // Create depth map with estimated values
            var depthArray = [Float](repeating: 1.0, count: width * height) // Default 1m

            // Use feature points to improve depth estimation
            for point in frame.rawFeaturePoints?.points ?? [] {
                let worldPos = point
                let cameraPos = frame.camera.transform.columns.3

                // Calculate distance from camera
                let dx = worldPos.x - cameraPos.x
                let dy = worldPos.y - cameraPos.y
                let dz = worldPos.z - cameraPos.z
                let distance = sqrt(dx * dx + dy * dy + dz * dz)

                // Project to image coordinates (simplified)
                let projected = frame.camera.projectPoint(
                    worldPos,
                    orientation: .portrait,
                    viewportSize: CGSize(width: width, height: height)
                )

                let x = Int(projected.x)
                let y = Int(projected.y)

                if x >= 0 && x < width && y >= 0 && y < height {
                    let index = y * width + x
                    depthArray[index] = distance

                    // Spread depth to neighbors
                    for dy in -2...2 {
                        for dx in -2...2 {
                            let nx = x + dx
                            let ny = y + dy
                            if nx >= 0 && nx < width && ny >= 0 && ny < height {
                                let nIndex = ny * width + nx
                                depthArray[nIndex] = distance
                            }
                        }
                    }
                }
            }

            return depthArray
        }
    }
}
