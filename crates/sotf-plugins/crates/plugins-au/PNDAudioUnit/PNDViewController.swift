import AppKit
import CoreAudioKit
import AudioToolbox

public class PNDViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "PND" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try PNDAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
