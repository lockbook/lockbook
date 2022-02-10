import Foundation
import SwiftUI

struct EditorView: NSViewRepresentable {
    
    @EnvironmentObject var model: DocumentLoader
    
    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSScrollView()
        let textView = NSTextView()
        
        textView.autoresizingMask = .width
        textView.delegate = context.coordinator
        textView.string = model.textDocument!
        textView.allowsUndo = true
        
        scrollView.documentView = textView
        return scrollView
    }
    
    func makeCoordinator() -> Coordinator {
        Coordinator(textChange: updateModel)
    }
    
    func updateModel(update: String) {
        model.textDocument = update
    }
    
    func updateNSView(_ scroll: NSScrollView, context: Context) {
        if let nsView = scroll.documentView as? NSTextView {
            if model.reloadContent {
                model.reloadContent = false
                
                nsView.string = model.textDocument!
            }
        }
    }
}

class Coordinator: NSObject, NSTextViewDelegate {
    public var onTextChange: (String) -> Void
    
    init(textChange: @escaping (String) -> Void) {
        self.onTextChange = textChange
    }
    
    public func textDidChange(_ notification: Notification) {
        guard let textView = notification.object as? NSTextView else {
            print("textview not a textview")
            return
        }
        
        onTextChange(textView.string)
    }
}
