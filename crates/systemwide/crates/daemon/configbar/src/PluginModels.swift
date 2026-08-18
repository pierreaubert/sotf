import Foundation

// MARK: - Plugin Models

/// Parameter metadata for descriptor-driven plugin editors.
public struct PluginParameterDescriptor: Identifiable {
    public let key: String
    public let name: String
    public let type: String
    public let unit: String
    public let group: String
    public let doc: String
    public let updateMode: String
    public let min: Double?
    public let max: Double?
    public let step: Double?
    public let defaultDouble: Double?
    public let defaultBool: Bool?
    public let choices: [String]?
    public let trueLabel: String?
    public let falseLabel: String?

    public var id: String { key }

    public init(
        key: String,
        name: String,
        type: String,
        unit: String = "",
        group: String = "General",
        doc: String = "",
        updateMode: String = "realtime",
        min: Double? = nil,
        max: Double? = nil,
        step: Double? = nil,
        defaultDouble: Double? = nil,
        defaultBool: Bool? = nil,
        choices: [String]? = nil,
        trueLabel: String? = nil,
        falseLabel: String? = nil
    ) {
        self.key = key
        self.name = name
        self.type = type
        self.unit = unit
        self.group = group
        self.doc = doc
        self.updateMode = updateMode
        self.min = min
        self.max = max
        self.step = step
        self.defaultDouble = defaultDouble
        self.defaultBool = defaultBool
        self.choices = choices
        self.trueLabel = trueLabel
        self.falseLabel = falseLabel
    }

}

/// Metadata for an available plugin type (from get_available_plugins)
public struct AvailablePlugin: Identifiable {
    public let type_: String
    public let name: String
    public let description: String
    public let category: String
    public let maturity: String
    public let defaultParameters: [String: Any]
    public let parameters: [PluginParameterDescriptor]

    public init(
        type_: String,
        name: String,
        description: String,
        category: String,
        maturity: String,
        defaultParameters: [String: Any],
        parameters: [PluginParameterDescriptor]
    ) {
        self.type_ = type_
        self.name = name
        self.description = description
        self.category = category
        self.maturity = maturity
        self.defaultParameters = defaultParameters
        self.parameters = parameters
    }

    public var id: String { type_ }
}

/// A plugin instance in the current chain (from get_plugins)
public class PluginInstance: Identifiable, ObservableObject {
    public let id: UUID
    public let index: Int
    public let pluginType: String
    public let pluginName: String
    @Published public var parameters: [String: Any]
    @Published public var inputChannels: Int
    @Published public var bypassed: Bool

    public init(
        index: Int,
        pluginType: String,
        pluginName: String,
        parameters: [String: Any],
        inputChannels: Int = 1,
        bypassed: Bool = false
    ) {
        self.id = UUID()
        self.index = index
        self.pluginType = pluginType
        self.pluginName = pluginName
        self.parameters = parameters
        self.inputChannels = max(1, inputChannels)
        self.bypassed = bypassed
    }
}

/// Plugin categories for the picker
public struct PluginCategory: Identifiable {
    public let name: String
    public let plugins: [AvailablePlugin]

    public init(name: String, plugins: [AvailablePlugin]) {
        self.name = name
        self.plugins = plugins
    }

    public var id: String { name }
}

public enum PluginPipelineTopology: String {
    case rack
    case graph
}

public struct PluginGraphNodeModel: Identifiable {
    public let id: Int
    public var pluginType: String
    public var parameters: [String: Any]
    public var inputChannels: Int
    public var bypassed: Bool

    public init(
        id: Int,
        pluginType: String,
        parameters: [String: Any],
        inputChannels: Int,
        bypassed: Bool
    ) {
        self.id = id
        self.pluginType = pluginType
        self.parameters = parameters
        self.inputChannels = inputChannels
        self.bypassed = bypassed
    }

    public var pluginName: String {
        pluginDisplayName(pluginType)
    }

    public var artifact: [String: Any] {
        [
            "id": id,
            "plugin_type": pluginType,
            "parameters": parameters,
            "input_channels": inputChannels,
            "bypassed": bypassed,
        ]
    }
}

public struct PluginGraphEdgeModel: Identifiable, Hashable {
    public let fromNode: Int
    public let toNode: Int

    public init(fromNode: Int, toNode: Int) {
        self.fromNode = fromNode
        self.toNode = toNode
    }

    public var id: String { "\(fromNode)->\(toNode)" }

    public var artifact: [String: Any] {
        ["from_node": fromNode, "to_node": toNode]
    }

    public static func == (lhs: PluginGraphEdgeModel, rhs: PluginGraphEdgeModel) -> Bool {
        lhs.fromNode == rhs.fromNode && lhs.toNode == rhs.toNode
    }

    public func hash(into hasher: inout Hasher) {
        hasher.combine(fromNode)
        hasher.combine(toNode)
    }
}

public struct PluginGraphModel {
    public var nodes: [PluginGraphNodeModel]
    public var edges: [PluginGraphEdgeModel]

    public init(nodes: [PluginGraphNodeModel], edges: [PluginGraphEdgeModel]) {
        self.nodes = nodes
        self.edges = edges
    }

    public var artifact: [String: Any] {
        [
            "nodes": nodes.map(\.artifact),
            "edges": edges.map(\.artifact),
        ]
    }

    public var linearNodeIDs: [Int]? {
        guard !nodes.isEmpty else { return [] }
        let nodeIDs = Set(nodes.map(\.id))
        guard nodeIDs.count == nodes.count,
              edges.count == nodes.count - 1,
              edges.allSatisfy({ nodeIDs.contains($0.fromNode) && nodeIDs.contains($0.toNode) })
        else {
            return nil
        }

        var incoming: [Int: Int] = Dictionary(uniqueKeysWithValues: nodes.map { ($0.id, 0) })
        var outgoing: [Int: [Int]] = Dictionary(uniqueKeysWithValues: nodes.map { ($0.id, []) })
        for edge in edges {
            incoming[edge.toNode, default: 0] += 1
            outgoing[edge.fromNode, default: []].append(edge.toNode)
        }
        guard incoming.values.allSatisfy({ $0 <= 1 }),
              outgoing.values.allSatisfy({ $0.count <= 1 }),
              let root = incoming.first(where: { $0.value == 0 })?.key,
              incoming.values.filter({ $0 == 0 }).count == 1
        else {
            return nil
        }

        var ordered: [Int] = []
        var current: Int? = root
        var visited = Set<Int>()
        while let id = current, visited.insert(id).inserted {
            ordered.append(id)
            current = outgoing[id]?.first
        }
        return ordered.count == nodes.count ? ordered : nil
    }

    public var isLinear: Bool {
        linearNodeIDs != nil
    }
}

public struct PluginPipelineModel {
    public var topology: PluginPipelineTopology
    public var plugins: [[String: Any]]
    public var graph: PluginGraphModel?
    public var generation: Int?

    public init(
        topology: PluginPipelineTopology,
        plugins: [[String: Any]],
        graph: PluginGraphModel?,
        generation: Int?
    ) {
        self.topology = topology
        self.plugins = plugins
        self.graph = graph
        self.generation = generation
    }
}

/// Group available plugins by category
public func groupPluginsByCategory(_ plugins: [AvailablePlugin]) -> [PluginCategory] {
    var categoryMap: [String: [AvailablePlugin]] = [:]
    for plugin in plugins {
        categoryMap[plugin.category, default: []].append(plugin)
    }

    let order = ["EQ & Tone", "Dynamics", "Spatial & Routing", "Effects", "Restoration", "Utility"]
    var result: [PluginCategory] = []
    for name in order {
        if let plugins = categoryMap.removeValue(forKey: name) {
            result.append(PluginCategory(name: name, plugins: plugins))
        }
    }
    // Append any remaining categories
    for (name, plugins) in categoryMap.sorted(by: { $0.key < $1.key }) {
        result.append(PluginCategory(name: name, plugins: plugins))
    }
    return result
}

/// Display name for a plugin type string
public func pluginDisplayName(_ type: String) -> String {
    switch type {
    case "eq": return "EQ"
    case "gain": return "Gain"
    case "compressor": return "Compressor"
    case "limiter": return "Limiter"
    case "gate": return "Gate"
    case "expander": return "Expander"
    case "upmixer": return "Upmixer"
    case "multiband_compressor": return "Multiband Compressor"
    case "multiband_expander": return "Multiband Expander"
    case "loudness_compensation": return "Loudness Compensation"
    case "fletcher_munson": return "Fletcher-Munson"
    case "binaural_decoder": return "Binaural Decoder"
    case "convolution": return "Convolution"
    case "matrix": return "Matrix Mixer"
    case "channel_mute_solo": return "Channel Mute/Solo"
    case "xtc": return "Crosstalk Cancellation"
    case "denoiser": return "Denoiser"
    case "pnd": return "PND Varispeed"
    case "delay": return "Delay"
    case "downmix": return "Downmix"
    case "mono_to_stereo": return "Mono to Stereo"
    case "de_esser": return "De-Esser"
    case "declick": return "Declick"
    case "hiss_reducer": return "Hiss Reducer"
    case "speech_denoiser": return "Speech Denoiser"
    case "stereo_imager": return "Stereo Imager"
    case "transient_shaper": return "Transient Shaper"
    case "saturation": return "Saturation"
    case "dynamic_eq": return "Dynamic EQ"
    case "linear_phase_eq": return "Linear Phase EQ"
    case "spectral_compressor": return "Spectral Compressor"
    case "aae": return "AAE"
    case "aec": return "AEC"
    case "beamformer": return "Beamformer"
    case "ambisonics_decoder": return "Ambisonics Decoder"
    case "ab_compare": return "A/B Compare"
    default: return type
    }
}

public let engineTypeToAppGpuiSettingsVariant: [String: String] = [
    "eq": "EQ",
    "gain": "Gain",
    "upmixer": "Upmixer",
    "compressor": "Compressor",
    "limiter": "Limiter",
    "gate": "Gate",
    "expander": "Expander",
    "multiband_compressor": "MultibandCompressor",
    "multiband_expander": "MultibandExpander",
    "loudness_compensation": "LoudnessCompensation",
    "fletcher_munson": "FletcherMunson",
    "binaural_decoder": "BinauralDecoder",
    "convolution": "Convolution",
    "loudness_monitor": "LoudnessMonitor",
    "spectrum_analyzer": "SpectrumAnalyzer",
    "channel_mute_solo": "ChannelMuteSolo",
    "matrix": "Matrix",
    "xtc": "XTC",
    "denoiser": "Denoiser",
    "declick": "Declick",
    "hiss_reducer": "HissReducer",
    "speech_denoiser": "SpeechDenoiser",
    "pnd": "Pnd",
    "ab_compare": "ABCompare",
    "band_split": "BandSplit",
    "band_merge": "BandMerge",
    "downmix": "Downmix",
    "mono_to_stereo": "MonoToStereo",
    "crossfeed": "Crossfeed",
    "delay": "Delay",
    "aec": "Aec",
    "beamformer": "Beamformer",
    "ambisonics_decoder": "AmbisonicsDecoder",
    "stereo_imager": "StereoImager",
    "de_esser": "DeEsser",
    "transient_shaper": "TransientShaper",
    "saturation": "Saturation",
    "dynamic_eq": "DynamicEq",
    "linear_phase_eq": "LinearPhaseEq",
    "spectral_compressor": "SpectralCompressor",
    "aae": "AAE",
]

public let appGpuiSettingsVariantToEngineType: [String: String] = Dictionary(
    uniqueKeysWithValues: engineTypeToAppGpuiSettingsVariant.map { ($0.value, $0.key) }
)
