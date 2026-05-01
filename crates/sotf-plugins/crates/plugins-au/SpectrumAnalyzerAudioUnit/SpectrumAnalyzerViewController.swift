import AppKit
import CoreAudioKit
import AudioToolbox

public class SpectrumAnalyzerViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "SpectrumAnalyzer" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try SpectrumAnalyzerAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
