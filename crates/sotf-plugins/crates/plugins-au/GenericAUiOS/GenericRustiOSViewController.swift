#if os(iOS)
import AudioToolbox
import CoreAudioKit
import UIKit

open class GenericRustiOSViewController: AUViewController {
    open class var pluginType: String { fatalError("Subclass must override pluginType") }
    open class var defaultSize: CGSize { CGSize(width: 800, height: 500) }

    nonisolated(unsafe) public var audioUnit: GenericRustAudioUnit?
    nonisolated(unsafe) public var pluginView: GenericRustiOSView?

    public override func loadView() {
        view = UIView(frame: CGRect(origin: .zero, size: Self.defaultSize))
    }

    public override func viewDidLoad() {
        super.viewDidLoad()
        preferredContentSize = Self.defaultSize

        let pluginView = GenericRustiOSView(pluginType: Self.pluginType, audioUnit: audioUnit)
        pluginView.translatesAutoresizingMaskIntoConstraints = false
        view.addSubview(pluginView)
        NSLayoutConstraint.activate([
            pluginView.leadingAnchor.constraint(equalTo: view.leadingAnchor),
            pluginView.trailingAnchor.constraint(equalTo: view.trailingAnchor),
            pluginView.topAnchor.constraint(equalTo: view.topAnchor),
            pluginView.bottomAnchor.constraint(equalTo: view.bottomAnchor),
        ])
        self.pluginView = pluginView
    }

    public override func viewDidLayoutSubviews() {
        super.viewDidLayoutSubviews()
        if let audioUnit = audioUnit, let pluginView = pluginView {
            pluginView.connectAudioUnit(audioUnit)
        }
    }
}
#endif
