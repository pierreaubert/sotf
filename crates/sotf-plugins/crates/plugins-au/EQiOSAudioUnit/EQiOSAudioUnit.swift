#if os(iOS)
import AVFoundation

public final class EQiOSAudioUnit: GenericRustAudioUnit {
    override public class var pluginType: String { "EQ" }
    override public class var pluginSubtype: String { "SOEQ" }
    override public class var pluginName: String { "SOTF: Parametric EQ" }
}
#endif
