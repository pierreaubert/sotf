// EQViewController.swift
// SwiftUI-based UI for SOTF Parametric EQ

import SwiftUI
import CoreAudioKit
import AudioToolbox

// MARK: - View Controller

public class EQViewController: AUViewController {
    private var observation: NSKeyValueObservation?

    public override func viewDidLoad() {
        super.viewDidLoad()

        // Observe audioUnit changes
        observation = observe(\.audioUnit, options: [.new]) { [weak self] _, change in
            DispatchQueue.main.async {
                self?.audioUnitChanged()
            }
        }

        audioUnitChanged()
    }

    private func audioUnitChanged() {
        guard let eqAudioUnit = audioUnit as? EQAudioUnit else {
            return
        }

        // Create SwiftUI view
        let eqView = EQView(audioUnit: eqAudioUnit)
        let hostingController = NSHostingController(rootView: eqView)

        // Add as child
        addChild(hostingController)
        view.addSubview(hostingController.view)

        // Layout
        hostingController.view.translatesAutoresizingMaskIntoConstraints = false
        NSLayoutConstraint.activate([
            hostingController.view.topAnchor.constraint(equalTo: view.topAnchor),
            hostingController.view.bottomAnchor.constraint(equalTo: view.bottomAnchor),
            hostingController.view.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            hostingController.view.trailingAnchor.constraint(equalTo: view.trailingAnchor),
        ])

        hostingController.didMove(toParent: self)
    }
}

// MARK: - SwiftUI View

struct EQView: View {
    @ObservedObject private var viewModel: EQViewModel

    init(audioUnit: EQAudioUnit) {
        self.viewModel = EQViewModel(audioUnit: audioUnit)
    }

    var body: some View {
        VStack(spacing: 0) {
            // Header
            headerView

            // EQ Bands
            ScrollView {
                VStack(spacing: 12) {
                    ForEach(viewModel.bands.indices, id: \.self) { index in
                        bandView(index: index)
                    }
                }
                .padding()
            }
        }
        .frame(minWidth: 400, minHeight: 500)
        .background(Color(NSColor.windowBackgroundColor))
    }

    private var headerView: some View {
        VStack(spacing: 4) {
            Text("SOTF Parametric EQ")
                .font(.title)
                .fontWeight(.bold)

            Text("10-Band Equalizer")
                .font(.subheadline)
                .foregroundColor(.secondary)
        }
        .padding()
        .frame(maxWidth: .infinity)
        .background(Color(NSColor.controlBackgroundColor))
    }

    private func bandView(index: Int) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            // Band header
            HStack {
                Text("Band \(index + 1)")
                    .font(.headline)

                Spacer()

                // Enable/disable toggle (future feature)
                // Toggle("", isOn: $viewModel.bands[index].enabled)
            }

            // Frequency
            parameterSlider(
                label: "Frequency",
                value: $viewModel.bands[index].frequency,
                range: 20...20000,
                unit: "Hz",
                onChange: { viewModel.updateFrequency(band: index) }
            )

            // Q
            parameterSlider(
                label: "Q",
                value: $viewModel.bands[index].q,
                range: 0.1...10.0,
                unit: "",
                onChange: { viewModel.updateQ(band: index) }
            )

            // Gain
            parameterSlider(
                label: "Gain",
                value: $viewModel.bands[index].gain,
                range: -12...12,
                unit: "dB",
                onChange: { viewModel.updateGain(band: index) }
            )
        }
        .padding()
        .background(Color(NSColor.controlBackgroundColor))
        .cornerRadius(8)
    }

    private func parameterSlider(
        label: String,
        value: Binding<Double>,
        range: ClosedRange<Double>,
        unit: String,
        onChange: @escaping () -> Void
    ) -> some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Text(label)
                    .font(.caption)
                    .foregroundColor(.secondary)

                Spacer()

                Text(formatValue(value.wrappedValue, unit: unit))
                    .font(.caption)
                    .monospacedDigit()
            }

            Slider(
                value: value,
                in: range,
                onEditingChanged: { editing in
                    if !editing {
                        onChange()
                    }
                }
            )
        }
    }

    private func formatValue(_ value: Double, unit: String) -> String {
        if unit == "Hz" {
            if value >= 1000 {
                return String(format: "%.1f kHz", value / 1000)
            } else {
                return String(format: "%.0f Hz", value)
            }
        } else if unit == "dB" {
            return String(format: "%+.1f dB", value)
        } else {
            return String(format: "%.2f", value)
        }
    }
}

// MARK: - View Model

class EQViewModel: ObservableObject {
    @Published var bands: [EQBand] = []

    private weak var audioUnit: EQAudioUnit?
    private var parameterObservers: [NSKeyValueObservation] = []

    struct EQBand {
        var enabled: Bool = false
        var frequency: Double = 1000.0
        var q: Double = 1.0
        var gain: Double = 0.0
    }

    init(audioUnit: EQAudioUnit) {
        self.audioUnit = audioUnit

        // Initialize bands
        for i in 0..<10 {
            bands.append(EQBand(
                enabled: false,
                frequency: defaultFrequency(for: i),
                q: 1.0,
                gain: 0.0
            ))
        }

        // Observe parameter changes from host
        setupParameterObservation()
    }

    private func defaultFrequency(for band: Int) -> Double {
        // Standard 10-band EQ frequencies
        let frequencies: [Double] = [31.5, 63, 125, 250, 500, 1000, 2000, 4000, 8000, 16000]
        return frequencies[band]
    }

    private func setupParameterObservation() {
        guard let paramTree = audioUnit?.parameterTree else { return }

        // Observe all parameters
        for param in paramTree.allParameters {
            let observation = param.observe(\.value, options: [.new]) { [weak self] param, _ in
                DispatchQueue.main.async {
                    self?.parameterChanged(param: param)
                }
            }
            parameterObservers.append(observation)
        }
    }

    private func parameterChanged(param: AUParameter) {
        let id = param.identifier

        // Parse parameter ID (e.g., "band0_freq")
        let components = id.split(separator: "_")
        guard components.count == 2,
              let bandStr = components[0].dropFirst(4) as? Substring,
              let bandIndex = Int(bandStr),
              bandIndex < bands.count else {
            return
        }

        let paramType = String(components[1])
        let value = Double(param.value)

        // Update band
        switch paramType {
        case "freq":
            bands[bandIndex].frequency = value
        case "q":
            bands[bandIndex].q = value
        case "gain":
            bands[bandIndex].gain = value
            bands[bandIndex].enabled = abs(value) > 0.01 // Auto-enable if gain != 0
        default:
            break
        }
    }

    // MARK: - Parameter Updates

    func updateFrequency(band: Int) {
        setParameter(band: band, type: "freq", value: bands[band].frequency)
    }

    func updateQ(band: Int) {
        setParameter(band: band, type: "q", value: bands[band].q)
    }

    func updateGain(band: Int) {
        setParameter(band: band, type: "gain", value: bands[band].gain)

        // Auto-enable/disable band based on gain
        bands[band].enabled = abs(bands[band].gain) > 0.01
    }

    private func setParameter(band: Int, type: String, value: Double) {
        guard let paramTree = audioUnit?.parameterTree else { return }

        let paramId = "band\(band)_\(type)"

        // Find parameter
        guard let param = paramTree.allParameters.first(where: { $0.identifier == paramId }) else {
            return
        }

        // Set value
        param.value = AUValue(value)
    }
}

// MARK: - Preview

#if DEBUG
struct EQView_Previews: PreviewProvider {
    static var previews: some View {
        // Preview requires a mock audio unit
        Text("EQ View Preview")
            .frame(width: 400, height: 500)
    }
}
#endif
