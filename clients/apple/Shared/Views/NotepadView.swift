import SwiftUI
import NotepadSwift
import Down
import Combine

#if os(iOS)
struct NotepadView: UIViewRepresentable {
    @State var model: DocumentLoader
    var frame: CGRect
    let theme: Theme
    let engine = MarkdownEngine()
    
    func makeUIView(context: Context) -> UITextView {
        let np = Notepad(frame: frame, theme: theme)
        np.smartQuotesType = .no
        np.smartDashesType = .no
        np.smartInsertDeleteType = .no
        np.onTextChange = { model.textDocument = $0 }
        np.storage.markdowner = { engine.render($0) }
        np.storage.applyMarkdown = { m in applyMarkdown(markdown: m) }
        np.storage.applyBody = { applyBody() }
        // If this is null we should crash on the spot and avoid writing garbage to the file
        np.text = model.textDocument!
        np.styleNow()

        return np
    }

    func updateUIView(_ uiView: UITextView, context: Context) {
        if let np = uiView as? Notepad, model.reloadContent {
            print("reload happened")
            model.reloadContent = false
            np.text = model.textDocument!
            np.styleNow()
        }
    }
}
#else
struct NotepadView: NSViewRepresentable {
    @StateObject var model: DocumentLoader
    var frame: CGRect
    let theme: Theme
    let engine = MarkdownEngine()

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSTextView.scrollableTextView()
        let np = Notepad(frame: frame)
        np.allowsUndo = true
        np.isAutomaticQuoteSubstitutionEnabled = false
        np.isAutomaticDashSubstitutionEnabled = false
        np.onTextChange = { model.textDocument = $0 }
        np.storage.markdowner = { engine.render($0) }
        np.storage.applyMarkdown = { m in applyMarkdown(markdown: m) }
        np.storage.applyBody = { applyBody() }
        np.storage.theme = theme
        np.insertionPointColor = theme.tintColor
        np.layoutManager?.replaceTextStorage(np.storage)
        scrollView.documentView = np
        np.string = model.textDocument!
        np.styleNow()

        return scrollView
    }

    func updateNSView(_ nsView: NSScrollView, context: Context) {
        print("updateUIView called")

        if let np = nsView.documentView as? Notepad, model.reloadContent {
            model.reloadContent = false
            np.string = model.textDocument!
            np.styleNow()
        }
        
        // This bit is what wraps our text
        if (nsView.frame != frame) {
            nsView.frame = frame
            if let np = nsView.documentView as? Notepad {
                np.frame = frame.insetBy(dx: 10, dy: 0)
            }
        }
    }
}

#endif
