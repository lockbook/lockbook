import SwiftUI
#if os(macOS)
import AppKit
#else
import UIKit
#endif

struct ContentEditor: View {
    let text: Binding<String>
    var body: some View {
        TextEditor(text: text)
            .padding(0.01)
    }
}
