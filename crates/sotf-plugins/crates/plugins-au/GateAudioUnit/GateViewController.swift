import AppKit
import CoreAudioKit
import AudioToolbox

public class GateViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "Gate" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try GateAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
