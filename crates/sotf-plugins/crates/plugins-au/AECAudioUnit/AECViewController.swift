import AppKit
import CoreAudioKit
import AudioToolbox

public class AECViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "AEC" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try AECAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
