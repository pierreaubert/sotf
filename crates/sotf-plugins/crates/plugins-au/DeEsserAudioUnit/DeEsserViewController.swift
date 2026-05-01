import AppKit
import CoreAudioKit
import AudioToolbox

public class DeEsserViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "DeEsser" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try DeEsserAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
