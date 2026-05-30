#if os(iOS)
import AudioToolbox
import CoreAudioKit
import UIKit

public final class GenericRustiOSView: UIView {
    private weak var audioUnit: GenericRustAudioUnit?
    private let pluginType: String
    private let titleLabel = UILabel()
    private let statusLabel = UILabel()

    public init(pluginType: String, audioUnit: GenericRustAudioUnit?) {
        self.pluginType = pluginType
        self.audioUnit = audioUnit
        super.init(frame: .zero)
        configure()
    }

    @available(*, unavailable)
    required init?(coder: NSCoder) {
        fatalError("init(coder:) is not supported")
    }

    public func connectAudioUnit(_ audioUnit: GenericRustAudioUnit) {
        self.audioUnit = audioUnit
        updateStatus()
    }

    private func configure() {
        backgroundColor = UIColor(red: 0.08, green: 0.08, blue: 0.10, alpha: 1.0)

        titleLabel.translatesAutoresizingMaskIntoConstraints = false
        titleLabel.text = "SOTF: \(pluginType)"
        titleLabel.textColor = .white
        titleLabel.font = .preferredFont(forTextStyle: .headline)
        titleLabel.textAlignment = .center

        statusLabel.translatesAutoresizingMaskIntoConstraints = false
        statusLabel.textColor = UIColor(white: 0.78, alpha: 1.0)
        statusLabel.font = .preferredFont(forTextStyle: .subheadline)
        statusLabel.numberOfLines = 0
        statusLabel.textAlignment = .center

        addSubview(titleLabel)
        addSubview(statusLabel)
        NSLayoutConstraint.activate([
            titleLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 16),
            titleLabel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -16),
            titleLabel.centerYAnchor.constraint(equalTo: centerYAnchor, constant: -20),

            statusLabel.leadingAnchor.constraint(equalTo: leadingAnchor, constant: 24),
            statusLabel.trailingAnchor.constraint(equalTo: trailingAnchor, constant: -24),
            statusLabel.topAnchor.constraint(equalTo: titleLabel.bottomAnchor, constant: 12),
        ])
        updateStatus()
    }

    private func updateStatus() {
        if let audioUnit = audioUnit, audioUnit.parameterTree != nil {
            statusLabel.text = "Connected"
        } else {
            statusLabel.text = "Waiting"
        }
    }
}
#endif
