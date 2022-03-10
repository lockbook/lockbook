import Foundation
import SwiftUI

struct EditorView: UIViewRepresentable {
    
    @EnvironmentObject var model: DocumentLoader
    let storage = Storage()
    
    lazy var delegate: Coordinator = Coordinator(textChange: updateModel)
    
    func makeUIView(context: Context) -> UITextView {
        let layoutManager = NSLayoutManager()
        storage.addLayoutManager(layoutManager)
        let textContainer = NSTextContainer(size: .zero)
        layoutManager.addTextContainer(textContainer)
        
        let textView = UITextView(frame: .zero, textContainer: textContainer)
        textView.delegate = context.coordinator
        textView.text = model.textDocument!
        textView.autoresizingMask = .flexibleHeight
        storage.syntaxHighlight()
        return textView
    }
    
    func makeCoordinator() -> Coordinator {
        Coordinator(textChange: updateModel)
    }
    
    func updateModel(update: String) {
        model.textDocument = update
    }
    
    func updateUIView(_ uiView: UITextView, context: Context) {
        if model.reloadContent {
            model.reloadContent = false
            uiView.text = model.textDocument!
        }
    }
}

class Coordinator: NSObject, UITextViewDelegate {
    
    public var onTextChange: (String) -> Void
    
    init(textChange: @escaping (String) -> Void) {
        self.onTextChange = textChange
    }
    
    public func textViewDidChange(_ textView: UITextView) {
        guard let storage = textView.textStorage as? Storage else {
            print("Wrong storage type attached to this textview")
            return
        }
        
        storage.syntaxHighlight()
        
        onTextChange(textView.text)
    }
}

// TODO check for sporadic scrolling, it did not happen in the old implementation
// but it does seem to happen with a raw UITextView subclass, anytime text is set
// the document will bounce to about 10% scrolled. This happens with a non-subclass
// too. Toggling scrolling on and off seems to help:
// https://stackoverflow.com/a/2757655/1060955
