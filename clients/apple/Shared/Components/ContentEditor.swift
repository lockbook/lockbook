import HighlightedTextEditor
import SwiftUI

struct ContentEditor: View {
    let text: Binding<String>
    
    var body: some View {
        HighlightedTextEditor(text: text, highlightRules: .markdown)
            .padding(0.01)
    }
}
