import Darwin
import Foundation
import XCTest
@testable import ConfigBarModels
@testable import ConfigBarUI

final class ConfigBarUITests: XCTestCase {
    private func makeUnixServer(path: String) throws -> Int32 {
        unlink(path)

        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let pathBytes = path.utf8
        let pathCapacity = MemoryLayout.size(ofValue: address.sun_path)
        XCTAssertLessThan(pathBytes.count + 1, pathCapacity)
        let copiedLength = withUnsafeMutableBytes(of: &address.sun_path) { rawBuffer -> Int in
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
        let bindResult = withUnsafePointer(to: &address) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) { sockaddrPointer in
                Darwin.bind(
                    serverFD,
                    sockaddrPointer,
                    socklen_t(MemoryLayout<sockaddr_un>.size)
                )
            }
        }
        XCTAssertEqual(bindResult, 0)
        XCTAssertEqual(Darwin.listen(serverFD, 4), 0)
        return serverFD
    }

    private func readLine(from fd: Int32, timeout: TimeInterval = 1.0) -> Data? {
        var data = Data()
        var byte: UInt8 = 0
        let deadline = Date().addingTimeInterval(timeout)
        while Date() < deadline {
            let count = withUnsafeMutableBytes(of: &byte) { buffer in
                Darwin.recv(fd, buffer.baseAddress, 1, 0)
            }
            if count == 1 {
                data.append(byte)
                if byte == 10 { return data }
            } else if count == 0 {
                return data.isEmpty ? nil : data
            } else if errno != EINTR {
                return nil
            }
        }
        return nil
    }

    private func sendLine(_ line: String, on fd: Int32) {
        let bytes = Array(line.utf8)
        bytes.withUnsafeBytes { buffer in
            var offset = 0
            while offset < bytes.count {
                let sent = Darwin.send(
                    fd,
                    buffer.baseAddress!.advanced(by: offset),
                    bytes.count - offset,
                    0
                )
                if sent > 0 {
                    offset += sent
                } else if sent < 0 && errno == EINTR {
                    continue
                } else {
                    break
                }
            }
        }
    }

    func testCommandWWindowPolicyOnlyMatchesCommandW() {
        XCTAssertTrue(
            ConfigBarWindowPolicy.shouldDismissCommandW(
                hasCommandModifier: true,
                charactersIgnoringModifiers: "w"
            )
        )
        XCTAssertFalse(
            ConfigBarWindowPolicy.shouldDismissCommandW(
                hasCommandModifier: false,
                charactersIgnoringModifiers: "w"
            )
        )
        XCTAssertFalse(
            ConfigBarWindowPolicy.shouldDismissCommandW(
                hasCommandModifier: true,
                charactersIgnoringModifiers: "q"
            )
        )
    }

    func testUITargetExposesModelBackedConfigurationPolicy() {
        XCTAssertTrue(isConfigBarVirtualDevice("SotF Virtual Audio"))
        XCTAssertFalse(isConfigBarVirtualDevice("Built-in Output"))
    }

    func testDaemonManagerAdoptsLiveFixtureAndRemainsResponsive() throws {
        let path = "/tmp/sotf-configbar-adoption-\(getpid()).sock"
        let serverFD = try makeUnixServer(path: path)
        defer {
            Darwin.close(serverFD)
            unlink(path)
        }

        let oldSocketPath = getenv("SOTF_DAEMON_SOCKET_PATH").map { String(cString: $0) }
        let oldDaemonPath = getenv("SOTF_DAEMON_PATH").map { String(cString: $0) }
        setenv("SOTF_DAEMON_SOCKET_PATH", path, 1)
        // If adoption regresses, launchDaemon may only start this harmless
        // executable; its termination would produce a second false callback.
        setenv("SOTF_DAEMON_PATH", "/bin/false", 1)
        defer {
            if let oldSocketPath { setenv("SOTF_DAEMON_SOCKET_PATH", oldSocketPath, 1) }
            else { unsetenv("SOTF_DAEMON_SOCKET_PATH") }
            if let oldDaemonPath { setenv("SOTF_DAEMON_PATH", oldDaemonPath, 1) }
            else { unsetenv("SOTF_DAEMON_PATH") }
        }

        let server = DispatchWorkItem {
            // The first connection is the startDaemon probe. The second is a
            // post-adoption probe proving the fixture stayed responsive.
            for _ in 0..<2 {
                let clientFD = Darwin.accept(serverFD, nil, nil)
                guard clientFD >= 0 else { return }
                defer { Darwin.close(clientFD) }
                guard self.readLine(from: clientFD) != nil else { return }
                self.sendLine(
                    "{\"success\":true,\"data\":{},\"error\":null}\n",
                    on: clientFD
                )
            }
        }
        DispatchQueue.global(qos: .utility).async(execute: server)

        let adopted = expectation(description: "live daemon is adopted")
        var callbackValues: [Bool] = []
        let manager = DaemonManager()
        manager.onStatusChange = { reachable in
            callbackValues.append(reachable)
            if reachable { adopted.fulfill() }
        }

        manager.startDaemon()
        wait(for: [adopted], timeout: 2.0)

        XCTAssertFalse(manager.isDaemonRunning)
        XCTAssertEqual(manager.restartActionTitle, "Reconnect")
        XCTAssertEqual(callbackValues, [true])
        XCTAssertTrue(ConfigBarIPC.probeDaemon(socketPath: path, timeoutMilliseconds: 500))
        XCTAssertEqual(server.wait(timeout: .now() + 1), .success)
        manager.stopDaemon()
    }

    func testStatusAndMeteringPollingReuseOneSocket() throws {
        let path = "/tmp/sotf-configbar-polling-\(getpid()).sock"
        let serverFD = try makeUnixServer(path: path)
        defer {
            AudioEngineClient.resetPollingConnectionForTests()
            Darwin.close(serverFD)
            unlink(path)
        }

        let oldSocketPath = getenv("SOTF_DAEMON_SOCKET_PATH").map { String(cString: $0) }
        setenv("SOTF_DAEMON_SOCKET_PATH", path, 1)
        defer {
            if let oldSocketPath { setenv("SOTF_DAEMON_SOCKET_PATH", oldSocketPath, 1) }
            else { unsetenv("SOTF_DAEMON_SOCKET_PATH") }
        }

        let lock = NSLock()
        var acceptedConnections = 0
        let server = DispatchWorkItem {
            let clientFD = Darwin.accept(serverFD, nil, nil)
            guard clientFD >= 0 else { return }
            lock.lock()
            acceptedConnections += 1
            lock.unlock()
            defer { Darwin.close(clientFD) }

            for _ in 0..<2 {
                guard let request = self.readLine(from: clientFD),
                      let command = try? JSONSerialization.jsonObject(with: request) as? [String: Any]
                else { return }
                let response: String
                if command["command"] as? String == "status" {
                    response = "{\"success\":true,\"data\":{\"state\":\"Idle\",\"volume\":1.0},\"error\":null}\n"
                } else {
                    response = "{\"success\":true,\"data\":{},\"error\":null}\n"
                }
                self.sendLine(response, on: clientFD)
            }
        }
        DispatchQueue.global(qos: .utility).async(execute: server)

        let statusDone = expectation(description: "status poll completes")
        let meteringDone = expectation(description: "metering poll completes")
        AudioEngineClient.pollStatus { status, reachable in
            XCTAssertEqual(status.state, .idle)
            XCTAssertTrue(reachable)
            statusDone.fulfill()
        }
        AudioEngineClient.pollMetering { metering in
            XCTAssertNotNil(metering)
            meteringDone.fulfill()
        }

        wait(for: [statusDone, meteringDone], timeout: 2.0)
        XCTAssertEqual(server.wait(timeout: .now() + 1), .success)

        // The polling queue is serial and both commands were completed on the
        // same accepted stream. No timer tick may create another connection.
        RunLoop.current.run(until: Date().addingTimeInterval(0.2))
        lock.lock()
        let count = acceptedConnections
        lock.unlock()
        XCTAssertEqual(count, 1)
    }
}
