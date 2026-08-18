import SwiftUI
import ConfigBarModels

// MARK: - Plugin Editor Views

/// Router that picks the right editor view for a plugin type
struct PluginEditorView: View {
    let pluginType: String
    let parameters: [String: Any]
    let descriptors: [PluginParameterDescriptor]
    let onUpdate: ([String: Any]) -> Void

    var body: some View {
        if !descriptors.isEmpty && pluginType != "eq" {
            DescriptorPluginEditor(
                pluginType: pluginType,
                parameters: parameters,
                descriptors: descriptors,
                onUpdate: onUpdate
            )
        } else {
            switch pluginType {
            case "eq":
                EQEditor(parameters: parameters, onUpdate: onUpdate)
            default:
                GenericPluginEditor(pluginType: pluginType, parameters: parameters, onUpdate: onUpdate)
            }
        }
    }
}

// MARK: - Gain Editor

struct GainEditor: View {
    let parameters: [String: Any]
    let onUpdate: ([String: Any]) -> Void

    @State private var gainDb: Double = 0.0

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("Gain:")
                    .frame(width: 80, alignment: .trailing)
                Slider(value: $gainDb, in: -60...20, step: 0.1)
                    .onChange(of: gainDb) { newValue in
                        onUpdate(["gain_db": newValue])
                    }
                Text("\(gainDb, specifier: "%.1f") dB")
                    .frame(width: 70, alignment: .trailing)
                    .monospacedDigit()
            }
        }
        .onAppear {
            gainDb = parameters["gain_db"] as? Double ?? 0.0
        }
    }
}

// MARK: - EQ Editor

struct EQEditor: View {
    let parameters: [String: Any]
    let onUpdate: ([String: Any]) -> Void

    @State private var filters: [[String: Any]] = []

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            ForEach(Array(filters.enumerated()), id: \.offset) { index, filter in
                EQBandRow(
                    index: index,
                    filter: filter,
                    onUpdate: { updatedFilter in
                        filters[index] = updatedFilter
                        emitUpdate()
                    },
                    onRemove: {
                        filters.remove(at: index)
                        emitUpdate()
                    }
                )
            }

            Button(action: {
                filters.append([
                    "filter_type": "peak",
                    "frequency": 1000.0,
                    "q": 1.0,
                    "gain_db": 0.0
                ] as [String: Any])
                emitUpdate()
            }) {
                Label("Add Band", systemImage: "plus.circle")
            }
            .buttonStyle(.borderless)
        }
        .onAppear {
            if let f = parameters["filters"] as? [[String: Any]] {
                filters = f
            }
        }
    }

    private func emitUpdate() {
        var params = parameters
        params["filters"] = filters
        onUpdate(params)
    }
}

struct EQBandRow: View {
    let index: Int
    let filter: [String: Any]
    let onUpdate: ([String: Any]) -> Void
    let onRemove: () -> Void

    @State private var filterType: String = "peak"
    @State private var frequency: Double = 1000.0
    @State private var q: Double = 1.0
    @State private var gainDb: Double = 0.0

    let filterTypes = ["peak", "lowshelf", "highshelf", "lowpass", "highpass", "notch", "bandpass"]

    var body: some View {
        HStack(spacing: 6) {
            Text("#\(index + 1)")
                .frame(width: 25)
                .foregroundColor(.secondary)

            Picker("", selection: $filterType) {
                ForEach(filterTypes, id: \.self) { type_ in
                    Text(type_.capitalized).tag(type_)
                }
            }
            .frame(width: 90)
            .onChange(of: filterType) { _ in emitUpdate() }

            Text("Hz:")
            TextField("", value: $frequency, format: .number)
                .frame(width: 60)
                .onChange(of: frequency) { _ in emitUpdate() }

            Text("Q:")
            TextField("", value: $q, format: .number)
                .frame(width: 45)
                .onChange(of: q) { _ in emitUpdate() }

            Text("dB:")
            TextField("", value: $gainDb, format: .number)
                .frame(width: 50)
                .onChange(of: gainDb) { _ in emitUpdate() }

            Button(action: onRemove) {
                Image(systemName: "minus.circle")
                    .foregroundColor(.red)
            }
            .buttonStyle(.borderless)
        }
        .onAppear {
            filterType = filter["filter_type"] as? String ?? "peak"
            frequency = filter["frequency"] as? Double ?? 1000.0
            q = filter["q"] as? Double ?? 1.0
            gainDb = filter["gain_db"] as? Double ?? 0.0
        }
    }

    private func emitUpdate() {
        onUpdate([
            "filter_type": filterType,
            "frequency": frequency,
            "q": q,
            "gain_db": gainDb
        ] as [String: Any])
    }
}

// MARK: - Compressor Editor

struct CompressorEditor: View {
    let parameters: [String: Any]
    let onUpdate: ([String: Any]) -> Void

    @State private var threshold: Double = -20.0
    @State private var ratio: Double = 4.0
    @State private var attack: Double = 5.0
    @State private var release: Double = 50.0
    @State private var knee: Double = 6.0
    @State private var makeupGain: Double = 0.0
    @State private var mix: Double = 1.0

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            paramSlider("Threshold:", value: $threshold, range: -60...0, unit: "dB")
            paramSlider("Ratio:", value: $ratio, range: 1...20, unit: ":1")
            paramSlider("Attack:", value: $attack, range: 0.1...100, unit: "ms")
            paramSlider("Release:", value: $release, range: 10...1000, unit: "ms")
            paramSlider("Knee:", value: $knee, range: 0...20, unit: "dB")
            paramSlider("Makeup:", value: $makeupGain, range: -24...24, unit: "dB")
            paramSlider("Mix:", value: $mix, range: 0...1, unit: "")
        }
        .onAppear { loadParams() }
    }

    private func loadParams() {
        threshold = parameters["threshold_db"] as? Double ?? -20.0
        ratio = parameters["ratio"] as? Double ?? 4.0
        attack = parameters["attack_ms"] as? Double ?? 5.0
        release = parameters["release_ms"] as? Double ?? 50.0
        knee = parameters["knee_db"] as? Double ?? 6.0
        makeupGain = parameters["makeup_gain_db"] as? Double ?? 0.0
        mix = parameters["mix"] as? Double ?? 1.0
    }

    private func emitUpdate() {
        onUpdate([
            "threshold_db": threshold,
            "ratio": ratio,
            "attack_ms": attack,
            "release_ms": release,
            "knee_db": knee,
            "makeup_gain_db": makeupGain,
            "mix": mix,
        ] as [String: Any])
    }

    private func paramSlider(_ label: String, value: Binding<Double>, range: ClosedRange<Double>, unit: String) -> some View {
        HStack {
            Text(label)
                .frame(width: 80, alignment: .trailing)
            Slider(value: value, in: range)
                .onChange(of: value.wrappedValue) { _ in emitUpdate() }
            Text("\(value.wrappedValue, specifier: "%.1f") \(unit)")
                .frame(width: 80, alignment: .trailing)
                .monospacedDigit()
        }
    }
}

// MARK: - Limiter Editor

struct LimiterEditor: View {
    let parameters: [String: Any]
    let onUpdate: ([String: Any]) -> Void

    @State private var threshold: Double = -0.1
    @State private var release: Double = 50.0
    @State private var lookahead: Double = 5.0

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            paramSlider("Threshold:", value: $threshold, range: -20...0, unit: "dB")
            paramSlider("Release:", value: $release, range: 10...1000, unit: "ms")
            paramSlider("Lookahead:", value: $lookahead, range: 0...20, unit: "ms")
        }
        .onAppear {
            threshold = parameters["threshold_db"] as? Double ?? -0.1
            release = parameters["release_ms"] as? Double ?? 50.0
            lookahead = parameters["lookahead_ms"] as? Double ?? 5.0
        }
    }

    private func emitUpdate() {
        onUpdate([
            "threshold_db": threshold,
            "release_ms": release,
            "lookahead_ms": lookahead,
        ] as [String: Any])
    }

    private func paramSlider(_ label: String, value: Binding<Double>, range: ClosedRange<Double>, unit: String) -> some View {
        HStack {
            Text(label)
                .frame(width: 80, alignment: .trailing)
            Slider(value: value, in: range)
                .onChange(of: value.wrappedValue) { _ in emitUpdate() }
            Text("\(value.wrappedValue, specifier: "%.1f") \(unit)")
                .frame(width: 80, alignment: .trailing)
                .monospacedDigit()
        }
    }
}

// MARK: - Gate Editor

struct GateEditor: View {
    let parameters: [String: Any]
    let onUpdate: ([String: Any]) -> Void

    @State private var threshold: Double = -40.0
    @State private var ratio: Double = 10.0
    @State private var attack: Double = 1.0
    @State private var hold: Double = 10.0
    @State private var release: Double = 100.0

    var body: some View {
        VStack(alignment: .leading, spacing: 6) {
            paramSlider("Threshold:", value: $threshold, range: -80...0, unit: "dB")
            paramSlider("Ratio:", value: $ratio, range: 1...100, unit: ":1")
            paramSlider("Attack:", value: $attack, range: 0.1...50, unit: "ms")
            paramSlider("Hold:", value: $hold, range: 0...1000, unit: "ms")
            paramSlider("Release:", value: $release, range: 10...2000, unit: "ms")
        }
        .onAppear {
            threshold = parameters["threshold_db"] as? Double ?? -40.0
            ratio = parameters["ratio"] as? Double ?? 10.0
            attack = parameters["attack_ms"] as? Double ?? 1.0
            hold = parameters["hold_ms"] as? Double ?? 10.0
            release = parameters["release_ms"] as? Double ?? 100.0
        }
    }

    private func emitUpdate() {
        onUpdate([
            "threshold_db": threshold,
            "ratio": ratio,
            "attack_ms": attack,
            "hold_ms": hold,
            "release_ms": release,
        ] as [String: Any])
    }

    private func paramSlider(_ label: String, value: Binding<Double>, range: ClosedRange<Double>, unit: String) -> some View {
        HStack {
            Text(label)
                .frame(width: 80, alignment: .trailing)
            Slider(value: value, in: range)
                .onChange(of: value.wrappedValue) { _ in emitUpdate() }
            Text("\(value.wrappedValue, specifier: "%.1f") \(unit)")
                .frame(width: 80, alignment: .trailing)
                .monospacedDigit()
        }
    }
}

// MARK: - Descriptor-Driven Plugin Editor

struct DescriptorPluginEditor: View {
    let pluginType: String
    let parameters: [String: Any]
    let descriptors: [PluginParameterDescriptor]
    let onUpdate: ([String: Any]) -> Void

    @State private var draftParameters: [String: Any] = [:]

    private var visibleDescriptors: [PluginParameterDescriptor] {
        switch pluginType {
        case "crossfeed":
            return crossfeedVisibleDescriptors
        case "multiband_compressor", "multiband_expander":
            return multibandVisibleDescriptors
        case "upmixer":
            return descriptors.filter { $0.key != "speaker_config" }
        default:
            return descriptors
        }
    }

    private var crossfeedVisibleDescriptors: [PluginParameterDescriptor] {
        let modeIndex = crossfeedAlgorithmModeIndex()
        return descriptors.filter { descriptor in
            if descriptor.key == "crossfeed_mode" {
                return false
            }

            switch descriptor.group.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() {
            case "bauer":
                return modeIndex == 1
            case "meier":
                return modeIndex == 2
            case "multiband", "multibands":
                return modeIndex == 3
            default:
                return true
            }
        }
    }

    private var multibandVisibleDescriptors: [PluginParameterDescriptor] {
        let activeCrossoverCount = max(multibandBandCount() - 1, 0)
        return descriptors.filter { descriptor in
            guard let crossoverIndex = crossoverFrequencyIndex(for: descriptor.key) else {
                return true
            }
            return crossoverIndex <= activeCrossoverCount
        }
    }

    private var speakerConfigDescriptor: PluginParameterDescriptor? {
        descriptors.first { $0.key == "speaker_config" }
    }

    private var descriptorGroups: [(String, [PluginParameterDescriptor])] {
        var groupOrder: [String] = []
        var groupedDescriptors: [String: [PluginParameterDescriptor]] = [:]

        for descriptor in visibleDescriptors {
            let trimmedGroup = descriptor.group.trimmingCharacters(in: .whitespacesAndNewlines)
            let groupName = trimmedGroup.isEmpty ? "General" : trimmedGroup
            if groupedDescriptors[groupName] == nil {
                groupOrder.append(groupName)
                groupedDescriptors[groupName] = []
            }
            groupedDescriptors[groupName, default: []].append(descriptor)
        }

        return groupOrder.compactMap { groupName in
            guard let descriptors = groupedDescriptors[groupName] else {
                return nil
            }
            return (groupName, descriptors)
        }
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            if pluginType == "crossfeed" {
                crossfeedModeButtonBar
            }

            if pluginType == "upmixer" {
                upmixerSpeakerConfigMenu
            }

            ForEach(descriptorGroups, id: \.0) { group in
                VStack(alignment: .leading, spacing: 6) {
                    Text(group.0.uppercased())
                        .font(.caption.weight(.semibold))
                        .foregroundColor(.secondary)

                    ForEach(group.1) { descriptor in
                        descriptorRow(descriptor)
                    }
                }
            }
        }
        .onAppear {
            draftParameters = normalizedInitialParameters(parameters)
        }
    }

    @ViewBuilder
    private var upmixerSpeakerConfigMenu: some View {
        if let descriptor = speakerConfigDescriptor {
            VStack(alignment: .leading, spacing: 6) {
                Text("OUTPUT LAYOUT")
                    .font(.caption.weight(.semibold))
                    .foregroundColor(.secondary)

                descriptorRow(descriptor)
            }
        }
    }

    private var crossfeedModeButtonBar: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack(spacing: 8) {
                Text("Mode")
                    .frame(width: 150, alignment: .trailing)
                    .font(.caption)

                Picker("", selection: crossfeedAlgorithmModeBinding) {
                    Text("Bauer").tag(1)
                    Text("Meier").tag(2)
                    Text("Multiband").tag(3)
                }
                .pickerStyle(.segmented)
                .labelsHidden()
                .frame(width: 260)
            }

            Text("Crossfeed algorithm selection")
                .font(.caption2)
                .foregroundColor(.secondary)
                .padding(.leading, 158)
                .frame(maxWidth: .infinity, alignment: .leading)
        }
        .padding(.vertical, 3)
    }

    @ViewBuilder
    private func descriptorRow(_ descriptor: PluginParameterDescriptor) -> some View {
        VStack(alignment: .leading, spacing: 2) {
            HStack(spacing: 8) {
                Text(descriptor.name)
                    .frame(width: 150, alignment: .trailing)
                    .font(.caption)

                switch descriptor.type {
                case "bool":
                    Toggle("", isOn: boolBinding(for: descriptor))
                        .labelsHidden()

                case "choice":
                    Picker("", selection: choiceIndexBinding(for: descriptor)) {
                        ForEach(Array((descriptor.choices ?? []).enumerated()), id: \.offset) { index, label in
                            Text(label).tag(index)
                        }
                    }
                    .frame(width: descriptor.key == "speaker_config" ? 220 : 180)

                case "int":
                    Stepper(
                        value: intBinding(for: descriptor),
                        in: Int(descriptor.min ?? 0)...Int(descriptor.max ?? 100),
                        step: max(Int(descriptor.step ?? 1), 1)
                    ) {
                        Text("\(intValue(for: descriptor))\(descriptor.unit.isEmpty ? "" : " \(descriptor.unit)")")
                            .frame(width: 90, alignment: .leading)
                            .monospacedDigit()
                    }

                case "file_path":
                    TextField("", text: stringBinding(for: descriptor))
                        .textFieldStyle(.roundedBorder)
                        .frame(minWidth: 220)

                default:
                    Slider(
                        value: doubleBinding(for: descriptor),
                        in: (descriptor.min ?? -100.0)...(descriptor.max ?? 100.0),
                        step: max(descriptor.step ?? 0.1, 0.0001)
                    )
                    Text("\(doubleValue(for: descriptor), specifier: "%.2f")\(descriptor.unit.isEmpty ? "" : " \(descriptor.unit)")")
                        .frame(width: 100, alignment: .trailing)
                        .monospacedDigit()
                }
            }

            if !descriptor.doc.isEmpty {
                Text(descriptor.doc)
                    .font(.caption2)
                    .foregroundColor(.secondary)
                    .padding(.leading, 158)
                    .frame(maxWidth: .infinity, alignment: .leading)
            }
        }
        .padding(.vertical, descriptor.doc.isEmpty ? 1 : 3)
    }

    private func updateValue(_ value: Any, for descriptor: PluginParameterDescriptor) {
        if pluginType == "crossfeed" {
            switch descriptor.key {
            case "crossfeed_mode":
                let index = crossfeedModeIndex(from: value) ?? 3
                draftParameters["crossfeed_mode"] = index
                draftParameters["mode"] = crossfeedModeValue(for: index)
                onUpdate(draftParameters)
                return
            case "crossfeed_preset":
                let index = crossfeedPresetIndex(from: value) ?? 0
                draftParameters["crossfeed_preset"] = index
                draftParameters["preset"] = crossfeedPresetValue(for: index)
                onUpdate(draftParameters)
                return
            default:
                break
            }
        }

        draftParameters[descriptor.key] = value
        onUpdate(draftParameters)
    }

    private func doubleBinding(for descriptor: PluginParameterDescriptor) -> Binding<Double> {
        Binding(
            get: { doubleValue(for: descriptor) },
            set: { updateValue($0, for: descriptor) }
        )
    }

    private func intBinding(for descriptor: PluginParameterDescriptor) -> Binding<Int> {
        Binding(
            get: { intValue(for: descriptor) },
            set: { updateValue($0, for: descriptor) }
        )
    }

    private func boolBinding(for descriptor: PluginParameterDescriptor) -> Binding<Bool> {
        Binding(
            get: { boolValue(for: descriptor) },
            set: { updateValue($0, for: descriptor) }
        )
    }

    private func stringBinding(for descriptor: PluginParameterDescriptor) -> Binding<String> {
        Binding(
            get: { stringValue(for: descriptor) },
            set: { updateValue($0, for: descriptor) }
        )
    }

    private func choiceIndexBinding(for descriptor: PluginParameterDescriptor) -> Binding<Int> {
        Binding(
            get: { choiceIndex(for: descriptor) },
            set: { index in
                updateValue(choiceValue(for: descriptor, index: index), for: descriptor)
            }
        )
    }

    private var crossfeedAlgorithmModeBinding: Binding<Int> {
        Binding(
            get: { crossfeedAlgorithmModeIndex() },
            set: { updateCrossfeedMode($0) }
        )
    }

    private func doubleValue(for descriptor: PluginParameterDescriptor) -> Double {
        numberValue(rawValue(for: descriptor)) ?? descriptor.defaultDouble ?? descriptor.min ?? 0.0
    }

    private func intValue(for descriptor: PluginParameterDescriptor) -> Int {
        Int((numberValue(rawValue(for: descriptor)) ?? descriptor.defaultDouble ?? descriptor.min ?? 0.0).rounded())
    }

    private func boolValue(for descriptor: PluginParameterDescriptor) -> Bool {
        let raw = rawValue(for: descriptor)
        if let bool = raw as? Bool {
            return bool
        }
        if let number = numberValue(raw) {
            return number > 0.5
        }
        if let string = raw as? String {
            let trueWords = ["true", "on", "yes", "1", descriptor.trueLabel?.lowercased() ?? ""]
            return trueWords.contains(string.lowercased())
        }
        return descriptor.defaultBool ?? false
    }

    private func stringValue(for descriptor: PluginParameterDescriptor) -> String {
        if let string = rawValue(for: descriptor) as? String {
            return string
        }
        return ""
    }

    private func choiceIndex(for descriptor: PluginParameterDescriptor) -> Int {
        if pluginType == "crossfeed" && descriptor.key == "crossfeed_mode" {
            return crossfeedModeIndex()
        }
        if pluginType == "crossfeed" && descriptor.key == "crossfeed_preset" {
            return crossfeedPresetIndex()
        }

        let choices = descriptor.choices ?? []
        let raw = rawValue(for: descriptor)
        if let string = raw as? String,
           let index = choices.firstIndex(of: string) {
            return index
        }
        return Int((numberValue(raw) ?? descriptor.defaultDouble ?? 0.0).rounded())
            .clamped(to: 0...max(choices.count - 1, 0))
    }

    private func choiceValue(for descriptor: PluginParameterDescriptor, index: Int) -> Any {
        let choices = descriptor.choices ?? []
        if pluginType == "crossfeed" && (descriptor.key == "crossfeed_mode" || descriptor.key == "crossfeed_preset") {
            return index
        }
        if descriptor.key == "speaker_config", let selected = choices[safe: index] {
            return selected
        }
        if let current = rawValue(for: descriptor) as? String,
           choices.contains(current),
           let selected = choices[safe: index] {
            return selected
        }
        return index
    }

    private func rawValue(for descriptor: PluginParameterDescriptor) -> Any? {
        if pluginType == "crossfeed" {
            switch descriptor.key {
            case "crossfeed_mode":
                return draftParameters["crossfeed_mode"] ?? draftParameters["mode"]
            case "crossfeed_preset":
                return draftParameters["crossfeed_preset"] ?? draftParameters["preset"]
            default:
                break
            }
        }
        return draftParameters[descriptor.key]
    }

    private func normalizedInitialParameters(_ parameters: [String: Any]) -> [String: Any] {
        guard pluginType == "crossfeed" else {
            return parameters
        }

        var normalized = parameters
        let parsedModeIndex = crossfeedModeIndex(from: normalized["crossfeed_mode"] ?? normalized["mode"]) ?? 3
        let modeIndex = parsedModeIndex == 0 ? 3 : parsedModeIndex
        if parsedModeIndex == 0 {
            normalized["enabled"] = false
        }
        let presetIndex = crossfeedPresetIndex(from: normalized["crossfeed_preset"] ?? normalized["preset"]) ?? 0
        normalized["crossfeed_mode"] = modeIndex
        normalized["mode"] = crossfeedModeValue(for: modeIndex)
        normalized["crossfeed_preset"] = presetIndex
        normalized["preset"] = crossfeedPresetValue(for: presetIndex)
        return normalized
    }

    private func updateCrossfeedMode(_ index: Int) {
        let modeIndex = index.clamped(to: 1...3)
        draftParameters["crossfeed_mode"] = modeIndex
        draftParameters["mode"] = crossfeedModeValue(for: modeIndex)
        onUpdate(draftParameters)
    }

    private func multibandBandCount() -> Int {
        guard let descriptor = descriptors.first(where: { $0.key == "num_bands" }) else {
            return 3
        }

        let rawCount = numberValue(rawValue(for: descriptor)) ?? descriptor.defaultDouble ?? descriptor.min ?? 3.0
        let minBands = max(Int((descriptor.min ?? 2.0).rounded()), 1)
        let maxBands = max(Int((descriptor.max ?? Double(minBands)).rounded()), minBands)
        return Int(rawCount.rounded()).clamped(to: minBands...maxBands)
    }

    private func crossoverFrequencyIndex(for key: String) -> Int? {
        let prefix = "crossover_freq_"
        guard key.hasPrefix(prefix) else {
            return nil
        }
        return Int(String(key.dropFirst(prefix.count)))
    }

    private func crossfeedAlgorithmModeIndex() -> Int {
        let mode = crossfeedModeIndex()
        return mode == 0 ? 3 : mode.clamped(to: 1...3)
    }

    private func crossfeedModeIndex() -> Int {
        crossfeedModeIndex(from: draftParameters["crossfeed_mode"] ?? draftParameters["mode"]) ?? 3
    }

    private func crossfeedModeIndex(from raw: Any?) -> Int? {
        if let number = numberValue(raw) {
            return Int(number.rounded()).clamped(to: 0...3)
        }
        guard let string = raw as? String else {
            return nil
        }

        switch string.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() {
        case "disable", "disabled", "off":
            return 0
        case "bauer":
            return 1
        case "meier":
            return 2
        case "multiband", "mb":
            return 3
        default:
            return nil
        }
    }

    private func crossfeedModeValue(for index: Int) -> String {
        switch index.clamped(to: 0...3) {
        case 0:
            return "Off"
        case 1:
            return "Bauer"
        case 2:
            return "Meier"
        default:
            return "Mb"
        }
    }

    private func crossfeedPresetIndex() -> Int {
        crossfeedPresetIndex(from: draftParameters["crossfeed_preset"] ?? draftParameters["preset"]) ?? 0
    }

    private func crossfeedPresetIndex(from raw: Any?) -> Int? {
        if let number = numberValue(raw) {
            return Int(number.rounded()).clamped(to: 0...4)
        }
        guard let string = raw as? String else {
            return nil
        }

        switch string.trimmingCharacters(in: .whitespacesAndNewlines).lowercased() {
        case "default":
            return 0
        case "cmoy":
            return 1
        case "meier":
            return 2
        case "mb", "multiband":
            return 3
        case "off", "disable", "disabled":
            return 4
        default:
            return nil
        }
    }

    private func crossfeedPresetValue(for index: Int) -> String {
        switch index.clamped(to: 0...4) {
        case 1:
            return "Cmoy"
        case 2:
            return "Meier"
        case 3:
            return "Mb"
        case 4:
            return "Off"
        default:
            return "Default"
        }
    }

    private func numberValue(_ raw: Any?) -> Double? {
        if let double = raw as? Double {
            return double
        }
        if let int = raw as? Int {
            return Double(int)
        }
        if let number = raw as? NSNumber {
            return number.doubleValue
        }
        if let string = raw as? String {
            return Double(string)
        }
        return nil
    }
}

private extension Array {
    subscript(safe index: Int) -> Element? {
        indices.contains(index) ? self[index] : nil
    }
}

private extension Comparable {
    func clamped(to range: ClosedRange<Self>) -> Self {
        Swift.min(Swift.max(self, range.lowerBound), range.upperBound)
    }
}

// MARK: - Generic Plugin Editor (fallback)

struct GenericPluginEditor: View {
    let pluginType: String
    let parameters: [String: Any]
    let onUpdate: ([String: Any]) -> Void

    @State private var jsonText: String = ""
    @State private var parseError: String? = nil

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            Text("Parameters (JSON):")
                .font(.caption)
                .foregroundColor(.secondary)

            TextEditor(text: $jsonText)
                .font(.system(.body, design: .monospaced))
                .frame(minHeight: 80, maxHeight: 200)
                .border(Color.gray.opacity(0.3))

            HStack {
                if let error = parseError {
                    Text(error)
                        .font(.caption)
                        .foregroundColor(.red)
                }
                Spacer()
                Button("Update Draft") {
                    applyJson()
                }
            }
        }
        .onAppear {
            if let data = try? JSONSerialization.data(withJSONObject: parameters, options: .prettyPrinted),
               let str = String(data: data, encoding: .utf8) {
                jsonText = str
            }
        }
    }

    private func applyJson() {
        guard let data = jsonText.data(using: .utf8),
              let parsed = try? JSONSerialization.jsonObject(with: data) as? [String: Any] else {
            parseError = "Invalid JSON"
            return
        }
        parseError = nil
        onUpdate(parsed)
    }
}
