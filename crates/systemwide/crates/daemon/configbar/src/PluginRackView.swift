import SwiftUI
import AppKit
import UniformTypeIdentifiers
import ConfigBarModels

// MARK: - Plugin Rack View

/// Main plugin rack view showing the current plugin chain with add/edit/remove/reorder
struct PluginRackView: View {
    let client: AudioEngineClient
    let outputChannels: Int
    let availableOutputChannels: Int?
    let refreshTrigger: Int

    @State private var plugins: [PluginInstance] = []
    @State private var graph: PluginGraphModel? = nil
    @State private var graphGeneration: Int? = nil
    @State private var availablePlugins: [AvailablePlugin] = []
    @State private var showingAddSheet = false
    @State private var editingPluginID: UUID? = nil
    @State private var errorMessage: String? = nil
    @State private var loadingAvailablePlugins = false
    @State private var refreshingPlugins = false
    @State private var graphRevision = 0

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            // Header
            HStack {
                Text("Signal Chain")
                    .font(.headline)
                Text(
                    graph.map { "(\($0.nodes.count) graph nodes)" }
                        ?? "(\(plugins.count) plugins)"
                )
                    .font(.caption)
                    .foregroundColor(.secondary)

                Spacer()

                Button(action: { refreshPlugins() }) {
                    if refreshingPlugins {
                        ProgressView()
                            .controlSize(.small)
                    } else {
                        Image(systemName: "arrow.clockwise")
                    }
                }
                .buttonStyle(.borderless)
                .disabled(refreshingPlugins)
                .help("Refresh plugin list")

                if graph == nil {
                    Button(action: {
                        showingAddSheet = true
                        loadAvailablePlugins()
                    }) {
                        if loadingAvailablePlugins {
                            ProgressView()
                                .controlSize(.small)
                            Text("Loading Plugins…")
                        } else {
                            Label("Add Plugin", systemImage: "plus.circle")
                        }
                    }
                    .buttonStyle(.borderless)
                    .disabled(loadingAvailablePlugins)
                }
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
            if let graph {
                if graph.isLinear {
                    LinearGraphRackEditorView(
                        graph: graph,
                        availablePlugins: availablePlugins,
                        onApply: applyGraphMutation,
                        onReorder: reorderGraph
                    )
                    .id(graphRevision)
                } else {
                    PluginGraphEditorView(
                        graph: graph,
                        availablePlugins: availablePlugins,
                        onApply: applyGraphMutation
                    )
                    .id(graphRevision)
                }
            } else if refreshingPlugins && plugins.isEmpty {
                HStack {
                    Spacer()
                    VStack(spacing: 8) {
                        ProgressView()
                            .controlSize(.small)
                        Text("Loading signal chain…")
                            .font(.caption)
                            .foregroundColor(.secondary)
                    }
                    .padding(.vertical, 24)
                    Spacer()
                }
            } else if plugins.isEmpty {
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
                            requiredOutputChannels: requiredOutputChannels(for: plugin),
                            onEdit: {
                                editingPluginID = plugin.id
                            },
                            onRemove: {
                                removePlugin(at: index)
                            },
                            onStateChange: { inputChannels, bypassed in
                                setRackPluginState(
                                    for: plugin.id,
                                    inputChannels: inputChannels,
                                    bypassed: bypassed
                                )
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
        .onChange(of: refreshTrigger) { _ in
            refreshPlugins()
        }
        .sheet(isPresented: $showingAddSheet) {
            AddPluginSheet(
                availablePlugins: availablePlugins,
                isLoading: loadingAvailablePlugins,
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
                get: { editingPluginID != nil },
                set: { isPresented in
                    if !isPresented {
                        editingPluginID = nil
                    }
                }
            )
        ) {
            if let pluginID = editingPluginID,
               let index = plugins.firstIndex(where: { $0.id == pluginID }) {
                let plugin = plugins[index]
                PluginEditSheet(
                    plugin: plugin,
                    descriptors: descriptors(for: plugin.pluginType),
                    onApply: { newParams in
                        applyPluginUpdate(for: pluginID, parameters: newParams)
                    },
                    onCancel: {
                        editingPluginID = nil
                    },
                    onClose: {
                        editingPluginID = nil
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
            let result = AudioEngineClient().getPluginPipeline()

            DispatchQueue.main.async {
                refreshingPlugins = false
                if let result = result {
                    graph = result.graph
                    graphGeneration = result.generation
                    graphRevision += 1
                    plugins = result.plugins.enumerated().map { index, dict in
                        let type_ = dict["plugin_type"] as? String ?? "unknown"
                        let params = dict["parameters"] as? [String: Any] ?? [:]
                        return PluginInstance(
                            index: index,
                            pluginType: type_,
                            pluginName: pluginDisplayName(type_),
                            parameters: params,
                            inputChannels: max(1, dict["input_channels"] as? Int ?? 1),
                            bypassed: dict["bypassed"] as? Bool ?? false
                        )
                    }
                } else {
                    errorMessage = "Failed to fetch plugins from daemon"
                }
            }
        }
    }

    private func applyGraphMutation(_ candidate: PluginGraphModel) -> Bool {
        errorMessage = nil
        var command: [String: Any] = [
            "command": "load_plugin_artifact",
            "artifact": ["graph": candidate.artifact],
        ]
        if let graphGeneration {
            command["base_generation"] = graphGeneration
        }
        client.sendCommandAsync(command) { response in
            guard response?.success == true else {
                errorMessage = response?.error ?? "Graph validation or engine apply failed; refresh before retrying."
                refreshPlugins()
                return
            }
            graph = candidate
            refreshPlugins()
        }
        // Keep the editor responsive while the serialized mutation is in
        // flight; the completion above reconciles the authoritative graph.
        graph = candidate
        return true
    }

    private func reorderGraph(_ order: [Int]) -> Bool {
        errorMessage = nil
        var command: [String: Any] = [
            "command": "reorder_graph",
            "order": order,
        ]
        if let graphGeneration {
            command["base_generation"] = graphGeneration
        }

        client.sendCommandAsync(command) { response in
            if response?.success != true {
                errorMessage = response?.error ?? "Graph reorder failed; refresh before retrying."
            }
            // The daemon owns graph order and generation. Reconcile both
            // success and failure with a fresh authoritative response.
            refreshPlugins()
        }
        return true
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
        client.sendCommandAsync([
            "command": "add_plugin",
            "plugin": ["plugin_type": type, "parameters": pluginParameters],
        ]) { response in
            if response?.success == true {
                refreshPlugins()
            } else {
                errorMessage = response?.error ?? "Failed to add plugin"
            }
        }
    }

    private func descriptors(for pluginType: String) -> [PluginParameterDescriptor] {
        availablePlugins.first { $0.type_ == pluginType }?.parameters ?? []
    }

    private func removePlugin(at index: Int) {
        errorMessage = nil
        if plugins.indices.contains(index),
           editingPluginID == plugins[index].id {
            editingPluginID = nil
        }
        client.sendCommandAsync(["command": "remove_plugin", "index": index]) { response in
            if response?.success == true {
                refreshPlugins()
            } else {
                errorMessage = response?.error ?? "Failed to remove plugin"
            }
        }
    }

    private func applyPluginUpdate(for pluginID: UUID, parameters: [String: Any]) -> Bool {
        errorMessage = nil
        guard let index = plugins.firstIndex(where: { $0.id == pluginID }) else {
            errorMessage = "Plugin is no longer in the chain"
            return false
        }

        plugins[index].parameters = parameters
        client.sendCommandAsync([
            "command": "update_plugin",
            "index": index,
            "parameters": parameters,
        ]) { response in
            if response?.success != true {
                errorMessage = response?.error ?? "Failed to update plugin"
                refreshPlugins()
            }
        }
        return true
    }

    private func movePlugins(from source: IndexSet, to destination: Int) {
        errorMessage = nil
        var indices = Array(0..<plugins.count)
        indices.move(fromOffsets: source, toOffset: destination)
        client.sendCommandAsync(["command": "reorder_plugins", "order": indices]) { response in
            if response?.success == true {
                refreshPlugins()
            } else {
                errorMessage = response?.error ?? "Failed to reorder plugins"
            }
        }
    }

    private func setRackPluginState(
        for pluginID: UUID,
        inputChannels: Int?,
        bypassed: Bool?
    ) {
        guard let index = plugins.firstIndex(where: { $0.id == pluginID }) else {
            errorMessage = "Plugin is no longer in the rack"
            return
        }
        guard inputChannels != nil || bypassed != nil else { return }

        if let inputChannels {
            plugins[index].inputChannels = max(1, min(inputChannels, 32))
        }
        if let bypassed {
            plugins[index].bypassed = bypassed
        }

        var command: [String: Any] = [
            "command": "set_rack_plugin_state",
            "index": index,
        ]
        if let inputChannels {
            command["input_channels"] = max(1, min(inputChannels, 32))
        }
        if let bypassed {
            command["bypassed"] = bypassed
        }
        if let graphGeneration {
            command["base_generation"] = graphGeneration
        }

        client.sendCommandAsync(command) { response in
            if response?.success != true {
                errorMessage = response?.error ?? "Failed to update plugin state"
            }
            // A successful state patch promotes the rack to a graph. Refresh
            // on both paths so the UI always reflects daemon-owned state.
            refreshPlugins()
        }
    }
}

// MARK: - Plugin Graph Editor

struct LinearGraphRackEditorView: View {
    let graph: PluginGraphModel
    let availablePlugins: [AvailablePlugin]
    let onApply: (PluginGraphModel) -> Bool
    let onReorder: ([Int]) -> Bool

    @State private var draft: PluginGraphModel
    @State private var editingNodeID: Int? = nil
    @State private var graphError: String? = nil

    init(
        graph: PluginGraphModel,
        availablePlugins: [AvailablePlugin],
        onApply: @escaping (PluginGraphModel) -> Bool,
        onReorder: @escaping ([Int]) -> Bool
    ) {
        self.graph = graph
        self.availablePlugins = availablePlugins
        self.onApply = onApply
        self.onReorder = onReorder
        _draft = State(initialValue: graph)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 8) {
            HStack {
                Text("Linear graph").font(.caption).foregroundColor(.secondary)
                Spacer()
                Menu {
                    ForEach(availablePlugins) { plugin in
                        Button(plugin.name) { appendNode(plugin) }
                    }
                } label: {
                    Label("Add Plugin", systemImage: "plus.circle")
                }
            }
            if let graphError {
                Text(graphError).font(.caption).foregroundColor(.orange)
            }
            List {
                ForEach(orderedNodes) { node in
                    HStack {
                        Image(systemName: "line.3.horizontal")
                            .foregroundColor(.secondary)
                        VStack(alignment: .leading, spacing: 1) {
                            Text(node.pluginName).font(.body.weight(.medium))
                            Text("Node \(node.id) · \(node.inputChannels) ch")
                                .font(.caption)
                                .foregroundColor(.secondary)
                        }
                        Spacer()
                        Button {
                            editingNodeID = node.id
                        } label: {
                            Image(systemName: "slider.horizontal.3")
                        }
                        .buttonStyle(.borderless)
                        Button {
                            toggleBypass(node.id)
                        } label: {
                            Image(systemName: node.bypassed ? "power.circle.fill" : "power.circle")
                        }
                        .buttonStyle(.borderless)
                        .foregroundColor(node.bypassed ? .orange : .primary)
                        Button(role: .destructive) {
                            removeNode(node.id)
                        } label: {
                            Image(systemName: "trash")
                        }
                        .buttonStyle(.borderless)
                    }
                }
                .onMove(perform: moveNodes)
            }
            .listStyle(.bordered)
            .frame(minHeight: 120, maxHeight: 400)
        }
        .sheet(
            isPresented: Binding(
                get: { editingNodeID != nil },
                set: { if !$0 { editingNodeID = nil } }
            )
        ) {
            if let nodeID = editingNodeID,
               let node = draft.nodes.first(where: { $0.id == nodeID }) {
                let instance = PluginInstance(
                    index: node.id,
                    pluginType: node.pluginType,
                    pluginName: node.pluginName,
                    parameters: node.parameters
                )
                PluginEditSheet(
                    plugin: instance,
                    descriptors: descriptors(for: node.pluginType),
                    onApply: { updateNode(nodeID, parameters: $0) },
                    onCancel: { editingNodeID = nil },
                    onClose: { editingNodeID = nil }
                )
                .frame(width: 720, height: 520)
            }
        }
    }

    private var orderedNodes: [PluginGraphNodeModel] {
        let byID = Dictionary(uniqueKeysWithValues: draft.nodes.map { ($0.id, $0) })
        return (draft.linearNodeIDs ?? draft.nodes.map(\.id)).compactMap { byID[$0] }
    }

    private func descriptors(for pluginType: String) -> [PluginParameterDescriptor] {
        availablePlugins.first { $0.type_ == pluginType }?.parameters ?? []
    }

    private func commit(_ mutation: (inout PluginGraphModel) -> Void) -> Bool {
        var candidate = draft
        mutation(&candidate)
        guard onApply(candidate) else {
            graphError = "Graph rejected; the active linear graph was not replaced."
            return false
        }
        draft = candidate
        graphError = nil
        return true
    }

    private func rebuildEdges(_ candidate: inout PluginGraphModel, order: [Int]) {
        candidate.edges = zip(order, order.dropFirst()).map {
            PluginGraphEdgeModel(fromNode: $0.0, toNode: $0.1)
        }
    }

    private func appendNode(_ plugin: AvailablePlugin) {
        let previousOrder = draft.linearNodeIDs ?? []
        let nextID = (draft.nodes.map(\.id).max() ?? -1) + 1
        _ = commit { candidate in
            candidate.nodes.append(
                PluginGraphNodeModel(
                    id: nextID,
                    pluginType: plugin.type_,
                    parameters: plugin.defaultParameters,
                    inputChannels: 2,
                    bypassed: false
                )
            )
            rebuildEdges(&candidate, order: previousOrder + [nextID])
        }
    }

    private func removeNode(_ id: Int) {
        let order = (draft.linearNodeIDs ?? []).filter { $0 != id }
        _ = commit { candidate in
            candidate.nodes.removeAll { $0.id == id }
            rebuildEdges(&candidate, order: order)
        }
    }

    private func moveNodes(from source: IndexSet, to destination: Int) {
        var order = draft.linearNodeIDs ?? []
        order.move(fromOffsets: source, toOffset: destination)
        guard onReorder(order) else {
            graphError = "Graph reorder was rejected; the active graph was not changed."
            return
        }

        // Reordering has a dedicated daemon command. Update the local draft
        // only for responsiveness; the parent refresh above replaces it with
        // the authoritative graph (or rolls it back after a rejection).
        var candidate = draft
        rebuildEdges(&candidate, order: order)
        draft = candidate
        graphError = nil
    }

    private func updateNode(_ id: Int, parameters: [String: Any]) -> Bool {
        commit { candidate in
            guard let index = candidate.nodes.firstIndex(where: { $0.id == id }) else { return }
            candidate.nodes[index].parameters = parameters
        }
    }

    private func toggleBypass(_ id: Int) {
        _ = commit { candidate in
            guard let index = candidate.nodes.firstIndex(where: { $0.id == id }) else { return }
            candidate.nodes[index].bypassed.toggle()
        }
    }
}

struct PluginGraphEditorView: View {
    let graph: PluginGraphModel
    let availablePlugins: [AvailablePlugin]
    let onApply: (PluginGraphModel) -> Bool

    @State private var draft: PluginGraphModel
    @State private var editingNodeID: Int? = nil
    @State private var fromNodeID: Int = 0
    @State private var toNodeID: Int = 0
    @State private var graphError: String? = nil

    init(
        graph: PluginGraphModel,
        availablePlugins: [AvailablePlugin],
        onApply: @escaping (PluginGraphModel) -> Bool
    ) {
        self.graph = graph
        self.availablePlugins = availablePlugins
        self.onApply = onApply
        _draft = State(initialValue: graph)
        _fromNodeID = State(initialValue: graph.nodes.first?.id ?? 0)
        _toNodeID = State(initialValue: graph.nodes.dropFirst().first?.id ?? graph.nodes.first?.id ?? 0)
    }

    var body: some View {
        VStack(alignment: .leading, spacing: 10) {
            HStack {
                Label("DSP Graph", systemImage: "point.3.connected.trianglepath.dotted")
                    .font(.headline)
                Text("\(draft.nodes.count) nodes · \(draft.edges.count) connections")
                    .font(.caption)
                    .foregroundColor(.secondary)
                Spacer()
                Menu {
                    ForEach(availablePlugins) { plugin in
                        Button(plugin.name) {
                            addNode(plugin)
                        }
                    }
                } label: {
                    Label("Add Node", systemImage: "plus.circle")
                }
            }

            if let graphError {
                Label(graphError, systemImage: "exclamationmark.triangle.fill")
                    .font(.caption)
                    .foregroundColor(.orange)
                    .fixedSize(horizontal: false, vertical: true)
            }

            HStack(alignment: .top, spacing: 12) {
                List {
                    ForEach(draft.nodes) { node in
                        HStack {
                            VStack(alignment: .leading, spacing: 2) {
                                Text(node.pluginName).font(.body.weight(.medium))
                                Text("Node \(node.id)")
                                    .font(.caption)
                                    .foregroundColor(.secondary)
                                if node.bypassed {
                                    Text("Bypassed")
                                        .font(.caption2)
                                        .foregroundColor(.orange)
                                }
                            }
                            Spacer()
                            Stepper(
                                "\(node.inputChannels) ch",
                                value: Binding(
                                    get: { node.inputChannels },
                                    set: { updateNodeChannels(node.id, channels: $0) }
                                ),
                                in: 1...32
                            )
                            .frame(width: 90)
                            .help("Node input channel count")
                            Button {
                                editingNodeID = node.id
                            } label: {
                                Image(systemName: "slider.horizontal.3")
                            }
                            .buttonStyle(.borderless)
                            Button {
                                toggleBypass(node.id)
                            } label: {
                                Image(systemName: node.bypassed ? "power.circle.fill" : "power.circle")
                            }
                            .buttonStyle(.borderless)
                            .help(node.bypassed ? "Enable node" : "Bypass node")
                            Button(role: .destructive) {
                                removeNode(node.id)
                            } label: {
                                Image(systemName: "trash")
                            }
                            .buttonStyle(.borderless)
                        }
                    }
                    .onMove(perform: moveNodes)
                }
                .listStyle(.bordered)
                .frame(minWidth: 300, minHeight: 180, maxHeight: 380)

                VStack(alignment: .leading, spacing: 8) {
                    Text("Connections").font(.headline)
                    List {
                        ForEach(draft.edges) { edge in
                            HStack {
                                Text("\(edge.fromNode) → \(edge.toNode)")
                                    .font(.system(.body, design: .monospaced))
                                Spacer()
                                Button(role: .destructive) {
                                    disconnect(edge)
                                } label: {
                                    Image(systemName: "xmark.circle")
                                }
                                .buttonStyle(.borderless)
                            }
                        }
                    }
                    .listStyle(.bordered)
                    .frame(minWidth: 220, minHeight: 120, maxHeight: 280)

                    HStack {
                        Picker("From", selection: $fromNodeID) {
                            ForEach(draft.nodes) { node in
                                Text("\(node.id)").tag(node.id)
                            }
                        }
                        Picker("To", selection: $toNodeID) {
                            ForEach(draft.nodes) { node in
                                Text("\(node.id)").tag(node.id)
                            }
                        }
                        Button("Connect") {
                            connect()
                        }
                        .disabled(fromNodeID == toNodeID || draft.nodes.count < 2)
                    }
                }
            }
        }
        .sheet(
            isPresented: Binding(
                get: { editingNodeID != nil },
                set: { if !$0 { editingNodeID = nil } }
            )
        ) {
            if let nodeID = editingNodeID,
               let node = draft.nodes.first(where: { $0.id == nodeID }) {
                let instance = PluginInstance(
                    index: node.id,
                    pluginType: node.pluginType,
                    pluginName: node.pluginName,
                    parameters: node.parameters
                )
                PluginEditSheet(
                    plugin: instance,
                    descriptors: descriptors(for: node.pluginType),
                    onApply: { parameters in
                        updateNode(nodeID, parameters: parameters)
                    },
                    onCancel: { editingNodeID = nil },
                    onClose: { editingNodeID = nil }
                )
                .frame(width: 720, height: 520)
            }
        }
    }

    private func descriptors(for pluginType: String) -> [PluginParameterDescriptor] {
        availablePlugins.first { $0.type_ == pluginType }?.parameters ?? []
    }

    private func commit(_ mutation: (inout PluginGraphModel) -> Void) -> Bool {
        var candidate = draft
        mutation(&candidate)
        guard onApply(candidate) else {
            graphError = "Graph rejected. Check cycles, paths, and channel compatibility; the active graph was not replaced."
            return false
        }
        graphError = nil
        draft = candidate
        return true
    }

    private func addNode(_ plugin: AvailablePlugin) {
        let nextID = (draft.nodes.map(\.id).max() ?? -1) + 1
        _ = commit { candidate in
            candidate.nodes.append(
                PluginGraphNodeModel(
                    id: nextID,
                    pluginType: plugin.type_,
                    parameters: plugin.defaultParameters,
                    inputChannels: 2,
                    bypassed: false
                )
            )
        }
        fromNodeID = draft.nodes.first?.id ?? 0
        toNodeID = draft.nodes.last?.id ?? 0
    }

    private func removeNode(_ id: Int) {
        _ = commit { candidate in
            candidate.nodes.removeAll { $0.id == id }
            candidate.edges.removeAll { $0.fromNode == id || $0.toNode == id }
        }
    }

    private func updateNode(_ id: Int, parameters: [String: Any]) -> Bool {
        commit { candidate in
            guard let index = candidate.nodes.firstIndex(where: { $0.id == id }) else {
                return
            }
            candidate.nodes[index].parameters = parameters
        }
    }

    private func toggleBypass(_ id: Int) {
        _ = commit { candidate in
            guard let index = candidate.nodes.firstIndex(where: { $0.id == id }) else {
                return
            }
            candidate.nodes[index].bypassed.toggle()
        }
    }

    private func updateNodeChannels(_ id: Int, channels: Int) {
        _ = commit { candidate in
            guard let index = candidate.nodes.firstIndex(where: { $0.id == id }) else {
                return
            }
            candidate.nodes[index].inputChannels = channels
        }
    }

    private func connect() {
        let edge = PluginGraphEdgeModel(fromNode: fromNodeID, toNode: toNodeID)
        guard !draft.edges.contains(edge) else { return }
        _ = commit { $0.edges.append(edge) }
    }

    private func disconnect(_ edge: PluginGraphEdgeModel) {
        _ = commit { $0.edges.removeAll { $0 == edge } }
    }

    private func moveNodes(from source: IndexSet, to destination: Int) {
        _ = commit { $0.nodes.move(fromOffsets: source, toOffset: destination) }
    }
}

// MARK: - Plugin Row

struct PluginRowView: View {
    let plugin: PluginInstance
    let requiredOutputChannels: Int
    let onEdit: () -> Void
    let onRemove: () -> Void
    let onStateChange: (_ inputChannels: Int?, _ bypassed: Bool?) -> Void

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
                    Text("Requires \(requiredOutputChannels) output channel\(requiredOutputChannels == 1 ? "" : "s")")
                        .font(.caption2)
                        .foregroundColor(.secondary)
                }

                Spacer()

                Stepper(
                    value: Binding(
                        get: { plugin.inputChannels },
                        set: { onStateChange($0, nil) }
                    ),
                    in: 1...32
                ) {
                    Text("\(plugin.inputChannels) ch")
                        .font(.caption)
                        .frame(minWidth: 42)
                }
                .help("Set plugin input channel count")

                Button {
                    onStateChange(nil, !plugin.bypassed)
                } label: {
                    Image(systemName: plugin.bypassed ? "power.circle.fill" : "power.circle")
                        .foregroundColor(plugin.bypassed ? .orange : .primary)
                }
                .buttonStyle(.borderless)
                .help(plugin.bypassed ? "Enable plugin" : "Bypass plugin")

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

private enum PluginParameterFileFormat: Int, CaseIterable {
    case parameterJSON
    case enginePluginJSON
    case appGpuiPresetJSON
    case rawParametersJSON

    var title: String {
        switch self {
        case .parameterJSON:
            return "Parameter JSON"
        case .enginePluginJSON:
            return "Engine plugin JSON"
        case .appGpuiPresetJSON:
            return "App GPUI preset JSON"
        case .rawParametersJSON:
            return "Raw parameters JSON"
        }
    }
}

struct PluginEditSheet: View {
    @ObservedObject var plugin: PluginInstance
    let descriptors: [PluginParameterDescriptor]
    let onApply: ([String: Any]) -> Bool
    let onCancel: () -> Void
    let onClose: () -> Void

    @State private var draftParameters: [String: Any]
    @State private var baselineParameters: [String: Any]
    @State private var editorRevision = 0
    @State private var liveApplyRevision = 0
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
        _baselineParameters = State(initialValue: plugin.parameters)
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
                        updateDraftParameters(newParameters)
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
                    .keyboardShortcut(.defaultAction)

                    Button("Revert") {
                        revertDraft()
                    }

                    Button("Cancel") {
                        revertAndClose()
                    }
                    .keyboardShortcut(.cancelAction)

                    Button("Close") {
                        // Edits are applied after the debounce interval. Close
                        // leaves the current live state; Revert or Cancel
                        // restores the parameters captured when editing began.
                        liveApplyRevision &+= 1
                        onClose()
                    }
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
        liveApplyRevision &+= 1
        if onApply(draftParameters) {
            sheetError = nil
            if closeAfterApply {
                onClose()
            }
        } else {
            sheetError = "Failed to apply parameters to the plugin chain"
        }
    }

    private func updateDraftParameters(_ newParameters: [String: Any]) {
        draftParameters = newParameters
        sheetError = nil
        liveApplyRevision &+= 1
        let revision = liveApplyRevision
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.3) {
            guard liveApplyRevision == revision else { return }
            if !onApply(newParameters) {
                sheetError = "Failed to apply parameters to the plugin chain"
            }
        }
    }

    private func revertDraft() {
        liveApplyRevision &+= 1
        guard onApply(baselineParameters) else {
            sheetError = "Failed to revert parameters on the plugin chain"
            return
        }
        draftParameters = baselineParameters
        sheetError = nil
        editorRevision += 1
    }

    private func revertAndClose() {
        liveApplyRevision &+= 1
        guard onApply(baselineParameters) else {
            sheetError = "Failed to revert parameters on the plugin chain"
            return
        }
        draftParameters = baselineParameters
        onCancel()
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

        let formatPicker = NSPopUpButton(frame: NSRect(x: 0, y: 0, width: 220, height: 26), pullsDown: false)
        PluginParameterFileFormat.allCases.forEach { formatPicker.addItem(withTitle: $0.title) }
        formatPicker.selectItem(at: PluginParameterFileFormat.parameterJSON.rawValue)
        panel.accessoryView = formatPicker

        guard panel.runModal() == .OK, let url = panel.url else {
            return
        }

        let selectedFormat = PluginParameterFileFormat(rawValue: formatPicker.indexOfSelectedItem) ?? .parameterJSON
        let savedPlugin = parameterDocument(for: selectedFormat)

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

    private func parameterDocument(for format: PluginParameterFileFormat) -> Any {
        switch format {
        case .parameterJSON:
            return [
                "plugin_type": plugin.pluginType,
                "plugin_name": plugin.pluginName,
                "parameters": draftParameters,
            ]

        case .enginePluginJSON:
            return [
                "plugin_type": plugin.pluginType,
                "parameters": draftParameters,
            ]

        case .appGpuiPresetJSON:
            return [
                "version": 2,
                "plugins": [
                    appGpuiPluginRecord()
                ],
            ]

        case .rawParametersJSON:
            return draftParameters
        }
    }

    private func appGpuiPluginRecord() -> [String: Any] {
        var record: [String: Any] = [
            "id": 0,
            "enabled": true,
            "permanent": false,
            "plugin_type": plugin.pluginType,
            "plugin_name": plugin.pluginName,
            "parameters": draftParameters,
        ]

        if let variant = engineTypeToAppGpuiSettingsVariant[plugin.pluginType] {
            record["settings"] = [variant: draftParameters]
        }

        return record
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
    let isLoading: Bool
    let onAdd: (String, [String: Any]) -> Void
    let onCancel: () -> Void

    @AppStorage("configbar.pluginPicker.search") private var searchText = ""
    @AppStorage("configbar.pluginPicker.selection") private var rememberedPluginID = ""
    @State private var selectedPluginID: String? = nil
    @State private var draftParameters: [String: Any] = [:]
    @FocusState private var focusedPluginID: String?

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
                    if isLoading && categories.isEmpty {
                        HStack(spacing: 8) {
                            ProgressView()
                                .controlSize(.small)
                            Text("Loading plugins…")
                                .foregroundColor(.secondary)
                        }
                    } else {
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
                                    .focused($focusedPluginID, equals: plugin.type_)
                                    .listRowBackground(
                                        selectedPluginID == plugin.type_
                                            ? Color.accentColor.opacity(0.16)
                                            : Color.clear
                                    )
                                    .simultaneousGesture(
                                        TapGesture(count: 2).onEnded {
                                            onAdd(plugin.type_, plugin.defaultParameters)
                                        }
                                    )
                                }
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
        .onChange(of: availablePlugins.count) { _ in
            selectInitialPluginIfNeeded()
        }
        .onMoveCommand { direction in
            movePluginSelection(direction)
        }
    }

    private func selectInitialPluginIfNeeded() {
        guard selectedPluginID == nil else {
            return
        }
        let remembered = visiblePlugins.first { $0.type_ == rememberedPluginID }
        let first = remembered ?? categories.first?.plugins.first
        if let first {
            selectPlugin(first)
        }
    }

    private func selectPlugin(_ plugin: AvailablePlugin) {
        selectedPluginID = plugin.type_
        rememberedPluginID = plugin.type_
        draftParameters = plugin.defaultParameters
    }

    private func movePluginSelection(_ direction: MoveCommandDirection) {
        let options = categories.flatMap(\.plugins)
        guard !options.isEmpty else { return }

        let currentIndex = focusedPluginID.flatMap { id in
            options.firstIndex { $0.type_ == id }
        }
        let nextIndex: Int
        switch direction {
        case .up:
            nextIndex = max((currentIndex ?? options.count) - 1, 0)
        case .down:
            nextIndex = min((currentIndex ?? -1) + 1, options.count - 1)
        default:
            return
        }

        let plugin = options[nextIndex]
        selectPlugin(plugin)
        focusedPluginID = plugin.type_
    }
}
