import Foundation

// MARK: - Plugin Models

/// Metadata for an available plugin type (from get_available_plugins)
struct AvailablePlugin: Identifiable, Codable {
    let type_: String
    let name: String
    let description: String
    let category: String
    let maturity: String

    var id: String { type_ }

    enum CodingKeys: String, CodingKey {
        case type_ = "type"
        case name
        case description
        case category
        case maturity
    }
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
    default: return type
    }
}
