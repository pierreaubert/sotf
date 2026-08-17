import Foundation

// MARK: - Plugin Models

/// Parameter metadata for descriptor-driven plugin editors.
struct PluginParameterDescriptor: Identifiable {
    let key: String
    let name: String
    let type: String
    let unit: String
    let group: String
    let doc: String
    let updateMode: String
    let min: Double?
    let max: Double?
    let step: Double?
    let defaultDouble: Double?
    let defaultBool: Bool?
    let choices: [String]?
    let trueLabel: String?
    let falseLabel: String?

    var id: String { key }

    init(
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
struct AvailablePlugin: Identifiable {
    let type_: String
    let name: String
    let description: String
    let category: String
    let maturity: String
    let defaultParameters: [String: Any]
    let parameters: [PluginParameterDescriptor]

    var id: String { type_ }
}

/// A plugin instance in the current chain (from get_plugins)
class PluginInstance: Identifiable, ObservableObject {
    let id: UUID
    let index: Int
    let pluginType: String
    let pluginName: String
    @Published var parameters: [String: Any]

    init(index: Int, pluginType: String, pluginName: String, parameters: [String: Any]) {
        self.id = UUID()
        self.index = index
        self.pluginType = pluginType
        self.pluginName = pluginName
        self.parameters = parameters
    }
}

/// Plugin categories for the picker
struct PluginCategory: Identifiable {
    let name: String
    let plugins: [AvailablePlugin]
    var id: String { name }
}

enum PluginPipelineTopology: String {
    case rack
    case graph
}

struct PluginGraphNodeModel: Identifiable {
    let id: Int
    var pluginType: String
    var parameters: [String: Any]
    var inputChannels: Int
    var bypassed: Bool

    var pluginName: String {
        pluginDisplayName(pluginType)
    }

    var artifact: [String: Any] {
        [
            "id": id,
            "plugin_type": pluginType,
            "parameters": parameters,
            "input_channels": inputChannels,
            "bypassed": bypassed,
        ]
    }
}

struct PluginGraphEdgeModel: Identifiable, Hashable {
    let fromNode: Int
    let toNode: Int

    var id: String { "\(fromNode)->\(toNode)" }

    var artifact: [String: Any] {
        ["from_node": fromNode, "to_node": toNode]
    }

    static func == (lhs: PluginGraphEdgeModel, rhs: PluginGraphEdgeModel) -> Bool {
        lhs.fromNode == rhs.fromNode && lhs.toNode == rhs.toNode
    }

    func hash(into hasher: inout Hasher) {
        hasher.combine(fromNode)
        hasher.combine(toNode)
    }
}

struct PluginGraphModel {
    var nodes: [PluginGraphNodeModel]
    var edges: [PluginGraphEdgeModel]

    var artifact: [String: Any] {
        [
            "nodes": nodes.map(\.artifact),
            "edges": edges.map(\.artifact),
        ]
    }

    var linearNodeIDs: [Int]? {
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

    var isLinear: Bool {
        linearNodeIDs != nil
    }
}

struct PluginPipelineModel {
    var topology: PluginPipelineTopology
    var plugins: [[String: Any]]
    var graph: PluginGraphModel?
    var generation: Int?
}

/// Group available plugins by category
func groupPluginsByCategory(_ plugins: [AvailablePlugin]) -> [PluginCategory] {
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
func pluginDisplayName(_ type: String) -> String {
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

let engineTypeToAppGpuiSettingsVariant: [String: String] = [
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

let appGpuiSettingsVariantToEngineType: [String: String] = Dictionary(
    uniqueKeysWithValues: engineTypeToAppGpuiSettingsVariant.map { ($0.value, $0.key) }
)
