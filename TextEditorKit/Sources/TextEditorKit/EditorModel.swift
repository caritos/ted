import Foundation
import Observation

public enum EditorError: Error, Equatable {
    case noFile
}

@MainActor @Observable
public final class EditorModel {
    public var text: String = ""
    public private(set) var url: URL?

    private var savedText: String = ""
    private let store: FileStore

    public init(store: FileStore = DiskFileStore()) {
        self.store = store
    }

    public var isDirty: Bool { text != savedText }

    public func open(_ url: URL) throws {
        let loaded = try store.read(url)
        text = loaded
        savedText = loaded
        self.url = url
    }

    public func save() throws {
        guard let url else { throw EditorError.noFile }
        try store.write(text, to: url)
        savedText = text
    }

    public func save(as url: URL) throws {
        try store.write(text, to: url)
        self.url = url
        savedText = text
    }
}
