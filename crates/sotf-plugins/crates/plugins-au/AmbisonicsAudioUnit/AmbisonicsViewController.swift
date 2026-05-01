import AppKit
import CoreAudioKit
import AudioToolbox

public class AmbisonicsViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "AmbisonicsDecoder" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try AmbisonicsAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
