#if os(iOS)
import AudioToolbox
import CoreAudioKit
import UIKit

public final class EQiOSViewController: GenericRustiOSViewController, AUAudioUnitFactory {
    public override class var pluginType: String { "EQ" }

    public nonisolated func createAudioUnit(
        with componentDescription: AudioComponentDescription
    ) throws -> AUAudioUnit {
        let unit = try EQiOSAudioUnit(componentDescription: componentDescription, options: [])
        audioUnit = unit
        return unit
    }
}
#endif
