import Foundation
import Testing
@testable import TextEditorKit

final class FakeFileStore: FileStore, @unchecked Sendable {
    var files: [URL: String] = [:]
    var failNextRead = false
    var failNextWrite = false

    func read(_ url: URL) throws -> String {
        if failNextRead { failNextRead = false; throw FileStoreError.unreadable }
        guard let text = files[url] else { throw FileStoreError.unreadable }
        return text
    }

    func write(_ text: String, to url: URL) throws {
        if failNextWrite { failNextWrite = false; throw FileStoreError.unwritable }
        files[url] = text
    }
}

@MainActor
struct EditorModelTests {
    let url = URL(fileURLWithPath: "/tmp/a.txt")

    @Test func startsEmptyAndClean() {
        let model = EditorModel(store: FakeFileStore())
        #expect(model.text == "")
        #expect(model.url == nil)
        #expect(model.isDirty == false)
    }

    @Test func openLoadsTextAndIsClean() throws {
        let store = FakeFileStore()
        store.files[url] = "hello"
        let model = EditorModel(store: store)
        try model.open(url)
        #expect(model.text == "hello")
        #expect(model.url == url)
        #expect(model.isDirty == false)
    }

    @Test func editingMarksDirty() throws {
        let store = FakeFileStore()
        store.files[url] = "hello"
        let model = EditorModel(store: store)
        try model.open(url)
        model.text = "hello world"
        #expect(model.isDirty)
    }

    @Test func editingBackToOriginalIsClean() throws {
        let store = FakeFileStore()
        store.files[url] = "hello"
        let model = EditorModel(store: store)
        try model.open(url)
        model.text = "hello!"
        model.text = "hello"
        #expect(model.isDirty == false)
    }

    @Test func saveWritesAndClearsDirty() throws {
        let store = FakeFileStore()
        store.files[url] = "hello"
        let model = EditorModel(store: store)
        try model.open(url)
        model.text = "changed"
        try model.save()
        #expect(store.files[url] == "changed")
        #expect(model.isDirty == false)
    }

    @Test func saveWithoutURLThrows() {
        let model = EditorModel(store: FakeFileStore())
        model.text = "x"
        #expect(throws: EditorError.noFile) { try model.save() }
    }

    @Test func saveAsSetsURLAndWrites() throws {
        let store = FakeFileStore()
        let model = EditorModel(store: store)
        model.text = "new"
        try model.save(as: url)
        #expect(model.url == url)
        #expect(store.files[url] == "new")
        #expect(model.isDirty == false)
    }

    @Test func failedOpenKeepsCurrentDocument() throws {
        let store = FakeFileStore()
        store.files[url] = "keep me"
        let model = EditorModel(store: store)
        try model.open(url)
        model.text = "unsaved"
        let other = URL(fileURLWithPath: "/tmp/missing.txt")
        #expect(throws: FileStoreError.unreadable) { try model.open(other) }
        #expect(model.text == "unsaved")
        #expect(model.url == url)
        #expect(model.isDirty)
    }

    @Test func failedSaveStaysDirty() throws {
        let store = FakeFileStore()
        store.files[url] = "hello"
        let model = EditorModel(store: store)
        try model.open(url)
        model.text = "changed"
        store.failNextWrite = true
        #expect(throws: FileStoreError.unwritable) { try model.save() }
        #expect(model.isDirty)
    }
}

struct DiskFileStoreTests {
    @Test func roundTripsUTF8() throws {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString + ".txt")
        defer { try? FileManager.default.removeItem(at: url) }
        let store = DiskFileStore()
        try store.write("héllo\nwörld 🌍", to: url)
        #expect(try store.read(url) == "héllo\nwörld 🌍")
    }

    @Test func missingFileThrows() {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString + ".txt")
        #expect(throws: (any Error).self) { try DiskFileStore().read(url) }
    }

    @Test func nonUTF8FileThrowsInsteadOfCorrupting() throws {
        let url = FileManager.default.temporaryDirectory
            .appendingPathComponent(UUID().uuidString + ".bin")
        defer { try? FileManager.default.removeItem(at: url) }
        try Data([0xFF, 0xFE, 0x00, 0xC0]).write(to: url)
        #expect(throws: FileStoreError.notUTF8) { try DiskFileStore().read(url) }
    }
}
