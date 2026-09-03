import Darwin
import Foundation
import XCTest
@testable import ConfigBarModels

final class ConfigBarIPCTests: XCTestCase {
    func testLiveDaemonProbeAndAdoptionUseTheConfiguredSocket() throws {
        // Keep the path below sockaddr_un's 104-byte macOS limit. The test
        // process ID makes collisions with a previous test invocation very
        // unlikely, and the stale entry is removed before binding.
        let path = "/tmp/sotf-configbar-\(getpid()).sock"
        unlink(path)
        defer { unlink(path) }
        var serverAddress = sockaddr_un()
        serverAddress.sun_family = sa_family_t(AF_UNIX)
        let pathBytes = path.utf8
        XCTAssertLessThan(pathBytes.count + 1, MemoryLayout.size(ofValue: serverAddress.sun_path))
        let copiedLength = withUnsafeMutableBytes(of: &serverAddress.sun_path) { rawBuffer -> Int in
            path.withCString { pathCString in
                Int(strlcpy(
                    rawBuffer.baseAddress!.assumingMemoryBound(to: CChar.self),
                    pathCString,
                    rawBuffer.count
                ))
            }
        }
        XCTAssertEqual(copiedLength, pathBytes.count)

        let serverFD = Darwin.socket(AF_UNIX, SOCK_STREAM, 0)
        XCTAssertGreaterThanOrEqual(serverFD, 0)
        defer { Darwin.close(serverFD) }
        let bindResult = withUnsafePointer(to: &serverAddress) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) { sockaddrPointer in
                Darwin.bind(
                    serverFD,
                    sockaddrPointer,
                    socklen_t(MemoryLayout<sockaddr_un>.size)
                )
            }
        }
        XCTAssertEqual(bindResult, 0)
        XCTAssertEqual(Darwin.listen(serverFD, 1), 0)
        let flags = Darwin.fcntl(serverFD, F_GETFL, 0)
        _ = Darwin.fcntl(serverFD, F_SETFL, flags | O_NONBLOCK)

        let server = DispatchWorkItem {
            var clientFD: Int32 = -1
            let deadline = Date().addingTimeInterval(1.0)
            while clientFD < 0 && Date() < deadline {
                clientFD = Darwin.accept(serverFD, nil, nil)
                if clientFD < 0 {
                    usleep(1_000)
                }
            }
            guard clientFD >= 0 else { return }
            defer { Darwin.close(clientFD) }
            var request = [UInt8](repeating: 0, count: 128)
            let received = request.withUnsafeMutableBytes { buffer in
                Darwin.recv(clientFD, buffer.baseAddress, buffer.count, 0)
            }
            guard received > 0,
                  String(data: Data(request.prefix(received)), encoding: .utf8)
                    == "{\"command\":\"ping\"}\n"
            else { return }
            let response = Data("{\"success\":true,\"data\":{},\"error\":null}\n".utf8)
            _ = response.withUnsafeBytes { buffer in
                Darwin.send(clientFD, buffer.baseAddress, buffer.count, 0)
            }
        }
        DispatchQueue.global().async(execute: server)

        XCTAssertTrue(ConfigBarIPC.probeDaemon(socketPath: path, timeoutMilliseconds: 500))
        XCTAssertTrue(ConfigBarDaemonAdoption.shouldAdopt(
            reachable: true,
            managedProcessRunning: false
        ))
        XCTAssertFalse(ConfigBarDaemonAdoption.shouldAdopt(
            reachable: true,
            managedProcessRunning: true
        ))
        XCTAssertEqual(
            server.wait(timeout: .now() + 1),
            DispatchTimeoutResult.success
        )
    }

    func testDelayedDaemonOperationDoesNotBlockTheMainRunLoop() {
        let operationFinished = expectation(description: "delayed operation finishes")
        let mainQueueRemainsResponsive = expectation(description: "main queue remains responsive")
        let start = Date()

        ConfigBarAsyncOperation.perform(
            on: DispatchQueue(label: "configbar-test-daemon"),
            work: {
                Thread.sleep(forTimeInterval: 0.5)
                return true
            },
            completion: { result in
                XCTAssertTrue(result)
                operationFinished.fulfill()
            }
        )

        DispatchQueue.main.asyncAfter(deadline: .now() + 0.05) {
            XCTAssertLessThan(Date().timeIntervalSince(start), 0.25)
            mainQueueRemainsResponsive.fulfill()
        }

        wait(for: [mainQueueRemainsResponsive, operationFinished], timeout: 2.0)
    }

    func testResponseLimitsAreCommandSpecific() {
        XCTAssertEqual(
            ConfigBarIPC.maximumResponseBytes(for: ["command": "status"]),
            ConfigBarIPC.defaultMaxResponseBytes
        )
        XCTAssertEqual(
            ConfigBarIPC.maximumResponseBytes(for: ["command": "get_snapshot"]),
            ConfigBarIPC.structuredMaxResponseBytes
        )
        XCTAssertEqual(
            ConfigBarIPC.maximumResponseBytes(for: ["command": "get_available_plugins"]),
            ConfigBarIPC.pluginCatalogMaxResponseBytes
        )
        XCTAssertLessThan(
            ConfigBarIPC.defaultMaxResponseBytes,
            ConfigBarIPC.structuredMaxResponseBytes
        )
        XCTAssertLessThan(
            ConfigBarIPC.structuredMaxResponseBytes,
            ConfigBarIPC.pluginCatalogMaxResponseBytes
        )
    }

    func testProbeTreatsOlderDaemonCommandRejectionAsReachable() {
        XCTAssertTrue(ConfigBarIPC.responseShowsDaemonReachable(["success": true]))
        XCTAssertTrue(ConfigBarIPC.responseShowsDaemonReachable([
            "success": false,
            "error": "unknown command"
        ]))
        XCTAssertFalse(ConfigBarIPC.responseShowsDaemonReachable(["error": "malformed response"]))
    }

    func testResponseTimeoutsAreCommandSpecific() {
        XCTAssertEqual(
            ConfigBarIPC.responseTimeoutMicros(for: ["command": "status"]),
            ConfigBarIPC.defaultResponseTimeoutMicros
        )
        XCTAssertEqual(
            ConfigBarIPC.responseTimeoutMicros(for: ["command": "get_metering"]),
            ConfigBarIPC.defaultResponseTimeoutMicros
        )

        let pipelineMutationCommands = [
            "set_device",
            "apply_configuration",
            "load_plugins",
            "load_plugin_artifact",
            "add_plugin",
            "remove_plugin",
            "update_plugin",
            "reorder_plugins",
            "reorder_graph",
            "set_input_channels",
            "set_output_channels",
            "set_pipeline_channels",
            "set_rack_plugin_state",
        ]
        for command in pipelineMutationCommands {
            XCTAssertEqual(
                ConfigBarIPC.responseTimeoutMicros(for: ["command": command]),
                ConfigBarIPC.pipelineMutationResponseTimeoutMicros,
                command
            )
        }

        XCTAssertLessThan(
            ConfigBarIPC.defaultResponseTimeoutMicros,
            ConfigBarIPC.pipelineMutationResponseTimeoutMicros
        )
        XCTAssertEqual(ConfigBarIPC.pipelineMutationResponseTimeoutMicros, 30_000_000)
    }

    func testConfigurationLoaderReturnsTopLevelObject() throws {
        let directory = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString, isDirectory: true)
        try FileManager.default.createDirectory(at: directory, withIntermediateDirectories: true)
        defer { try? FileManager.default.removeItem(at: directory) }
        let url = directory.appendingPathComponent("plugins.json")
        try Data(#"{"plugins":[]}"#.utf8).write(to: url)

        let artifact = try ConfigBarIPC.loadConfigurationArtifact(from: url)
        XCTAssertNotNil(artifact["plugins"] as? [Any])
    }

    func testConfigurationLoaderRejectsOversizedInputBeforeParsing() throws {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: url) }
        try Data(repeating: UInt8(ascii: " "), count: 9).write(to: url)

        XCTAssertThrowsError(
            try ConfigBarIPC.loadConfigurationArtifact(from: url, maxBytes: 8)
        ) { error in
            XCTAssertEqual(error as? ConfigBarConfigurationError, .tooLarge(maxBytes: 8))
        }
    }

    func testConfigurationLoaderRejectsNonObjectJSON() throws {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: url) }
        try Data("[]".utf8).write(to: url)

        XCTAssertThrowsError(try ConfigBarIPC.loadConfigurationArtifact(from: url)) { error in
            XCTAssertEqual(error as? ConfigBarConfigurationError, .topLevelMustBeObject)
        }
    }

    func testConfigurationLoaderRejectsArtifactAboveDaemonCommandLimit() throws {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString)
        defer { try? FileManager.default.removeItem(at: url) }
        let oversized = String(
            repeating: "x",
            count: ConfigBarIPC.maximumDaemonCommandBytes
        )
        let data = try JSONSerialization.data(withJSONObject: ["value": oversized])
        try data.write(to: url)

        XCTAssertThrowsError(try ConfigBarIPC.loadConfigurationArtifact(from: url)) { error in
            XCTAssertEqual(
                error as? ConfigBarConfigurationError,
                .encodedCommandTooLarge(maxBytes: ConfigBarIPC.maximumDaemonCommandBytes)
            )
        }
    }

    func testLineFramerHandlesFragmentedAndMultipleLines() throws {
        var framer = ConfigBarLineFramer(maxLineBytes: 64)

        try framer.append(Data("{\"success\":true".utf8))
        XCTAssertNil(framer.nextLine())

        try framer.append(Data("}\n{\"success\":false}\n".utf8))
        XCTAssertEqual(framer.nextLine(), Data("{\"success\":true}".utf8))
        XCTAssertEqual(framer.nextLine(), Data("{\"success\":false}".utf8))
        XCTAssertNil(framer.nextLine())
    }

    func testLineFramerRejectsAnOversizedLineWithoutNewline() {
        var framer = ConfigBarLineFramer(maxLineBytes: 4)

        XCTAssertThrowsError(try framer.append(Data("12345".utf8))) { error in
            XCTAssertEqual(error as? ConfigBarIPCError, .lineTooLong)
        }
    }

    func testWriteAllRetriesShortWrites() {
        var sockets: [Int32] = [-1, -1]
        XCTAssertEqual(socketpair(AF_UNIX, SOCK_STREAM, 0, &sockets), 0)
        defer {
            Darwin.close(sockets[0])
            Darwin.close(sockets[1])
        }

        let payload = Data("abcdefghijklmnopqrstuvwxyz".utf8)
        var calls = 0
        var written = 0

        let result = ConfigBarIPC.writeAll(fd: sockets[0], data: payload) { _, _, count, _ in
            calls += 1
            let amount = min(3, count)
            written += amount
            return amount
        }

        XCTAssertTrue(result)
        XCTAssertEqual(written, payload.count)
        XCTAssertGreaterThan(calls, 1)
    }

    func testWriteAllSendsCompleteLineThroughSocketPair() throws {
        var sockets: [Int32] = [-1, -1]
        XCTAssertEqual(socketpair(AF_UNIX, SOCK_STREAM, 0, &sockets), 0)
        defer {
            Darwin.close(sockets[0])
            Darwin.close(sockets[1])
        }

        let payload = Data("{\"success\":true}\n".utf8)
        XCTAssertTrue(ConfigBarIPC.writeAll(fd: sockets[0], data: payload))

        var received = [UInt8](repeating: 0, count: payload.count)
        let count = received.withUnsafeMutableBytes { buffer in
            Darwin.recv(sockets[1], buffer.baseAddress, buffer.count, 0)
        }

        XCTAssertEqual(count, payload.count)
        XCTAssertEqual(Data(received), payload)
    }

    func testWriteToClosedDaemonSocketReturnsFailureWithoutSIGPIPE() throws {
        var sockets: [Int32] = [-1, -1]
        XCTAssertEqual(socketpair(AF_UNIX, SOCK_STREAM, 0, &sockets), 0)
        defer { Darwin.close(sockets[0]) }

        Darwin.close(sockets[1])

        // writeAll itself must install SO_NOSIGPIPE. Without that invariant,
        // this send terminates the test process instead of returning false.
        XCTAssertFalse(
            ConfigBarIPC.writeAll(fd: sockets[0], data: Data("stale\n".utf8))
        )
    }
}
