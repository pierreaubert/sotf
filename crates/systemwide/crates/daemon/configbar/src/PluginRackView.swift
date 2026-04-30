import SwiftUI
import AppKit
import UniformTypeIdentifiers

// MARK: - Plugin Rack View

/// Main plugin rack view showing the current plugin chain with add/edit/remove/reorder
struct PluginRackView: View {
    let client: AudioEngineClient
    let outputChannels: Int
    let availableOutputChannels: Int?
    let refreshTrigger: Int

    @State private var plugins: [PluginInstance] = []
    @State private var availablePlugins: [AvailablePlugin] = []
    @State private var showingAddSheet = false
    @State private var editingIndex: Int? = nil
    @State private var errorMessage: String? = nil
    @State private var loadingAvailablePlugins = false
    @State private var refreshingPlugins = false

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Header
            HStack {
                Text("Signal Chain")
                    .font(.headline)
                Text("(\(plugins.count) plugins)")
                    .font(.caption)
                    .foregroundColor(.secondary)

                Spacer()

                Button(action: { refreshPlugins() }) {
                    Image(systemName: "arrow.clockwise")
                }
                .buttonStyle(.borderless)
                .help("Refresh plugin list")

                Button(action: {
                    loadAvailablePlugins()
                    showingAddSheet = true
                }) {
                    Label("Add Plugin", systemImage: "plus.circle")
                }
                .buttonStyle(.borderless)
            }

            if let error = errorMessage {
                Text(error)
                    .font(.caption)
                    .foregroundColor(.red)
                    .padding(.vertical, 2)
            }

            if let warning = channelCompatibilityWarning {
                Label {
                    Text(warning)
                        .font(.caption)
                        .foregroundColor(.orange)
                        .fixedSize(horizontal: false, vertical: true)
                } icon: {
                    Image(systemName: "exclamationmark.triangle.fill")
                        .foregroundColor(.orange)
                }
                .padding(.vertical, 4)
            }

            // Plugin list
            if plugins.isEmpty {
                HStack {
                    Spacer()
                    VStack(spacing: 4) {
                        Image(systemName: "slider.horizontal.3")
                            .font(.title2)
                            .foregroundColor(.secondary)
                        Text("No plugins in chain")
                            .foregroundColor(.secondary)
                        Text("Audio passes through unprocessed")
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                    .padding(.vertical, 16)
                    Spacer()
                }
            } else {
                List {
                    ForEach(Array(plugins.enumerated()), id: \.element.id) { index, plugin in
                        PluginRowView(
                            plugin: plugin,
                            onEdit: {
                                editingIndex = index
                            },
                            onRemove: {
                                removePlugin(at: index)
                            }
                        )
                    }
                    .onMove { source, destination in
                        movePlugins(from: source, to: destination)
                    }
                }
                .listStyle(.bordered)
                .frame(minHeight: 100, maxHeight: 400)
            }
        }
        .onAppear {
            loadAvailablePlugins()
            refreshPlugins()
        }
        .onChange(of: refreshTrigger) { _, _ in
            refreshPlugins()
        }
        .sheet(isPresented: $showingAddSheet) {
            AddPluginSheet(
                availablePlugins: availablePlugins,
                onAdd: { pluginType, parameters in
                    addPlugin(type: pluginType, parameters: parameters)
                    showingAddSheet = false
                },
                onCancel: {
                    showingAddSheet = false
                }
            )
        }
        .sheet(
            isPresented: Binding(
                get: { editingIndex != nil },
                set: { isPresented in
                    if !isPresented {
                        editingIndex = nil
                    }
                }
            )
        ) {
            if let index = editingIndex, plugins.indices.contains(index) {
                let plugin = plugins[index]
                PluginEditSheet(
                    plugin: plugin,
                    descriptors: descriptors(for: plugin.pluginType),
                    onApply: { newParams in
                        applyPluginUpdate(at: index, parameters: newParams)
                    },
                    onCancel: {
                        editingIndex = nil
                    },
                    onClose: {
                        editingIndex = nil
                    }
                )
            } else {
                EmptyView()
                    .frame(width: 720, height: 520)
            }
        }
    }

    // MARK: - Actions

    private var channelCompatibilityWarning: String? {
        guard let availableOutputChannels, availableOutputChannels > 0 else {
            return nil
        }

        let required = max(outputChannels, plugins.map { requiredOutputChannels(for: $0) }.max() ?? outputChannels)
        guard required > availableOutputChannels else {
            return nil
        }

        let limitingPlugins = plugins
            .filter { requiredOutputChannels(for: $0) > availableOutputChannels }
            .map { $0.pluginName }

        let subject = limitingPlugins.isEmpty
            ? "The plugin chain"
            : limitingPlugins.prefix(2).joined(separator: ", ")
        let suffix = limitingPlugins.count > 2 ? " and \(limitingPlugins.count - 2) more" : ""

        return "\(subject)\(suffix) wants \(required) output channels, but the selected output device exposes \(availableOutputChannels). Choose a higher-channel interface or reduce the plugin output layout."
    }

    private func requiredOutputChannels(for plugin: PluginInstance) -> Int {
        let params = plugin.parameters

        if let channels = intValue(params["output_channels"]) ?? intValue(params["physical_output_channels"]) {
            return channels
        }

        if let map = params["output_channel_map"] as? [Any] {
            let maxIndex = map.compactMap(intValue).max()
            if let maxIndex {
                return maxIndex + 1
            }
        }

        if plugin.pluginType == "upmixer" || plugin.pluginType == "aae",
           let channels = speakerConfigChannels(params["speaker_config"]) {
            return channels
        }

        return outputChannels
    }

    private func intValue(_ raw: Any?) -> Int? {
        if let int = raw as? Int {
            return int
        }
        if let double = raw as? Double {
            return Int(double)
        }
        if let number = raw as? NSNumber {
            return number.intValue
        }
        if let string = raw as? String {
            return Int(string)
        }
        return nil
    }

    private func speakerConfigChannels(_ raw: Any?) -> Int? {
        let config: String?
        if let string = raw as? String {
            config = string
        } else if let index = intValue(raw) {
            let choices = ["2.0", "5.0", "5.1", "7.1", "5.1.2", "5.1.4", "7.1.2", "7.1.4", "9.1.4", "9.1.6"]
            config = choices.indices.contains(index) ? choices[index] : nil
        } else {
            config = nil
        }

        guard let config else { return nil }

        switch config {
        case "1.0": return 1
        case "2.0": return 2
        case "2.1": return 3
        case "5.0": return 5
        case "5.1": return 6
        case "7.1", "5.1.2": return 8
        case "5.1.4", "7.1.2": return 10
        case "7.1.4": return 12
        case "9.1.4": return 14
        case "9.1.6": return 16
        default: return nil
        }
    }

    private func refreshPlugins() {
        guard !refreshingPlugins else { return }
        refreshingPlugins = true
        errorMessage = nil

        DispatchQueue.global(qos: .utility).async {
            let result = AudioEngineClient().getPlugins()

            DispatchQueue.main.async {
                refreshingPlugins = false
                if let result = result {
                    plugins = result.enumerated().map { index, dict in
                        let type_ = dict["plugin_type"] as? String ?? "unknown"
                        let params = dict["parameters"] as? [String: Any] ?? [:]
                        return PluginInstance(
                            index: index,
                            pluginType: type_,
                            pluginName: pluginDisplayName(type_),
                            parameters: params
                        )
                    }
                } else {
                    errorMessage = "Failed to fetch plugins from daemon"
                }
            }
        }
    }

    private func loadAvailablePlugins() {
        guard !loadingAvailablePlugins else { return }
        loadingAvailablePlugins = true

        DispatchQueue.global(qos: .utility).async {
            let result = AudioEngineClient().getAvailablePlugins()

            DispatchQueue.main.async {
                loadingAvailablePlugins = false
                if let result = result {
                    availablePlugins = result.filter { $0.type_ != "fletcher_munson" }
                }
            }
        }
    }

    private func addPlugin(type: String, parameters: [String: Any]? = nil) {
        errorMessage = nil
        let pluginParameters = parameters ?? availablePlugins.first { $0.type_ == type }?.defaultParameters ?? [:]
        if client.addPlugin(type: type, parameters: pluginParameters, index: nil) {
            refreshPlugins()
        } else {
            errorMessage = "Failed to add plugin"
        }
    }

    private func descriptors(for pluginType: String) -> [PluginParameterDescriptor] {
        availablePlugins.first { $0.type_ == pluginType }?.parameters ?? []
    }

    private func removePlugin(at index: Int) {
        errorMessage = nil
        if editingIndex == index {
            editingIndex = nil
        }
        if client.removePlugin(at: index) {
            refreshPlugins()
        } else {
            errorMessage = "Failed to remove plugin"
        }
    }

    private func applyPluginUpdate(at index: Int, parameters: [String: Any]) -> Bool {
        errorMessage = nil
        guard plugins.indices.contains(index) else {
            errorMessage = "Plugin is no longer in the chain"
            return false
        }

        guard client.updatePlugin(at: index, parameters: parameters) else {
            errorMessage = "Failed to update plugin"
            return false
        }

        plugins[index].parameters = parameters
        return true
    }

    private func movePlugins(from source: IndexSet, to destination: Int) {
        errorMessage = nil
        var indices = Array(0..<plugins.count)
        indices.move(fromOffsets: source, toOffset: destination)
        if client.reorderPlugins(order: indices) {
            refreshPlugins()
        } else {
            errorMessage = "Failed to reorder plugins"
        }
    }
}

// MARK: - Plugin Row

struct PluginRowView: View {
    let plugin: PluginInstance
    let onEdit: () -> Void
    let onRemove: () -> Void

    var body: some View {
        VStack(alignment: .leading, spacing: 4) {
            HStack {
                Image(systemName: "line.3.horizontal")
                    .foregroundColor(.secondary)
                    .font(.caption)

                VStack(alignment: .leading, spacing: 1) {
                    Text(plugin.pluginName)
                        .font(.body.weight(.medium))
                    Text(parameterSummary())
                        .font(.caption)
                        .foregroundColor(.secondary)
                        .lineLimit(1)
                }

                Spacer()

                Button(action: onEdit) {
                    Image(systemName: "slider.horizontal.3")
                }
                .buttonStyle(.borderless)
                .help("Edit parameters")

                Button(action: onRemove) {
                    Image(systemName: "trash")
                        .foregroundColor(.red)
                }
                .buttonStyle(.borderless)
                .help("Remove plugin")
            }
        }
        .padding(.vertical, 2)
    }

    private func parameterSummary() -> String {
        let params = plugin.parameters
        switch plugin.pluginType {
        case "gain":
            let db = params["gain_db"] as? Double ?? 0.0
            return String(format: "%.1f dB", db)
        case "eq":
            let filterCount = (params["filters"] as? [Any])?.count ?? 0
            return "\(filterCount) band\(filterCount == 1 ? "" : "s")"
        case "compressor":
            let threshold = params["threshold_db"] as? Double ?? -20.0
            let ratio = params["ratio"] as? Double ?? 4.0
            return String(format: "%.0f dB, %.1f:1", threshold, ratio)
        case "limiter":
            let threshold = params["threshold_db"] as? Double ?? -0.1
            return String(format: "%.1f dB", threshold)
        case "gate":
            let threshold = params["threshold_db"] as? Double ?? -40.0
            return String(format: "%.0f dB", threshold)
        case "upmixer":
            let config = params["speaker_config"] as? String ?? "5.0"
            return config
        default:
            let count = params.count
            return count > 0 ? "\(count) parameter\(count == 1 ? "" : "s")" : "defaults"
        }
    }
}

// MARK: - Edit Plugin Sheet

struct PluginEditSheet: View {
    @ObservedObject var plugin: PluginInstance
    let descriptors: [PluginParameterDescriptor]
    let onApply: ([String: Any]) -> Bool
    let onCancel: () -> Void
    let onClose: () -> Void

    @State private var draftParameters: [String: Any]
    @State private var editorRevision = 0
    @State private var sheetError: String? = nil

    init(
        plugin: PluginInstance,
        descriptors: [PluginParameterDescriptor],
        onApply: @escaping ([String: Any]) -> Bool,
        onCancel: @escaping () -> Void,
        onClose: @escaping () -> Void
    ) {
        self.plugin = plugin
        self.descriptors = descriptors
        self.onApply = onApply
        self.onCancel = onCancel
        self.onClose = onClose
        _draftParameters = State(initialValue: plugin.parameters)
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack {
                VStack(alignment: .leading, spacing: 2) {
                    Text(plugin.pluginName)
                        .font(.headline)
                    Text(parameterSummary())
                        .font(.caption)
                        .foregroundColor(.secondary)
                }

                Spacer()
            }
            .padding()

            Divider()

            ScrollView(.vertical, showsIndicators: true) {
                PluginEditorView(
                    pluginType: plugin.pluginType,
                    parameters: draftParameters,
                    descriptors: descriptors,
                    onUpdate: { newParameters in
                        draftParameters = newParameters
                        sheetError = nil
                    }
                )
                .id(editorRevision)
                .padding(20)
                .frame(maxWidth: .infinity, alignment: .topLeading)
            }

            Divider()

            VStack(alignment: .leading, spacing: 8) {
                if let sheetError {
                    Text(sheetError)
                        .font(.caption)
                        .foregroundColor(.red)
                }

                HStack {
                    Button("Load") {
                        loadParameters()
                    }

                    Button("Save") {
                        saveParameters()
                    }

                    Spacer()

                    Button("Apply") {
                        applyDraft(closeAfterApply: false)
                    }

                    Button("Cancel") {
                        onCancel()
                    }
                    .keyboardShortcut(.cancelAction)

                    Button("Close") {
                        applyDraft(closeAfterApply: true)
                    }
                    .keyboardShortcut(.defaultAction)
                }
            }
            .padding()
        }
        .frame(minWidth: 720, idealWidth: 780, minHeight: 520, idealHeight: 620)
    }

    private func parameterSummary() -> String {
        let count = draftParameters.count
        return count > 0 ? "\(count) parameter\(count == 1 ? "" : "s")" : "Default parameters"
    }

    private func applyDraft(closeAfterApply: Bool) {
        if onApply(draftParameters) {
            sheetError = nil
            if closeAfterApply {
                onClose()
            }
        } else {
            sheetError = "Failed to apply parameters to the plugin chain"
        }
    }

    private func loadParameters() {
        let panel = NSOpenPanel()
        panel.allowedContentTypes = [.json]
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        panel.message = "Select \(plugin.pluginName) parameter file"

        guard panel.runModal() == .OK, let url = panel.url else {
            return
        }

        do {
            let data = try Data(contentsOf: url)
            let json = try JSONSerialization.jsonObject(with: data)
            draftParameters = try parametersFromSupportedJSON(json)
            editorRevision += 1
            sheetError = nil
        } catch {
            sheetError = "Failed to load parameters: \(error.localizedDescription)"
        }
    }

    private func saveParameters() {
        let panel = NSSavePanel()
        panel.allowedContentTypes = [.json]
        panel.nameFieldStringValue = "\(plugin.pluginType)_parameters.json"
        panel.message = "Save \(plugin.pluginName) parameters"

        guard panel.runModal() == .OK, let url = panel.url else {
            return
        }

        let savedPlugin: [String: Any] = [
            "plugin_type": plugin.pluginType,
            "plugin_name": plugin.pluginName,
            "parameters": draftParameters,
        ]

        do {
            let data = try JSONSerialization.data(
                withJSONObject: savedPlugin,
                options: [.prettyPrinted, .sortedKeys]
            )
            try data.write(to: url)
            sheetError = nil
        } catch {
            sheetError = "Failed to save parameters: \(error.localizedDescription)"
        }
    }

    private func parametersFromSupportedJSON(_ json: Any) throws -> [String: Any] {
        if let entries = json as? [[String: Any]] {
            if let parameters = parametersFromPluginEntries(entries) {
                return parameters
            }
            throw pluginParameterFileError("No matching plugin parameters found")
        }

        guard let dict = json as? [String: Any] else {
            throw pluginParameterFileError("Invalid JSON parameter format")
        }

        if let parameters = dict["parameters"] as? [String: Any] {
            if let filePluginType = (dict["plugin_type"] as? String) ?? (dict["type"] as? String),
               filePluginType != plugin.pluginType {
                throw pluginParameterFileError("Parameter file is for \(pluginDisplayName(filePluginType)), not \(plugin.pluginName)")
            }
            return parameters
        }

        if let settings = dict["settings"],
           let record = pluginTypeAndParameters(fromAppGpuiSettings: settings) {
            guard record.pluginType == plugin.pluginType else {
                throw pluginParameterFileError("Parameter file is for \(pluginDisplayName(record.pluginType)), not \(plugin.pluginName)")
            }
            return record.parameters
        }

        if let plugins = dict["plugins"] as? [[String: Any]],
           let parameters = parametersFromPluginEntries(plugins) {
            return parameters
        }

        var pluginEntries: [[String: Any]] = []
        if let globalPlugins = dict["global_plugins"] as? [[String: Any]] {
            pluginEntries.append(contentsOf: globalPlugins)
        }
        if let channels = dict["channels"] as? [String: Any] {
            pluginEntries.append(contentsOf: pluginEntriesFromChannels(channels))
        }
        if !pluginEntries.isEmpty, let parameters = parametersFromPluginEntries(pluginEntries) {
            return parameters
        }

        let wrapperKeys: Set<String> = ["plugin_type", "type", "settings", "plugins", "global_plugins", "channels"]
        if dict.keys.contains(where: { wrapperKeys.contains($0) }) {
            throw pluginParameterFileError("No parameters found for \(plugin.pluginName)")
        }

        return dict
    }

    private func parametersFromPluginEntries(_ entries: [[String: Any]]) -> [String: Any]? {
        let records = entries.compactMap(pluginTypeAndParameters)
        if let matchingRecord = records.first(where: { $0.pluginType == plugin.pluginType }) {
            return matchingRecord.parameters
        }
        return records.count == 1 ? records[0].parameters : nil
    }

    private func pluginTypeAndParameters(from entry: [String: Any]) -> (pluginType: String, parameters: [String: Any])? {
        if let settings = entry["settings"] {
            return pluginTypeAndParameters(fromAppGpuiSettings: settings)
        }

        guard let pluginType = (entry["plugin_type"] as? String) ?? (entry["type"] as? String) else {
            return nil
        }

        return (pluginType, entry["parameters"] as? [String: Any] ?? [:])
    }

    private func pluginTypeAndParameters(fromAppGpuiSettings settings: Any) -> (pluginType: String, parameters: [String: Any])? {
        if let variantName = settings as? String,
           let pluginType = appGpuiSettingsVariantToEngineType[variantName] {
            return (pluginType, [:])
        }

        guard let settingsDict = settings as? [String: Any],
              let first = settingsDict.first,
              let pluginType = appGpuiSettingsVariantToEngineType[first.key] else {
            return nil
        }

        return (pluginType, first.value as? [String: Any] ?? [:])
    }

    private func pluginEntriesFromChannels(_ channels: [String: Any]) -> [[String: Any]] {
        var entries: [[String: Any]] = []
        for channelName in channels.keys.sorted() {
            guard let channelData = channels[channelName] as? [String: Any],
                  let plugins = channelData["plugins"] as? [[String: Any]] else {
                continue
            }
            entries.append(contentsOf: plugins)
        }
        return entries
    }

    private func pluginParameterFileError(_ message: String) -> NSError {
        NSError(
            domain: "org.spinorama.sotf.configbar.plugin-parameters",
            code: 1,
            userInfo: [NSLocalizedDescriptionKey: message]
        )
    }
}

// MARK: - Add Plugin Sheet

struct AddPluginSheet: View {
    let availablePlugins: [AvailablePlugin]
    let onAdd: (String, [String: Any]) -> Void
    let onCancel: () -> Void

    @State private var searchText = ""
    @State private var selectedPluginID: String? = nil
    @State private var draftParameters: [String: Any] = [:]

    private var visiblePlugins: [AvailablePlugin] {
        availablePlugins.filter { $0.type_ != "fletcher_munson" }
    }

    private var selectedPlugin: AvailablePlugin? {
        guard let selectedPluginID else {
            return nil
        }
        return visiblePlugins.first { $0.type_ == selectedPluginID }
    }

    var categories: [PluginCategory] {
        let filtered: [AvailablePlugin]
        if searchText.isEmpty {
            filtered = visiblePlugins
        } else {
            filtered = visiblePlugins.filter {
                $0.name.localizedCaseInsensitiveContains(searchText) ||
                $0.description.localizedCaseInsensitiveContains(searchText) ||
                $0.category.localizedCaseInsensitiveContains(searchText)
            }
        }
        return groupPluginsByCategory(filtered)
    }

    var body: some View {
        VStack(spacing: 0) {
            // Header
            HStack {
                Text("Add Plugin")
                    .font(.headline)
                Spacer()
                Button("Cancel") { onCancel() }
                    .keyboardShortcut(.cancelAction)
            }
            .padding()

            // Search
            TextField("Search plugins...", text: $searchText)
                .textFieldStyle(.roundedBorder)
                .padding(.horizontal)

            HStack(spacing: 0) {
                // Plugin list by category
                List {
                    ForEach(categories) { category in
                        Section(header: Text(category.name)) {
                            ForEach(category.plugins) { plugin in
                                Button(action: { selectPlugin(plugin) }) {
                                    VStack(alignment: .leading, spacing: 2) {
                                        HStack {
                                            Text(plugin.name)
                                                .font(.body.weight(.medium))
                                            if plugin.maturity != "Prod" {
                                                Text(plugin.maturity)
                                                    .font(.caption2)
                                                    .padding(.horizontal, 4)
                                                    .padding(.vertical, 1)
                                                    .background(plugin.maturity == "Beta" ? Color.blue.opacity(0.2) : Color.orange.opacity(0.2))
                                                    .cornerRadius(3)
                                            }
                                        }
                                        Text(plugin.description)
                                            .font(.caption)
                                            .foregroundColor(.secondary)
                                    }
                                    .frame(maxWidth: .infinity, alignment: .leading)
                                    .contentShape(Rectangle())
                                }
                                .buttonStyle(.plain)
                                .listRowBackground(
                                    selectedPluginID == plugin.type_
                                        ? Color.accentColor.opacity(0.16)
                                        : Color.clear
                                )
                            }
                        }
                    }
                }
                .listStyle(.sidebar)
                .frame(width: 300)

                Divider()

                VStack(alignment: .leading, spacing: 0) {
                    if let plugin = selectedPlugin {
                        VStack(alignment: .leading, spacing: 4) {
                            Text(plugin.name)
                                .font(.headline)
                            Text(plugin.description)
                                .font(.caption)
                                .foregroundColor(.secondary)
                        }
                        .padding()

                        Divider()

                        ScrollView(.vertical, showsIndicators: true) {
                            PluginEditorView(
                                pluginType: plugin.type_,
                                parameters: draftParameters,
                                descriptors: plugin.parameters,
                                onUpdate: { draftParameters = $0 }
                            )
                            .id(plugin.type_)
                            .padding(20)
                            .frame(maxWidth: .infinity, alignment: .topLeading)
                        }
                    } else {
                        Spacer()
                        Text("Select a plugin")
                            .foregroundColor(.secondary)
                            .frame(maxWidth: .infinity)
                        Spacer()
                    }
                }
                .frame(maxWidth: .infinity, maxHeight: .infinity)
            }

            Divider()

            HStack {
                Spacer()
                Button("Add") {
                    if let plugin = selectedPlugin {
                        onAdd(plugin.type_, draftParameters)
                    }
                }
                .keyboardShortcut(.defaultAction)
                .disabled(selectedPlugin == nil)
            }
            .padding()
        }
        .frame(width: 840, height: 620)
        .onAppear {
            selectInitialPluginIfNeeded()
        }
        .onChange(of: availablePlugins.count) { _, _ in
            selectInitialPluginIfNeeded()
        }
    }

    private func selectInitialPluginIfNeeded() {
        guard selectedPluginID == nil, let first = categories.first?.plugins.first else {
            return
        }
        selectPlugin(first)
    }

    private func selectPlugin(_ plugin: AvailablePlugin) {
        selectedPluginID = plugin.type_
        draftParameters = plugin.defaultParameters
    }
}
