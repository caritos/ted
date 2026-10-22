import Foundation

public enum FileStoreError: Error, Equatable {
    case unreadable
    case unwritable
    case notUTF8
}

public protocol FileStore: Sendable {
    func read(_ url: URL) throws -> String
    func write(_ text: String, to url: URL) throws
}

public struct DiskFileStore: FileStore {
    public init() {}

    public func read(_ url: URL) throws -> String {
        let data = try Data(contentsOf: url)
        guard let text = String(data: data, encoding: .utf8) else {
            throw FileStoreError.notUTF8
        }
        return text
    }

    public func write(_ text: String, to url: URL) throws {
        try Data(text.utf8).write(to: url, options: .atomic)
    }
}
