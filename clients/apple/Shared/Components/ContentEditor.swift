import SwiftUI

struct ContentEditor: View {
    let text: Binding<String>
    var body: some View {
        TextEditor(text: text)
            .padding(0.01)
    }
}
