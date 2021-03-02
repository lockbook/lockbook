import SwiftUI
import NotepadSwift

#if os(iOS)
struct NotepadView: UIViewRepresentable {
    @Binding var text: String
    var frame: CGRect
    let theme: Theme
    let onTextChange: (String) -> Void

    func makeUIView(context: Context) -> UITextView {
        let np = Notepad(frame: frame, theme: theme)
        np.onTextChange = onTextChange
        np.text = text

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

    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSTextView.scrollableTextView()
        let np = Notepad(frame: frame)
        np.onTextChange = onTextChange
        np.storage.theme = theme
        np.insertionPointColor = theme.tintColor
        np.layoutManager?.replaceTextStorage(np.storage)
        scrollView.documentView = np
        np.string = text

        return scrollView
    }

    func updateNSView(_ nsView: NSScrollView, context: Context) {

    }
}

#endif
