import AppKit
import CoreAudioKit
import AudioToolbox

public class DownmixViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "Downmix" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try DownmixAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
