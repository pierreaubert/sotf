import AppKit
import CoreAudioKit
import AudioToolbox

public class UpmixerViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "Upmixer" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try UpmixerAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
