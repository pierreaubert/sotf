//
//  HeadScannerApp.swift
//  HeadScanner iOS
//
//  Main app entry point
//

import SwiftUI

@main
struct HeadScannerApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}

struct ContentView: View {
    @StateObject private var viewModel = ScannerViewModel()

    var body: some View {
        ZStack {
            // AR Camera view
            ARScannerView(viewModel: viewModel)
                .edgesIgnoringSafeArea(.all)

            // Overlay UI
            VStack {
                // Top bar - metrics
                MetricsView(metrics: viewModel.metrics)
                    .padding()
                    .background(Color.black.opacity(0.6))
                    .cornerRadius(12)
                    .padding(.top, 50)

                Spacer()

                // Middle - guidance
                GuidanceView(
                    nextRegion: viewModel.nextRegion,
                    coveredRegions: viewModel.coveredRegions
                )
                .padding()

                Spacer()

                // Bottom bar - controls
                ControlsView(
                    isScanning: viewModel.isScanning,
                    onStartStop: viewModel.toggleScanning,
                    onSave: viewModel.saveScan,
                    onExport: viewModel.exportSOFA
                )
                .padding()
                .background(Color.black.opacity(0.6))
                .cornerRadius(12)
                .padding(.bottom, 30)
            }

            // Loading overlay
            if viewModel.isProcessing {
                LoadingView(message: viewModel.statusMessage)
            }

            // Error alert
            if let error = viewModel.errorMessage {
                ErrorView(message: error) {
                    viewModel.dismissError()
                }
            }
        }
        .statusBar(hidden: true)
    }
}

/// Metrics display
struct MetricsView: View {
    let metrics: QualityMetrics

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("Scan Progress")
                    .font(.headline)
                    .foregroundColor(.white)
                Spacer()
                Text("\(Int(metrics.coverage * 100))%")
                    .font(.title2)
                    .fontWeight(.bold)
                    .foregroundColor(metrics.isComplete ? .green : .white)
            }

            ProgressView(value: metrics.coverage)
                .progressViewStyle(LinearProgressViewStyle(tint: .blue))

            HStack(spacing: 20) {
                MetricItem(label: "Angular", value: metrics.angularCoverage)
                MetricItem(label: "Density", value: metrics.pointDensity)
                MetricItem(label: "Quality", value: metrics.blurScore)
            }
        }
        .padding()
    }
}

struct MetricItem: View {
    let label: String
    let value: Float

    var body: some View {
        VStack(spacing: 4) {
            Text(label)
                .font(.caption)
                .foregroundColor(.gray)
            Text("\(Int(value * 100))%")
                .font(.system(size: 14, weight: .semibold))
                .foregroundColor(.white)
        }
    }
}

/// Guidance overlay
struct GuidanceView: View {
    let nextRegion: HeadRegionEnum
    let coveredRegions: Set<HeadRegionEnum>

    var body: some View {
        VStack(spacing: 16) {
            // Instruction text
            VStack(spacing: 8) {
                Text("Scan:")
                    .font(.title3)
                    .foregroundColor(.white)
                Text(nextRegion.displayName)
                    .font(.title)
                    .fontWeight(.bold)
                    .foregroundColor(.yellow)
            }
            .padding()
            .background(Color.black.opacity(0.7))
            .cornerRadius(12)

            // Region grid
            RegionGridView(coveredRegions: coveredRegions, nextRegion: nextRegion)
                .frame(width: 200, height: 200)
        }
    }
}

/// Region coverage grid
struct RegionGridView: View {
    let coveredRegions: Set<HeadRegionEnum>
    let nextRegion: HeadRegionEnum

    var body: some View {
        Canvas { context, size in
            let cellSize = size.width / 3

            // Draw head outline (simplified)
            let center = CGPoint(x: size.width / 2, y: size.height / 2)
            let radius = min(size.width, size.height) / 2 * 0.8

            context.stroke(
                Path(ellipseIn: CGRect(
                    x: center.x - radius,
                    y: center.y - radius,
                    width: radius * 2,
                    height: radius * 2
                )),
                with: .color(.white),
                lineWidth: 2
            )

            // Draw region indicators
            drawRegionIndicator(context: context, region: .front, angle: 0, radius: radius, center: center)
            drawRegionIndicator(context: context, region: .right, angle: .pi / 2, radius: radius, center: center)
            drawRegionIndicator(context: context, region: .back, angle: .pi, radius: radius, center: center)
            drawRegionIndicator(context: context, region: .left, angle: .pi * 3 / 2, radius: radius, center: center)
            drawRegionIndicator(context: context, region: .top, angle: 0, radius: 0, center: center)
        }
    }

    private func drawRegionIndicator(
        context: GraphicsContext,
        region: HeadRegionEnum,
        angle: CGFloat,
        radius: CGFloat,
        center: CGPoint
    ) {
        let x = center.x + cos(angle) * radius * 0.7
        let y = center.y + sin(angle) * radius * 0.7
        let size: CGFloat = 20

        let color: Color
        if coveredRegions.contains(region) {
            color = .green
        } else if region == nextRegion {
            color = .yellow
        } else {
            color = .gray
        }

        context.fill(
            Path(ellipseIn: CGRect(x: x - size / 2, y: y - size / 2, width: size, height: size)),
            with: .color(color)
        )
    }
}

/// Control buttons
struct ControlsView: View {
    let isScanning: Bool
    let onStartStop: () -> Void
    let onSave: () -> Void
    let onExport: () -> Void

    var body: some View {
        HStack(spacing: 20) {
            // Start/Stop button
            Button(action: onStartStop) {
                HStack {
                    Image(systemName: isScanning ? "stop.fill" : "play.fill")
                    Text(isScanning ? "Stop" : "Start")
                }
                .font(.headline)
                .foregroundColor(.white)
                .padding(.horizontal, 24)
                .padding(.vertical, 12)
                .background(isScanning ? Color.red : Color.blue)
                .cornerRadius(8)
            }

            // Save button
            Button(action: onSave) {
                Image(systemName: "square.and.arrow.down")
                    .font(.headline)
                    .foregroundColor(.white)
                    .padding(12)
                    .background(Color.green)
                    .cornerRadius(8)
            }

            // Export SOFA button
            Button(action: onExport) {
                Image(systemName: "waveform")
                    .font(.headline)
                    .foregroundColor(.white)
                    .padding(12)
                    .background(Color.purple)
                    .cornerRadius(8)
            }
        }
    }
}

/// Loading overlay
struct LoadingView: View {
    let message: String

    var body: some View {
        ZStack {
            Color.black.opacity(0.7)
                .edgesIgnoringSafeArea(.all)

            VStack(spacing: 20) {
                ProgressView()
                    .progressViewStyle(CircularProgressViewStyle(tint: .white))
                    .scaleEffect(1.5)

                Text(message)
                    .font(.headline)
                    .foregroundColor(.white)
            }
            .padding(40)
            .background(Color.black.opacity(0.8))
            .cornerRadius(16)
        }
    }
}

/// Error overlay
struct ErrorView: View {
    let message: String
    let onDismiss: () -> Void

    var body: some View {
        ZStack {
            Color.black.opacity(0.5)
                .edgesIgnoringSafeArea(.all)
                .onTapGesture(perform: onDismiss)

            VStack(spacing: 20) {
                Image(systemName: "exclamationmark.triangle.fill")
                    .font(.system(size: 48))
                    .foregroundColor(.red)

                Text("Error")
                    .font(.title2)
                    .fontWeight(.bold)
                    .foregroundColor(.white)

                Text(message)
                    .font(.body)
                    .foregroundColor(.white)
                    .multilineTextAlignment(.center)

                Button("OK") {
                    onDismiss()
                }
                .font(.headline)
                .foregroundColor(.white)
                .padding(.horizontal, 40)
                .padding(.vertical, 12)
                .background(Color.blue)
                .cornerRadius(8)
            }
            .padding(30)
            .background(Color.black.opacity(0.9))
            .cornerRadius(16)
            .padding(40)
        }
    }
}
