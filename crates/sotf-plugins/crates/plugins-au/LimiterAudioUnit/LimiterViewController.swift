import AppKit
import CoreAudioKit
import AudioToolbox

public class LimiterViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "Limiter" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try LimiterAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
