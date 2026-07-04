import MediaPlayer
@testable import SotFPlayer
import XCTest

final class PlatformSmokeTests: XCTestCase {
    func testDynamicTypeScaleIsMonotonic() {
        let categories: [UIContentSizeCategory] = [
            .extraSmall,
            .small,
            .medium,
            .large,
            .extraLarge,
            .extraExtraLarge,
            .extraExtraExtraLarge,
            .accessibilityMedium,
            .accessibilityLarge,
            .accessibilityExtraLarge,
            .accessibilityExtraExtraLarge,
            .accessibilityExtraExtraExtraLarge,
        ]

        let scales = categories.map { IOSPlatformSupport.dynamicTypeScale(for: $0) }
        XCTAssertEqual(IOSPlatformSupport.dynamicTypeScale(for: .large), 1.0)
        XCTAssertTrue(zip(scales, scales.dropFirst()).allSatisfy { $0 <= $1 })
    }

    func testSafeAreaInsetsAreForwarded() {
        let insets = IOSPlatformSupport.safeAreaInsets(
            from: UIEdgeInsets(top: 44, left: 8, bottom: 34, right: 12)
        )

        XCTAssertEqual(insets, IOSPlatformInsets(top: 44, left: 8, bottom: 34, right: 12))
    }

    func testRoutePickerUsesHiddenMpVolumeView() {
        let routePicker = IOSPlatformSupport.makeHiddenRoutePickerView()

        XCTAssertFalse(routePicker.showsVolumeSlider)
        XCTAssertEqual(routePicker.frame, CGRect(x: -1000, y: -1000, width: 44, height: 44))
    }
}
