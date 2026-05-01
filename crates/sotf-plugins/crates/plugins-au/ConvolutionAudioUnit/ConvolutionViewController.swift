import AppKit
import CoreAudioKit
import AudioToolbox

public class ConvolutionViewController: GenericRustViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "Convolution" }

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try ConvolutionAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }
}
