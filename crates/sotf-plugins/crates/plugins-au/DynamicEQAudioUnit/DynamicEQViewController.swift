import AppKit
import CoreAudioKit
import AudioToolbox

public class DynamicEQViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "DynamicEQ" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try DynamicEQAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
