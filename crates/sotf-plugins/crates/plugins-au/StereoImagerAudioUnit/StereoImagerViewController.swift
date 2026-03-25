import AppKit
import CoreAudioKit
import AudioToolbox

public class StereoImagerViewController: AUViewController, AUAudioUnitFactory {
    nonisolated(unsafe) private var audioUnit: StereoImagerAudioUnit?
    nonisolated(unsafe) private var gpuiView: GPUIAUView?

    public nonisolated func createAudioUnit(with componentDescription: AudioComponentDescription) throws -> AUAudioUnit {
        let unit = try StereoImagerAudioUnit(componentDescription: componentDescription, options: [])
        self.audioUnit = unit
        return unit
    }

    public override func viewDidLoad() {
        super.viewDidLoad()
        let gpui = GPUIAUView(pluginType: "StereoImager", audioUnit: audioUnit)
        gpui.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(gpui)
        NSLayoutConstraint.activate([
            gpui.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            gpui.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            gpui.topAnchor.constraint(equalTo: view.topAnchor),
            gpui.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
        gpuiView = gpui
    }

    public override func viewDidLayout() {
        super.viewDidLayout()
        if let au = audioUnit, let gpui = gpuiView { gpui.connectAudioUnit(au) }
    }
}
