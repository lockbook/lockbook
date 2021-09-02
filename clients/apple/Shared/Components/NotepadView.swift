import SwiftUI
import NotepadSwift
import Down
import Combine

#if os(iOS)
struct NotepadView: UIViewRepresentable {
    let text: String
    var frame: CGRect
    let theme: Theme
    let onTextChange: (String) -> Void
    let engine = MarkdownEngine()
    
    init(text: String, frame: CGRect, theme: Theme, onTextChange: @escaping (String) -> Void) {
        self.text = text
        self.frame = frame
        self.theme = theme
        self.onTextChange = onTextChange
    }

    func makeUIView(context: Context) -> UITextView {
        let np = Notepad(frame: frame, theme: theme)
        np.smartQuotesType = .no
        np.smartDashesType = .no
        np.smartInsertDeleteType = .no
        np.onTextChange = onTextChange
        np.storage.markdowner = { engine.render($0) }
        np.storage.applyMarkdown = { m in applyMarkdown(markdown: m) }
        np.storage.applyBody = { applyBody() }
        np.text = text
        np.styleNow()

        return np
    }

    func updateUIView(_ uiView: UITextView, context: Context) {
        uiView.text = text
    }
}
#else
struct NotepadView: NSViewRepresentable {
    let text: String
    var frame: CGRect
    let theme: Theme
    let onTextChange: (String) -> Void
    let engine = MarkdownEngine()

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSTextView.scrollableTextView()
        let np = Notepad(frame: frame)
        np.allowsUndo = true
        np.isAutomaticQuoteSubstitutionEnabled = false
        np.isAutomaticDashSubstitutionEnabled = false
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
        if (nsView.frame != frame) {
            nsView.frame = frame
            if let np = nsView.documentView as? Notepad {
                np.frame = frame.insetBy(dx: 10, dy: 0)
                // Scroll to cursor
//                DispatchQueue.main.asyncAfter(deadline: .now() + .milliseconds(100)) {
//                    np.scrollRangeToVisible(np.selectedRange())
//                }
            }
        }
    }
}

#endif
