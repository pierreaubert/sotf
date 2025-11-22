//
//  ScannerViewModel.swift
//  HeadScanner iOS
//
//  View model for scanner state management
//

import Foundation
import Combine
import simd

@MainActor
class ScannerViewModel: ObservableObject {
    // Published state
    @Published var isScanning = false
    @Published var isProcessing = false
    @Published var metrics = QualityMetrics(coverage: 0, angularCoverage: 0, pointDensity: 0, blurScore: 0)
    @Published var nextRegion: HeadRegionEnum = .front
    @Published var coveredRegions: Set<HeadRegionEnum> = []
    @Published var statusMessage = ""
    @Published var errorMessage: String?

    // Rust scanner instance
    private var scanner: HeadScanner?
    private var guidance: ScanGuidanceWrapper?
    private var currentMesh: HeadMesh?

    // Frame processing queue
    private let processingQueue = DispatchQueue(label: "com.headscanner.processing", qos: .userInitiated)
    private var frameCounter = 0

    init() {
        setupScanner()
    }

    // MARK: - Setup

    private func setupScanner() {
        scanner = HeadScanner()
        guidance = scanner?.getGuidance()

        if scanner == nil {
            handleError("Failed to initialize scanner: \(HeadScanner.lastError() ?? "unknown error")")
        }
    }

    // MARK: - Scanning Control

    func toggleScanning() {
        isScanning.toggle()

        if isScanning {
            startScanning()
        } else {
            stopScanning()
        }
    }

    private func startScanning() {
        frameCounter = 0
        coveredRegions.removeAll()
        metrics = QualityMetrics(coverage: 0, angularCoverage: 0, pointDensity: 0, blurScore: 0)
        statusMessage = "Scanning..."
    }

    private func stopScanning() {
        statusMessage = "Scan stopped"
    }

    // MARK: - Frame Processing

    func processFrame(
        rgb: Data,
        depth: [Float],
        width: UInt32,
        height: UInt32,
        position: SIMD3<Float>,
        rotation: simd_quatf
    ) {
        // Process every N frames to avoid overload
        frameCounter += 1
        guard frameCounter % 3 == 0 else { return }

        // Validate input data before FFI call to catch errors early
        let expectedRGBSize = Int(width * height * 3)
        let expectedDepthSize = Int(width * height)

        guard rgb.count == expectedRGBSize else {
            Task { @MainActor in
                self.handleError("Invalid RGB data size: expected \(expectedRGBSize), got \(rgb.count)")
            }
            return
        }

        guard depth.count == expectedDepthSize else {
            Task { @MainActor in
                self.handleError("Invalid depth data size: expected \(expectedDepthSize), got \(depth.count)")
            }
            return
        }

        processingQueue.async { [weak self] in
            guard let self = self else { return }

            // Update guidance with current pose
            self.guidance?.updatePose(position: position, rotation: rotation)

            // Process frame through Rust scanner
            let success = self.scanner?.processFrame(
                rgb: rgb,
                depth: depth,
                width: width,
                height: height,
                position: position,
                rotation: rotation
            ) ?? false

            if !success {
                let error = HeadScanner.lastError() ?? "Unknown error"
                Task { @MainActor in
                    self.handleError("Frame processing failed: \(error)")
                }
                return
            }

            // Update metrics and guidance on main thread
            Task { @MainActor in
                self.updateMetrics()
            }
        }
    }

    private func updateMetrics() {
        guard let guidance = guidance else { return }

        // Get updated metrics
        metrics = guidance.getMetrics()

        // Update next region
        nextRegion = guidance.getNextRegion()

        // Update covered regions
        var covered = Set<HeadRegionEnum>()
        for region in HeadRegionEnum.allCases {
            if guidance.isRegionCovered(region) {
                covered.insert(region)
            }
        }
        coveredRegions = covered

        // Check if scan is complete
        if metrics.isComplete && isScanning {
            completeScan()
        }
    }

    private func completeScan() {
        isScanning = false
        statusMessage = "Scan complete!"

        // Vibrate to indicate completion
        let generator = UINotificationFeedbackGenerator()
        generator.notificationOccurred(.success)
    }

    // MARK: - Save and Export

    func saveScan() {
        guard let scanner = scanner else {
            handleError("No active scanner")
            return
        }

        isProcessing = true
        statusMessage = "Generating mesh..."

        processingQueue.async { [weak self] in
            guard let self = self else { return }

            // Get mesh from scanner
            guard let mesh = scanner.getMesh() else {
                Task { @MainActor in
                    self.isProcessing = false
                    self.handleError("Failed to generate mesh: \(HeadScanner.lastError() ?? "unknown error")")
                }
                return
            }

            self.currentMesh = mesh

            // Save to documents directory
            let filename = "head_scan_\(Date().timeIntervalSince1970).obj"
            if let documentsPath = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first {
                let filePath = documentsPath.appendingPathComponent(filename).path

                let success = mesh.exportOBJ(to: filePath)

                Task { @MainActor in
                    self.isProcessing = false

                    if success {
                        self.statusMessage = "Saved to \(filename)"
                        self.showSuccessAlert("Mesh saved successfully!")
                    } else {
                        self.handleError("Failed to save mesh: \(HeadScanner.lastError() ?? "unknown error")")
                    }
                }
            }
        }
    }

    func exportSOFA() {
        guard let mesh = currentMesh ?? scanner?.getMesh() else {
            handleError("No mesh available. Please scan first.")
            return
        }

        isProcessing = true
        statusMessage = "Generating SOFA file..."

        processingQueue.async { [weak self] in
            guard let self = self else { return }

            // Generate SOFA file
            let filename = "hrtf_\(Date().timeIntervalSince1970).sofa"
            if let documentsPath = FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).first {
                let filePath = documentsPath.appendingPathComponent(filename).path

                let success = mesh.generateSOFA(
                    to: filePath,
                    sampleRate: 44100.0,
                    azimuthResolution: 360,
                    elevationResolution: 180,
                    distance: 1.0
                )

                Task { @MainActor in
                    self.isProcessing = false

                    if success {
                        self.statusMessage = "SOFA file generated!"
                        self.showSuccessAlert("HRTF SOFA file saved to \(filename)")
                    } else {
                        self.handleError("Failed to generate SOFA: \(HeadScanner.lastError() ?? "unknown error")")
                    }
                }
            }
        }
    }

    // MARK: - Error Handling

    func handleError(_ message: String) {
        errorMessage = message
        print("❌ Error: \(message)")
    }

    func dismissError() {
        errorMessage = nil
    }

    private func showSuccessAlert(_ message: String) {
        // Show success message (could use a toast or alert)
        print("✅ Success: \(message)")

        // Vibrate for success
        let generator = UINotificationFeedbackGenerator()
        generator.notificationOccurred(.success)
    }
}
