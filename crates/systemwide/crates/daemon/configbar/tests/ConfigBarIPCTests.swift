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
            _ = request.withUnsafeMutableBytes { buffer in
                Darwin.recv(clientFD, buffer.baseAddress, buffer.count, 0)
            }
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
        let payload = Data("abcdefghijklmnopqrstuvwxyz".utf8)
        var calls = 0
        var written = 0

        let result = ConfigBarIPC.writeAll(fd: -1, data: payload) { _, _, count, _ in
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
}
