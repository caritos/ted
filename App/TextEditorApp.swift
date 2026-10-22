import SwiftUI
import AppKit
import TextEditorKit

@main
struct TextEditorApp: App {
    @State private var model = EditorModel()
    @State private var errorMessage: String?

    var body: some Scene {
        WindowGroup {
            ContentView(errorMessage: $errorMessage)
                .environment(model)
                .frame(minWidth: 480, minHeight: 320)
        }
        .commands {
            CommandGroup(replacing: .newItem) {
                Button("Open…") { open() }
                    .keyboardShortcut("o")
            }
            CommandGroup(replacing: .saveItem) {
                Button("Save") { save() }
                    .keyboardShortcut("s")
                Button("Save As…") { saveAs() }
                    .keyboardShortcut("s", modifiers: [.command, .shift])
            }
        }
    }

    private func open() {
        let panel = NSOpenPanel()
        panel.allowsMultipleSelection = false
        panel.canChooseDirectories = false
        guard panel.runModal() == .OK, let url = panel.url else { return }
        perform { try model.open(url) }
    }

    private func save() {
        if model.url == nil { saveAs(); return }
        perform { try model.save() }
    }

    private func saveAs() {
        let panel = NSSavePanel()
        guard panel.runModal() == .OK, let url = panel.url else { return }
        perform { try model.save(as: url) }
    }

    private func perform(_ action: () throws -> Void) {
        do {
            try action()
            errorMessage = nil
        } catch FileStoreError.notUTF8 {
            errorMessage = "That file isn't UTF-8 text, so it wasn't opened."
        } catch {
            errorMessage = error.localizedDescription
        }
    }
}
