import AppKit
import CoreAudioKit
import AudioToolbox

public class SpeechDenoiserViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "SpeechDenoiser" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try SpeechDenoiserAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
