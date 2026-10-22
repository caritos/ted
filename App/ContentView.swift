import SwiftUI
import TextEditorKit

struct ContentView: View {
    @Environment(EditorModel.self) private var model
    @Binding var errorMessage: String?

    var body: some View {
        @Bindable var model = model
        VStack(spacing: 0) {
            if let errorMessage {
                Text(errorMessage)
                    .frame(maxWidth: .infinity, alignment: .leading)
                    .padding(8)
                    .background(.red.opacity(0.15))
            }
            TextEditor(text: $model.text)
                .font(.system(.body, design: .monospaced))
        }
        .navigationTitle(title)
    }

    private var title: String {
        let name = model.url?.lastPathComponent ?? "Untitled"
        return model.isDirty ? "\(name) — Edited" : name
    }
}
