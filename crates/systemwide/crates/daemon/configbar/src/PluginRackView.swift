import SwiftUI

// MARK: - Plugin Rack View

/// Main plugin rack view showing the current plugin chain with add/edit/remove/reorder
struct PluginRackView: View {
    let client: AudioEngineClient
    let outputChannels: Int

    @State private var plugins: [PluginInstance] = []
    @State private var availablePlugins: [AvailablePlugin] = []
    @State private var showingAddSheet = false
    @State private var editingIndex: Int? = nil
    @State private var errorMessage: String? = nil
    @State private var updateDebounceTask: DispatchWorkItem? = nil

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
                            isEditing: editingIndex == index,
                            onToggleEdit: {
                                withAnimation {
                                    editingIndex = editingIndex == index ? nil : index
                                }
                            },
                            onRemove: {
                                removePlugin(at: index)
                            },
                            onUpdateParams: { newParams in
                                updatePlugin(at: index, parameters: newParams)
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
            refreshPlugins()
        }
        .sheet(isPresented: $showingAddSheet) {
            AddPluginSheet(
                availablePlugins: availablePlugins,
                onAdd: { pluginType in
                    addPlugin(type: pluginType)
                    showingAddSheet = false
                },
                onCancel: {
                    showingAddSheet = false
                }
            )
        }
    }

    // MARK: - Actions

    private func refreshPlugins() {
        errorMessage = nil
        if let result = client.getPlugins() {
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

    private func loadAvailablePlugins() {
        if let result = client.getAvailablePlugins() {
            availablePlugins = result
        }
    }

    private func addPlugin(type: String) {
        errorMessage = nil
        if client.addPlugin(type: type, parameters: [:], index: nil) {
            refreshPlugins()
        } else {
            errorMessage = "Failed to add plugin"
        }
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
    let isEditing: Bool
    let onToggleEdit: () -> Void
    let onRemove: () -> Void
    let onUpdateParams: ([String: Any]) -> Void

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

                Button(action: onToggleEdit) {
                    Image(systemName: isEditing ? "chevron.up" : "slider.horizontal.3")
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

            if isEditing {
                Divider()
                PluginEditorView(
                    pluginType: plugin.pluginType,
                    parameters: plugin.parameters,
                    onUpdate: onUpdateParams
                )
                .padding(.leading, 20)
                .padding(.vertical, 4)
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

// MARK: - Add Plugin Sheet

struct AddPluginSheet: View {
    let availablePlugins: [AvailablePlugin]
    let onAdd: (String) -> Void
    let onCancel: () -> Void

    @State private var searchText = ""

    var categories: [PluginCategory] {
        let filtered: [AvailablePlugin]
        if searchText.isEmpty {
            filtered = availablePlugins
        } else {
            filtered = availablePlugins.filter {
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

            // Plugin list by category
            List {
                ForEach(categories) { category in
                    Section(header: Text(category.name)) {
                        ForEach(category.plugins) { plugin in
                            Button(action: { onAdd(plugin.type_) }) {
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
                            }
                            .buttonStyle(.plain)
                        }
                    }
                }
            }
            .listStyle(.sidebar)
        }
        .frame(width: 450, height: 500)
    }
}
