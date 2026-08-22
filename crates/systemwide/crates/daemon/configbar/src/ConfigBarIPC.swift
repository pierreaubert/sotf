import Foundation
import Darwin

public enum ConfigBarIPCError: Error, Equatable {
    case lineTooLong
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

    public typealias SendFunction = (
        Int32,
        UnsafeRawPointer?,
        Int,
        Int32
    ) -> Int

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

    /// Probe a live daemon using the same line-oriented status command as the
    /// Configbar startup path. This is deliberately small and synchronous so
    /// callers can place it on a background queue; it also makes adoption
    /// behavior testable with a real Unix-domain listener.
    public static func probeDaemon(
        socketPath: String,
        timeoutMilliseconds: Int32 = 250
    ) -> Bool {
        guard let socketFD = connectUnixSocket(socketPath) else { return false }
        defer { Darwin.close(socketFD) }

        let command = Data("{\"command\":\"status\"}\n".utf8)
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
            return object["success"] as? Bool == true
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
