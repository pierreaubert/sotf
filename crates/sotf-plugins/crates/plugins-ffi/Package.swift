// swift-tools-version: 6.0

import PackageDescription

let package = Package(
    name: "SOTFPluginFFI",
    platforms: [
        .macOS(.v15),
        .iOS(.v17),
    ],
    products: [
        .library(
            name: "SOTFPluginFFI",
            targets: ["SOTFPluginFFI"]
        ),
    ],
    targets: [
        .target(
            name: "SOTFPluginFFI",
            path: "SwiftPackage/Sources/SOTFPluginFFI",
            publicHeadersPath: "include",
            cSettings: [
                .headerSearchPath("include"),
            ],
            linkerSettings: [
                .linkedLibrary("sotf_audio_plugins_ffi"),
                .linkedLibrary("sqlite3"),
                .linkedFramework("AVFoundation", .when(platforms: [.iOS, .macOS])),
                .linkedFramework("AudioToolbox", .when(platforms: [.iOS, .macOS])),
                .linkedFramework("CoreAudioKit", .when(platforms: [.iOS, .macOS])),
                .linkedFramework("Foundation", .when(platforms: [.iOS, .macOS])),
                .linkedFramework("Metal", .when(platforms: [.iOS, .macOS])),
                .linkedFramework("QuartzCore", .when(platforms: [.iOS, .macOS])),
                .linkedFramework("CoreGraphics", .when(platforms: [.iOS, .macOS])),
                .linkedFramework("CoreText", .when(platforms: [.iOS, .macOS])),
                .linkedFramework("IOKit", .when(platforms: [.macOS])),
                .linkedFramework("IOSurface", .when(platforms: [.macOS])),
            ]
        ),
    ]
)
