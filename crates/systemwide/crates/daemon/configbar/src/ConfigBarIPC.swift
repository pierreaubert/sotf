import Foundation
import Darwin

public enum ConfigBarIPCError: Error, Equatable {
    case lineTooLong
}

public enum ConfigBarConfigurationError: LocalizedError, Equatable {
    case tooLarge(maxBytes: Int)
    case encodedCommandTooLarge(maxBytes: Int)
    case topLevelMustBeObject

    public var errorDescription: String? {
        switch self {
        case .tooLarge(let maxBytes):
            return "Configuration exceeds the \(maxBytes / 1024) KiB size limit"
        case .encodedCommandTooLarge(let maxBytes):
            return "Configuration exceeds the daemon's \(maxBytes / 1024) KiB command limit"
        case .topLevelMustBeObject:
            return "Configuration must contain a JSON object"
        }
    }
}

/// Small, platform-level helpers used by the configbar's line-oriented daemon
/// protocol. Keeping framing and write-all behavior separate makes the socket
/// contract testable without constructing the SwiftUI application.
public struct ConfigBarLineFramer {
    public let maxLineBytes: Int
    public private(set) var bufferedData = Data()

    public init(maxLineBytes: Int = 64 * 1024) {
        precondition(maxLineBytes > 0)
        self.maxLineBytes = maxLineBytes
    }

    public mutating func append(_ data: Data) throws {
        bufferedData.append(data)

        if let newline = bufferedData.firstIndex(of: UInt8(ascii: "\n")) {
            let lineLength = bufferedData.distance(from: bufferedData.startIndex, to: newline)
            if lineLength > maxLineBytes {
                throw ConfigBarIPCError.lineTooLong
            }
        } else if bufferedData.count > maxLineBytes {
            throw ConfigBarIPCError.lineTooLong
        }
    }

    public mutating func nextLine() -> Data? {
        guard let newline = bufferedData.firstIndex(of: UInt8(ascii: "\n")) else {
            return nil
        }

        let line = Data(bufferedData[..<newline])
        bufferedData.removeSubrange(bufferedData.startIndex...newline)
        return line
    }
}

public enum ConfigBarIPC {
    public static let defaultMaxResponseBytes = 64 * 1024
    public static let structuredMaxResponseBytes = 256 * 1024
    public static let pluginCatalogMaxResponseBytes = 1024 * 1024
    public static let defaultResponseTimeoutMicros: useconds_t = 1_000_000
    // A failed CoreAudio start can consume its 10-second startup bound and
    // pipeline recovery can make a second bounded attempt. Keep this larger
    // than that complete transaction while the UI remains asynchronous.
    public static let pipelineMutationResponseTimeoutMicros: useconds_t = 30_000_000
    public static let maximumConfigurationFileBytes = 1024 * 1024
    public static let maximumDaemonCommandBytes = 64 * 1024

    /// Read at most one byte beyond the configured limit, then parse a plugin
    /// artifact without ever materialising an unbounded file in the UI process.
    public static func loadConfigurationArtifact(
        from url: URL,
        maxBytes: Int = maximumConfigurationFileBytes
    ) throws -> [String: Any] {
        precondition(maxBytes > 0 && maxBytes < Int.max)
        let handle = try FileHandle(forReadingFrom: url)
        defer { try? handle.close() }

        var data = Data()
        while data.count <= maxBytes {
            let remaining = maxBytes + 1 - data.count
            guard let chunk = try handle.read(upToCount: remaining), !chunk.isEmpty else {
                break
            }
            data.append(chunk)
        }
        guard data.count <= maxBytes else {
            throw ConfigBarConfigurationError.tooLarge(maxBytes: maxBytes)
        }
        let json = try JSONSerialization.jsonObject(with: data)
        guard let artifact = json as? [String: Any] else {
            throw ConfigBarConfigurationError.topLevelMustBeObject
        }
        let command: [String: Any] = [
            "command": "load_plugin_artifact",
            "artifact": artifact
        ]
        let encodedCommand = try JSONSerialization.data(withJSONObject: command)
        // The daemon's line bound includes the newline appended by the client.
        guard encodedCommand.count < maximumDaemonCommandBytes else {
            throw ConfigBarConfigurationError.encodedCommandTooLarge(
                maxBytes: maximumDaemonCommandBytes
            )
        }
        return artifact
    }

    /// Bound response allocation according to the requested endpoint instead
    /// of granting every command the plugin catalog's 1 MiB budget.
    public static func maximumResponseBytes(for command: [String: Any]) -> Int {
        switch command["command"] as? String {
        case "get_available_plugins":
            return pluginCatalogMaxResponseBytes
        case "dump_state", "get_snapshot", "get_plugins":
            return structuredMaxResponseBytes
        default:
            return defaultMaxResponseBytes
        }
    }

    /// Pipeline mutations can synchronously stop and recreate CoreAudio output.
    /// Keep polling commands on the short deadline, but allow bounded headroom
    /// for hardware device startup before treating the daemon reply as lost.
    public static func responseTimeoutMicros(for command: [String: Any]) -> useconds_t {
        switch command["command"] as? String {
        case "set_device",
             "apply_configuration",
             "load_plugins",
             "load_plugin_artifact",
             "load_plugin_artifact_path",
             "add_plugin",
             "remove_plugin",
             "update_plugin",
             "reorder_plugins",
             "reorder_graph",
             "set_input_channels",
             "set_output_channels",
             "set_pipeline_channels",
             "set_rack_plugin_state":
            return pipelineMutationResponseTimeoutMicros
        default:
            return defaultResponseTimeoutMicros
        }
    }

    /// Any protocol-shaped reply proves the process is reachable. In
    /// particular, an older daemon may reject the newer `ping` command but
    /// must be adopted rather than repeatedly killed during an upgrade.
    public static func responseShowsDaemonReachable(_ object: [String: Any]) -> Bool {
        object["success"] as? Bool != nil
    }

    public typealias SendFunction = (
        Int32,
        UnsafeRawPointer?,
        Int,
        Int32
    ) -> Int

    /// Prevent a stale Unix-domain connection from terminating ConfigBar.
    /// Darwin raises SIGPIPE by default when `send` races a daemon-side close;
    /// the UI must receive `EPIPE` and reconnect instead.
    @discardableResult
    public static func suppressSigPipe(on socketFD: Int32) -> Bool {
        var enabled: Int32 = 1
        return withUnsafePointer(to: &enabled) { pointer in
            Darwin.setsockopt(
                socketFD,
                SOL_SOCKET,
                SO_NOSIGPIPE,
                pointer,
                socklen_t(MemoryLayout<Int32>.size)
            )
        } == 0
    }

    /// Send all bytes in `data`, handling short writes and EINTR.
    @discardableResult
    public static func writeAll(
        fd: Int32,
        data: Data,
        send: @escaping SendFunction = { fd, buffer, length, flags in
            Darwin.send(fd, buffer, length, flags)
        }
    ) -> Bool {
        guard !data.isEmpty else { return true }
        // Keep the safety guarantee at the write boundary as well as the
        // socket factories. A new caller must not be able to make a stale
        // daemon connection process-fatal by omitting SO_NOSIGPIPE.
        guard suppressSigPipe(on: fd) else {
            return false
        }

        return data.withUnsafeBytes { rawBuffer in
            guard let baseAddress = rawBuffer.baseAddress else { return false }

            var offset = 0
            while offset < data.count {
                let result = send(
                    fd,
                    baseAddress.advanced(by: offset),
                    data.count - offset,
                    0
                )
                if result > 0 {
                    offset += result
                } else if result < 0 && errno == EINTR {
                    continue
                } else {
                    return false
                }
            }
            return true
        }
    }

    /// Probe the daemon without acquiring runtime-state locks. Full status is
    /// intentionally not used here: CoreAudio startup and pipeline replacement
    /// can hold those locks for several seconds while the daemon is healthy.
    public static func probeDaemon(
        socketPath: String,
        timeoutMilliseconds: Int32 = 1_000
    ) -> Bool {
        guard let socketFD = connectUnixSocket(socketPath) else { return false }
        defer { Darwin.close(socketFD) }

        let command = Data("{\"command\":\"ping\"}\n".utf8)
        guard writeAll(fd: socketFD, data: command) else { return false }

        var framer = ConfigBarLineFramer(maxLineBytes: 64 * 1024)
        let deadline = Date().addingTimeInterval(Double(timeoutMilliseconds) / 1000.0)
        var buffer = [UInt8](repeating: 0, count: 4096)

        while Date() < deadline {
            let remainingMilliseconds = max(
                1,
                Int32(Date().distance(to: deadline) * 1000.0)
            )
            var descriptor = pollfd(
                fd: socketFD,
                events: Int16(POLLIN),
                revents: 0
            )
            guard Darwin.poll(&descriptor, 1, remainingMilliseconds) > 0 else {
                return false
            }

            let bytesRead = buffer.withUnsafeMutableBytes { rawBuffer in
                Darwin.recv(socketFD, rawBuffer.baseAddress, rawBuffer.count, 0)
            }
            guard bytesRead > 0 else { return false }

            do {
                try framer.append(Data(buffer.prefix(bytesRead)))
            } catch {
                return false
            }

            guard let line = framer.nextLine(),
                  let object = try? JSONSerialization.jsonObject(with: line) as? [String: Any]
            else {
                continue
            }
            return responseShowsDaemonReachable(object)
        }

        return false
    }

    private static func connectUnixSocket(_ socketPath: String) -> Int32? {
        var address = sockaddr_un()
        address.sun_family = sa_family_t(AF_UNIX)
        let pathBytes = socketPath.utf8
        let pathCapacity = MemoryLayout.size(ofValue: address.sun_path)
        guard !pathBytes.contains(0), pathBytes.count + 1 < pathCapacity else {
            return nil
        }

        let socketFD = Darwin.socket(AF_UNIX, SOCK_STREAM, 0)
        guard socketFD >= 0 else { return nil }
        guard suppressSigPipe(on: socketFD) else {
            Darwin.close(socketFD)
            return nil
        }
        let copiedLength = withUnsafeMutableBytes(of: &address.sun_path) { rawBuffer -> Int in
            guard let baseAddress = rawBuffer.baseAddress else { return -1 }
            return socketPath.withCString { pathCString in
                Int(strlcpy(
                    baseAddress.assumingMemoryBound(to: CChar.self),
                    pathCString,
                    rawBuffer.count
                ))
            }
        }
        guard copiedLength == pathBytes.count else {
            Darwin.close(socketFD)
            return nil
        }

        let result = withUnsafePointer(to: &address) { pointer in
            pointer.withMemoryRebound(to: sockaddr.self, capacity: 1) { sockaddrPointer in
                Darwin.connect(
                    socketFD,
                    sockaddrPointer,
                    socklen_t(MemoryLayout<sockaddr_un>.size)
                )
            }
        }
        guard result == 0 else {
            Darwin.close(socketFD)
            return nil
        }
        return socketFD
    }
}
