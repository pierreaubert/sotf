// swift-tools-version:5.9
import PackageDescription

let package = Package(
    name: "ConfigBar",
    platforms: [
        .macOS(.v13),
    ],
    products: [
        .library(
            name: "ConfigBarModels",
            targets: ["ConfigBarModels"]
        ),
    ],
    targets: [
        .target(
            name: "ConfigBarModels",
            path: "src",
            exclude: [
                "ConfigBar.swift",
                "PluginEditorViews.swift",
                "PluginRackView.swift",
            ],
            sources: ["PluginModels.swift", "ConfigBarIPC.swift", "ConfigBarPure.swift"]
        ),
        .testTarget(
            name: "ConfigBarModelTests",
            dependencies: ["ConfigBarModels"],
            path: "tests",
            sources: ["ConfigBarModelTests.swift", "ConfigBarIPCTests.swift"]
        ),
    ]
)
