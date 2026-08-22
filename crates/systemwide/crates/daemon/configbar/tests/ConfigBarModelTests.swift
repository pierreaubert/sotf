import XCTest
@testable import ConfigBarModels

final class ConfigBarModelTests: XCTestCase {
    func testMenuBarPillConventionSeparatesPlayingRecordingAndIdle() {
        XCTAssertEqual(
            ConfigBarMenuBarPillAppearance.resolve(
                isPlaying: true,
                isRecording: false,
                hasIssue: false
            ),
            .playing
        )
        XCTAssertEqual(
            ConfigBarMenuBarPillAppearance.resolve(
                isPlaying: false,
                isRecording: true,
                hasIssue: false
            ),
            .recording
        )
        XCTAssertEqual(
            ConfigBarMenuBarPillAppearance.resolve(
                isPlaying: false,
                isRecording: false,
                hasIssue: false
            ),
            .transparent
        )
        XCTAssertEqual(
            ConfigBarMenuBarPillAppearance.resolve(
                isPlaying: true,
                isRecording: false,
                hasIssue: true
            ),
            .transparent
        )
    }


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

    // MARK: - Graph topology

    private func graphNode(_ id: Int) -> PluginGraphNodeModel {
        PluginGraphNodeModel(
            id: id,
            pluginType: "gain",
            parameters: [:],
            inputChannels: 2,
            bypassed: false
        )
    }

    private func graph(_ nodeIDs: [Int], edges: [(Int, Int)]) -> PluginGraphModel {
        PluginGraphModel(
            nodes: nodeIDs.map(graphNode),
            edges: edges.map { PluginGraphEdgeModel(fromNode: $0.0, toNode: $0.1) }
        )
    }

    func testGraphTopologyAcceptsLinearChainAndReturnsOrder() {
        let model = graph([10, 20, 30], edges: [(10, 20), (20, 30)])

        XCTAssertEqual(model.linearNodeIDs, [10, 20, 30])
        XCTAssertTrue(model.isLinear)
    }

    func testGraphTopologyRejectsCycle() {
        let model = graph([1, 2, 3], edges: [(1, 2), (2, 3), (3, 1)])

        XCTAssertNil(model.linearNodeIDs)
        XCTAssertFalse(model.isLinear)
    }

    func testGraphTopologyRejectsMultipleRoots() {
        let model = graph([1, 2, 3], edges: [(1, 3)])

        XCTAssertNil(model.linearNodeIDs)
        XCTAssertFalse(model.isLinear)
    }

    func testGraphTopologyRejectsMalformedEdges() {
        let unknownNode = graph([1, 2], edges: [(1, 99)])
        let duplicateNodeIDs = PluginGraphModel(
            nodes: [graphNode(1), graphNode(1)],
            edges: [PluginGraphEdgeModel(fromNode: 1, toNode: 1)]
        )

        XCTAssertNil(unknownNode.linearNodeIDs)
        XCTAssertNil(duplicateNodeIDs.linearNodeIDs)
    }

    // MARK: - Configbar pure behavior

    func testVirtualDeviceDetectionRejectsKnownLoopbackDevicesOnly() {
        XCTAssertTrue(isConfigBarVirtualDevice("SotF Virtual Audio"))
        XCTAssertTrue(isConfigBarVirtualDevice("BLACKHOLE 2ch"))
        XCTAssertFalse(isConfigBarVirtualDevice("Built-in Output"))
    }

    func testMeterPeakSanitizationClampsInvalidAndOversizedValues() {
        let values = sanitizeConfigBarPeaks([.nan, -1.0, 0.0, 0.75, 4.0])

        XCTAssertEqual(values, [0.0, 0.0, 0.0, 0.75, 2.0])
    }

    func testMeterPeakHoldsDecayWithoutDroppingBelowCurrentPeak() {
        let values = updateConfigBarPeakHolds(
            previous: [1.0, 0.25],
            current: [0.5, 0.5]
        )

        XCTAssertEqual(values[0], 0.96, accuracy: 0.0001)
        XCTAssertEqual(values[1], 0.5, accuracy: 0.0001)
        XCTAssertEqual(decayConfigBarPeaks([1.0]), [0.85])
    }

    func testEncryptionToggleGuardConsumesOnlyTheProgrammaticRollback() {
        var guardState = EncryptionToggleGuard()
        var daemonRequests = 0

        XCTAssertFalse(guardState.consumeProgrammaticChange())
        daemonRequests += 1 // the user's original failed request

        guardState.markProgrammaticChange()
        XCTAssertTrue(guardState.consumeProgrammaticChange())
        XCTAssertFalse(guardState.consumeProgrammaticChange())
        XCTAssertEqual(daemonRequests, 1)
    }

    func testRejectedOptimisticMutationsRollbackWithoutRetrying() {
        var device = ConfigBarMutationState(confirmed: "Built-in Output")
        var volume = ConfigBarMutationState(confirmed: Float(0.75))
        var channels = ConfigBarMutationState(confirmed: 2)
        var daemonRequests = 0

        let deviceGeneration = device.begin("USB DAC")
        daemonRequests += 1
        XCTAssertEqual(
            device.resolve(
                generation: deviceGeneration,
                requested: "USB DAC",
                succeeded: false
            ),
            .rolledBack("Built-in Output")
        )

        let volumeGeneration = volume.begin(0.25)
        daemonRequests += 1
        XCTAssertEqual(
            volume.resolve(
                generation: volumeGeneration,
                requested: 0.25,
                succeeded: false
            ),
            .rolledBack(0.75)
        )

        let channelGeneration = channels.begin(8)
        daemonRequests += 1
        XCTAssertEqual(
            channels.resolve(
                generation: channelGeneration,
                requested: 8,
                succeeded: false
            ),
            .rolledBack(2)
        )

        // Rollback is a state assignment only; it must not become a second
        // daemon request through a Picker or Slider callback.
        XCTAssertEqual(daemonRequests, 3)
        XCTAssertEqual(device.confirmed, "Built-in Output")
        XCTAssertEqual(volume.confirmed, 0.75, accuracy: 0.0001)
        XCTAssertEqual(channels.confirmed, 2)
    }

    func testStaleMutationCompletionCannotOverwriteNewerConfirmedValue() {
        var state = ConfigBarMutationState(confirmed: 2)
        let firstGeneration = state.begin(4)
        let secondGeneration = state.begin(8)

        XCTAssertNil(
            state.resolve(
                generation: firstGeneration,
                requested: 4,
                succeeded: false
            )
        )
        XCTAssertEqual(
            state.resolve(
                generation: secondGeneration,
                requested: 8,
                succeeded: true
            ),
            .confirmed(8)
        )
        XCTAssertEqual(state.confirmed, 8)
    }

    func testStatusWatermarkRejectsSnapshotStartedBeforeMutation() {
        var watermark = ConfigBarStatusWatermark()
        let firstSnapshot = watermark.generation

        XCTAssertTrue(watermark.accepts(snapshotGeneration: firstSnapshot))
        XCTAssertEqual(watermark.beginMutation(), 1)
        XCTAssertFalse(watermark.accepts(snapshotGeneration: firstSnapshot))

        let secondSnapshot = watermark.generation
        XCTAssertTrue(watermark.accepts(snapshotGeneration: secondSnapshot))
    }

    func testRejectedEncryptionMutationRollsBackOnceAndConsumesGuard() {
        var state = ConfigBarMutationState(confirmed: true)
        var toggleGuard = EncryptionToggleGuard()
        var daemonRequests = 0

        let generation = state.begin(false)
        daemonRequests += 1
        XCTAssertEqual(
            state.resolve(generation: generation, requested: false, succeeded: false),
            .rolledBack(true)
        )

        toggleGuard.markProgrammaticChange()
        XCTAssertTrue(toggleGuard.consumeProgrammaticChange())
        XCTAssertFalse(toggleGuard.consumeProgrammaticChange())
        XCTAssertEqual(daemonRequests, 1)
        XCTAssertTrue(state.confirmed)
    }

    // MARK: - Daemon watchdog policy

    func testWatchdogToleratesTransientProbeFailureButRestartsOnRepeatedOnes() {
        XCTAssertFalse(ConfigBarDaemonWatchdog.shouldRestart(consecutiveFailures: 0))
        XCTAssertFalse(ConfigBarDaemonWatchdog.shouldRestart(consecutiveFailures: 1))
        XCTAssertTrue(ConfigBarDaemonWatchdog.shouldRestart(
            consecutiveFailures: ConfigBarDaemonWatchdog.restartThreshold
        ))
        XCTAssertTrue(ConfigBarDaemonWatchdog.shouldRestart(consecutiveFailures: 5))
    }
}
