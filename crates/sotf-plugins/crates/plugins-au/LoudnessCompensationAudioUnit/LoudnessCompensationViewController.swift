import AppKit
import CoreAudioKit
import AudioToolbox

public class LoudnessCompensationViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "LoudnessCompensation" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try LoudnessCompensationAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
