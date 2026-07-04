import MediaPlayer
import UIKit

struct IOSPlatformInsets: Equatable {
    let top: Double
    let left: Double
    let bottom: Double
    let right: Double
}

enum IOSPlatformSupport {
    static func dynamicTypeScale(for category: UIContentSizeCategory) -> Double {
        switch category {
        case .extraSmall: return 0.85
        case .small: return 0.92
        case .medium: return 0.96
        case .large: return 1.0
        case .extraLarge: return 1.08
        case .extraExtraLarge: return 1.16
        case .extraExtraExtraLarge: return 1.24
        case .accessibilityMedium: return 1.34
        case .accessibilityLarge: return 1.48
        case .accessibilityExtraLarge: return 1.62
        case .accessibilityExtraExtraLarge: return 1.78
        case .accessibilityExtraExtraExtraLarge: return 1.95
        default: return 1.0
        }
    }

    static func safeAreaInsets(from insets: UIEdgeInsets) -> IOSPlatformInsets {
        IOSPlatformInsets(
            top: Double(insets.top),
            left: Double(insets.left),
            bottom: Double(insets.bottom),
            right: Double(insets.right)
        )
    }

    static func makeHiddenRoutePickerView() -> MPVolumeView {
        let routePicker = MPVolumeView(frame: CGRect(x: -1000, y: -1000, width: 44, height: 44))
        routePicker.showsVolumeSlider = false
        return routePicker
    }
}
