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
        .library(
            name: "ConfigBarUI",
            targets: ["ConfigBarUI"]
        ),
    ],
    targets: [
        .target(
            name: "ConfigBarModels",
            path: "src",
            exclude: [
                "ConfigBar.swift",
                "AppMain.swift",
                "PluginEditorViews.swift",
                "PluginRackView.swift",
            ],
            sources: ["PluginModels.swift", "ConfigBarIPC.swift", "ConfigBarPure.swift"]
        ),
        .target(
            name: "ConfigBarUI",
            dependencies: ["ConfigBarModels"],
            path: "src",
            exclude: ["AppMain.swift", "PluginModels.swift", "ConfigBarIPC.swift", "ConfigBarPure.swift"],
            sources: ["ConfigBar.swift", "PluginEditorViews.swift", "PluginRackView.swift"]
        ),
        .testTarget(
            name: "ConfigBarModelTests",
            dependencies: ["ConfigBarModels"],
            path: "tests",
            exclude: ["ConfigBarUITests.swift"],
            sources: ["ConfigBarModelTests.swift", "ConfigBarIPCTests.swift"]
        ),
        .testTarget(
            name: "ConfigBarUITests",
            dependencies: ["ConfigBarUI", "ConfigBarModels"],
            path: "tests",
            exclude: ["ConfigBarModelTests.swift", "ConfigBarIPCTests.swift"],
            sources: ["ConfigBarUITests.swift"]
        ),
    ]
)
