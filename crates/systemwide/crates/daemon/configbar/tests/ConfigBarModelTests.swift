import XCTest
@testable import ConfigBarModels

final class ConfigBarModelTests: XCTestCase {

    // MARK: - Helpers

    private func availablePlugin(
        type: String,
        name: String = "",
        description: String = "",
        category: String,
        maturity: String = ""
    ) -> AvailablePlugin {
        AvailablePlugin(
            type_: type,
            name: name,
            description: description,
            category: category,
            maturity: maturity,
            defaultParameters: [:],
            parameters: []
        )
    }

    // MARK: - Category grouping

    func testGroupPluginsByCategoryPreservesOrderAndSortsUnknownAlphabetically() {
        let plugins = [
            availablePlugin(type: "delay", category: "Effects"),
            availablePlugin(type: "eq", category: "EQ & Tone"),
            availablePlugin(type: "upmixer", category: "Spatial & Routing"),
            availablePlugin(type: "zebra", category: "Zebra"),
            availablePlugin(type: "compressor", category: "Dynamics"),
            availablePlugin(type: "alpha", category: "Alpha"),
            availablePlugin(type: "denoiser", category: "Restoration"),
        ]

        let grouped = groupPluginsByCategory(plugins)
        let names = grouped.map { $0.name }

        XCTAssertEqual(names, [
            "EQ & Tone",
            "Dynamics",
            "Spatial & Routing",
            "Effects",
            "Restoration",
            "Alpha",
            "Zebra",
        ])

        XCTAssertEqual(grouped.first { $0.name == "EQ & Tone" }?.plugins.map(\.type_), ["eq"])
        XCTAssertEqual(grouped.first { $0.name == "Effects" }?.plugins.map(\.type_), ["delay"])
        XCTAssertEqual(grouped.first { $0.name == "Alpha" }?.plugins.map(\.type_), ["alpha"])
        XCTAssertEqual(grouped.first { $0.name == "Zebra" }?.plugins.map(\.type_), ["zebra"])
    }

    func testGroupPluginsByCategoryReturnsEmptyForEmptyInput() {
        XCTAssertTrue(groupPluginsByCategory([]).isEmpty)
    }

    // MARK: - Display names

    func testPluginDisplayNameKnownTypes() {
        XCTAssertEqual(pluginDisplayName("eq"), "EQ")
        XCTAssertEqual(pluginDisplayName("compressor"), "Compressor")
        XCTAssertEqual(pluginDisplayName("upmixer"), "Upmixer")
        XCTAssertEqual(pluginDisplayName("multiband_compressor"), "Multiband Compressor")
        XCTAssertEqual(pluginDisplayName("loudness_compensation"), "Loudness Compensation")
        XCTAssertEqual(pluginDisplayName("ab_compare"), "A/B Compare")
    }

    func testPluginDisplayNameUnknownTypeReturnsInput() {
        let unknown = "unknown_fancy_plugin"
        XCTAssertEqual(pluginDisplayName(unknown), unknown)
    }

    // MARK: - PluginParameterDescriptor defaults

    func testParameterDescriptorDefaults() {
        let descriptor = PluginParameterDescriptor(
            key: "gain",
            name: "Gain",
            type: "double"
        )

        XCTAssertEqual(descriptor.unit, "")
        XCTAssertEqual(descriptor.group, "General")
        XCTAssertEqual(descriptor.updateMode, "realtime")
        XCTAssertNil(descriptor.min)
        XCTAssertNil(descriptor.max)
        XCTAssertNil(descriptor.step)
        XCTAssertNil(descriptor.defaultDouble)
        XCTAssertNil(descriptor.defaultBool)
        XCTAssertNil(descriptor.choices)
        XCTAssertNil(descriptor.trueLabel)
        XCTAssertNil(descriptor.falseLabel)
    }

    // MARK: - PluginParameterDescriptor round-trip

    func testParameterDescriptorCustomValuesRoundTrip() {
        let descriptor = PluginParameterDescriptor(
            key: "freq",
            name: "Frequency",
            type: "double",
            unit: "Hz",
            group: "Filter",
            doc: "Cutoff frequency",
            updateMode: "defer",
            min: 20.0,
            max: 20000.0,
            step: 1.0,
            defaultDouble: 1000.0,
            defaultBool: nil,
            choices: ["A", "B"],
            trueLabel: "On",
            falseLabel: "Off"
        )

        XCTAssertEqual(descriptor.key, "freq")
        XCTAssertEqual(descriptor.name, "Frequency")
        XCTAssertEqual(descriptor.type, "double")
        XCTAssertEqual(descriptor.unit, "Hz")
        XCTAssertEqual(descriptor.group, "Filter")
        XCTAssertEqual(descriptor.doc, "Cutoff frequency")
        XCTAssertEqual(descriptor.updateMode, "defer")
        XCTAssertEqual(descriptor.min, 20.0)
        XCTAssertEqual(descriptor.max, 20000.0)
        XCTAssertEqual(descriptor.step, 1.0)
        XCTAssertEqual(descriptor.defaultDouble, 1000.0)
        XCTAssertNil(descriptor.defaultBool)
        XCTAssertEqual(descriptor.choices, ["A", "B"])
        XCTAssertEqual(descriptor.trueLabel, "On")
        XCTAssertEqual(descriptor.falseLabel, "Off")
    }

    // MARK: - Identifiable conformance

    func testAvailablePluginIdEqualsType() {
        let plugin = availablePlugin(type: "eq", category: "EQ & Tone")
        XCTAssertEqual(plugin.id, plugin.type_)
        XCTAssertEqual(plugin.id, "eq")
    }

    func testPluginCategoryIdEqualsName() {
        let category = PluginCategory(
            name: "Dynamics",
            plugins: [availablePlugin(type: "compressor", category: "Dynamics")]
        )
        XCTAssertEqual(category.id, category.name)
        XCTAssertEqual(category.id, "Dynamics")
    }
}
