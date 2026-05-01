import AppKit
import CoreAudioKit
import AudioToolbox

public class GainViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "Gain" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try GainAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
