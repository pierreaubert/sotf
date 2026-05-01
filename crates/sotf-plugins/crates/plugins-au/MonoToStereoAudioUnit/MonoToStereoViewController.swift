import AppKit
import CoreAudioKit
import AudioToolbox

public class MonoToStereoViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "MonoToStereo" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try MonoToStereoAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
