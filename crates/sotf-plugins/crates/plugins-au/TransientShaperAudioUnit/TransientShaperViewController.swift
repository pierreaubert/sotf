import AppKit
import CoreAudioKit
import AudioToolbox

public class TransientShaperViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "TransientShaper" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try TransientShaperAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
