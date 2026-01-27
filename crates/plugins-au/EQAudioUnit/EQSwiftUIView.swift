// EQSwiftUIView.swift
// SwiftUI views for SOTF Parametric EQ Audio Unit
//
// Provides a modern SwiftUI interface as fallback when Rust Metal UI is unavailable.

import SwiftUI
import AppKit

// MARK: - Observable Model

/// Observable model that bridges EQFilterParams array with SwiftUI
@Observable
final class EQParameterModel {
    var bands: [EQFilterParams]
    var onParametersChanged: (([EQFilterParams]) -> Void)?

    init(bandCount: Int = 5) {
        let defaultFreqs = [100.0, 300.0, 1000.0, 3000.0, 10000.0]
        bands = (0..<bandCount).map { i in
            var filter = EQFilterParams.default()
            filter.frequency = defaultFreqs[i]
            return filter
        }
    }

    /// Update a band and notify listeners
    func updateBand(_ index: Int, _ update: (inout EQFilterParams) -> Void) {
        guard index < bands.count else { return }
        update(&bands[index])
        bands[index].clamp()
        onParametersChanged?(bands)
    }

    /// Notify that parameters changed (call after direct band modifications)
    func notifyChanged() {
        onParametersChanged?(bands)
    }
}

// MARK: - Main EQ View

/// Main EQ view displaying all bands in a vertical layout
struct EQView: View {
    @Bindable var model: EQParameterModel

    var body: some View {
        VStack(spacing: 16) {
            // Header
            VStack(spacing: 4) {
                Text("SOTF Parametric EQ")
                    .font(.system(size: 20, weight: .bold))
                    .foregroundStyle(.white)

                Text("5-Band Parametric Equalizer")
                    .font(.system(size: 12))
                    .foregroundStyle(Color(white: 0.6))
            }

            Divider()
                .background(Color(white: 0.25))

            // Column headers
            HStack(spacing: 12) {
                Text("Band")
                    .frame(width: 60)
                Text("Type")
                    .frame(width: 90)
                Text("Frequency")
                    .frame(width: 100)
                Text("Q")
                    .frame(width: 70)
                Text("Gain")
                    .frame(width: 80)
            }
            .font(.system(size: 11, weight: .semibold))
            .foregroundStyle(Color(white: 0.7))

            // Band rows
            ForEach(0..<model.bands.count, id: \.self) { index in
                EQBandRow(
                    bandIndex: index,
                    params: Binding(
                        get: { model.bands[index] },
                        set: { newValue in
                            model.bands[index] = newValue
                            model.bands[index].clamp()
                            model.notifyChanged()
                        }
                    ),
                    onChanged: { model.notifyChanged() }
                )
            }

            Spacer()

            // Footer with parameter limits
            Text("Freq: 20-20000 Hz  |  Q: 0.1-10.0  |  Gain: -24 to +24 dB")
                .font(.system(size: 10, design: .monospaced))
                .foregroundStyle(Color(white: 0.45))

            Text("v0.5.3")
                .font(.system(size: 10, design: .monospaced))
                .foregroundStyle(Color(white: 0.35))
        }
        .padding(16)
        .frame(minWidth: 500, minHeight: 350)
        .background(Color(nsColor: NSColor(calibratedRed: 0.12, green: 0.12, blue: 0.14, alpha: 1.0)))
    }
}

// MARK: - Band Row

/// Single EQ band row with all parameters
struct EQBandRow: View {
    let bandIndex: Int
    @Binding var params: EQFilterParams
    let onChanged: () -> Void

    private let bandColors: [Color] = [
        Color(red: 0.95, green: 0.3, blue: 0.3),   // Red
        Color(red: 0.95, green: 0.6, blue: 0.2),   // Orange
        Color(red: 0.95, green: 0.85, blue: 0.2),  // Yellow
        Color(red: 0.3, green: 0.85, blue: 0.4),   // Green
        Color(red: 0.3, green: 0.6, blue: 0.95),   // Blue
    ]

    var body: some View {
        HStack(spacing: 12) {
            // Band indicator
            Text("Band \(bandIndex + 1)")
                .font(.system(size: 12, weight: .medium))
                .foregroundStyle(bandColors[bandIndex % bandColors.count])
                .frame(width: 60)

            // Filter type picker
            Picker("", selection: $params.filterType) {
                ForEach(0..<EQFilterParams.filterTypeNames.count, id: \.self) { i in
                    Text(EQFilterParams.filterTypeNames[i]).tag(Int32(i))
                }
            }
            .pickerStyle(.menu)
            .frame(width: 90)
            .onChange(of: params.filterType) { _, _ in
                onChanged()
            }

            // Frequency field
            ParameterField(
                value: $params.frequency,
                range: EQFilterParams.frequencyRange,
                format: "%.0f",
                suffix: "Hz",
                onChanged: onChanged
            )
            .frame(width: 100)

            // Q field
            ParameterField(
                value: $params.q,
                range: EQFilterParams.qRange,
                format: "%.2f",
                suffix: "",
                onChanged: onChanged
            )
            .frame(width: 70)

            // Gain field
            ParameterField(
                value: $params.gainDb,
                range: EQFilterParams.gainRange,
                format: "%.1f",
                suffix: "dB",
                onChanged: onChanged
            )
            .frame(width: 80)
        }
        .padding(.vertical, 6)
        .padding(.horizontal, 8)
        .background(
            RoundedRectangle(cornerRadius: 6)
                .fill(Color(white: 0.16))
        )
    }
}

// MARK: - Parameter Field

/// Reusable text field for numeric parameter editing
struct ParameterField: View {
    @Binding var value: Double
    let range: ClosedRange<Double>
    let format: String
    let suffix: String
    let onChanged: () -> Void

    @State private var text: String = ""
    @State private var isEditing: Bool = false
    @FocusState private var isFocused: Bool

    var body: some View {
        HStack(spacing: 2) {
            TextField("", text: $text)
                .textFieldStyle(.plain)
                .font(.system(size: 12, design: .monospaced))
                .foregroundStyle(.white)
                .multilineTextAlignment(.trailing)
                .padding(.horizontal, 6)
                .padding(.vertical, 4)
                .background(
                    RoundedRectangle(cornerRadius: 4)
                        .fill(Color(white: isFocused ? 0.25 : 0.2))
                        .stroke(isFocused ? Color.accentColor : Color.clear, lineWidth: 1)
                )
                .focused($isFocused)
                .onAppear {
                    text = String(format: format, value)
                }
                .onChange(of: value) { _, newValue in
                    if !isEditing {
                        text = String(format: format, newValue)
                    }
                }
                .onSubmit {
                    commitValue()
                }
                .onChange(of: isFocused) { _, focused in
                    if focused {
                        isEditing = true
                    } else {
                        commitValue()
                        isEditing = false
                    }
                }

            if !suffix.isEmpty {
                Text(suffix)
                    .font(.system(size: 10))
                    .foregroundStyle(Color(white: 0.5))
                    .frame(width: 20, alignment: .leading)
            }
        }
    }

    private func commitValue() {
        if let newValue = Double(text) {
            let clamped = min(max(newValue, range.lowerBound), range.upperBound)
            value = clamped
            onChanged()
        }
        text = String(format: format, value)
    }
}

// MARK: - Preview

#Preview {
    EQView(model: EQParameterModel())
        .frame(width: 550, height: 400)
}
