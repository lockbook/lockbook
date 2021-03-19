import SwiftUI
import NotepadSwift
import Down
import Combine

#if os(iOS)
struct NotepadView: UIViewRepresentable {
    @Binding var text: String
    var frame: CGRect
    let theme: Theme
    let onTextChange: (String) -> Void
    let engine = MarkdownEngine()

    func makeUIView(context: Context) -> UITextView {
        let np = Notepad(frame: frame, theme: theme)
        np.onTextChange = onTextChange
        np.storage.markdowner = { engine.render($0) }
        np.storage.applyMarkdown = { m in applyMarkdown(markdown: m) }
        np.storage.applyBody = { applyBody() }
        np.text = text
        np.styleNow()

        return np
    }

    func updateUIView(_ uiView: UITextView, context: Context) {

    }
}
#else
struct NotepadView: NSViewRepresentable {
    @Binding var text: String
    var frame: CGRect
    let theme: Theme
    let onTextChange: (String) -> Void
    let engine = MarkdownEngine()

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSTextView.scrollableTextView()
        let np = Notepad(frame: frame)
        np.onTextChange = onTextChange
        np.storage.markdowner = { engine.render($0) }
        np.storage.applyMarkdown = { m in applyMarkdown(markdown: m) }
        np.storage.applyBody = { applyBody() }
        np.storage.theme = theme
        np.insertionPointColor = theme.tintColor
        np.layoutManager?.replaceTextStorage(np.storage)
        scrollView.documentView = np
        np.string = text
        np.styleNow()

        return scrollView
    }

    func updateNSView(_ nsView: NSScrollView, context: Context) {

    }
}

#endif
