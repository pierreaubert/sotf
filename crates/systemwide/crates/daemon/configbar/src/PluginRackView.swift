import SwiftUI

// MARK: - Plugin Rack View

/// Main plugin rack view showing the current plugin chain with add/edit/remove/reorder
struct PluginRackView: View {
    let client: AudioEngineClient
    let outputChannels: Int
    let refreshTrigger: Int

    @State private var plugins: [PluginInstance] = []
    @State private var availablePlugins: [AvailablePlugin] = []
    @State private var showingAddSheet = false
    @State private var editingIndex: Int? = nil
    @State private var errorMessage: String? = nil
    @State private var updateDebounceTask: DispatchWorkItem? = nil
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
                    onUpdate: { newParams in
                        plugin.parameters = newParams
                        updatePlugin(at: index, parameters: newParams)
                    },
                    onDone: {
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

    private func updatePlugin(at index: Int, parameters: [String: Any]) {
        // Debounce updates (100ms)
        updateDebounceTask?.cancel()
        let task = DispatchWorkItem {
            if !client.updatePlugin(at: index, parameters: parameters) {
                DispatchQueue.main.async {
                    errorMessage = "Failed to update plugin"
                }
            }
        }
        updateDebounceTask = task
        DispatchQueue.global(qos: .userInteractive).asyncAfter(deadline: .now() + 0.1, execute: task)
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
    let onUpdate: ([String: Any]) -> Void
    let onDone: () -> Void

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

                Button("Done") {
                    onDone()
                }
                .keyboardShortcut(.defaultAction)
            }
            .padding()

            Divider()

            ScrollView(.vertical, showsIndicators: true) {
                PluginEditorView(
                    pluginType: plugin.pluginType,
                    parameters: plugin.parameters,
                    descriptors: descriptors,
                    onUpdate: onUpdate
                )
                .padding(20)
                .frame(maxWidth: .infinity, alignment: .topLeading)
            }

            Divider()

            HStack {
                Spacer()
                Button("Close") {
                    onDone()
                }
                .keyboardShortcut(.cancelAction)
            }
            .padding()
        }
        .frame(minWidth: 720, idealWidth: 780, minHeight: 520, idealHeight: 620)
    }

    private func parameterSummary() -> String {
        let count = plugin.parameters.count
        return count > 0 ? "\(count) parameter\(count == 1 ? "" : "s")" : "Default parameters"
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
