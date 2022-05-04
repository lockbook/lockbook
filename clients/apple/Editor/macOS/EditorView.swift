import Foundation
import SwiftUI

struct EditorView: NSViewRepresentable {
    
    @EnvironmentObject var model: DocumentLoader
    let frame: CGRect
    let storage = Storage()
    
    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSScrollView()

        let layoutManager = NSLayoutManager()
        storage.addLayoutManager(layoutManager)
        print("frame 1 \(frame)")

        let textContainer = NSTextContainer(containerSize: scrollView.frame.size)
        textContainer.widthTracksTextView = true
        textContainer.containerSize = NSSize(
            width: scrollView.contentSize.width,
            height: CGFloat.greatestFiniteMagnitude
        )
        layoutManager.addTextContainer(textContainer)
        
        let textView = CustomNSTextView(frame: .zero, textContainer: textContainer)
        textView.autoresizingMask = .width
        textView.isVerticallyResizable = true
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.minSize = NSSize(width: 0, height: scrollView.contentSize.height)
        textView.delegate = context.coordinator
        textView.textContainerInset = NSSize(width: horizontalInset(), height: 20)
        textView.string = model.textDocument!
        textView.allowsUndo = true
        storage.syntaxHighlight()
        
        scrollView.documentView = textView
        scrollView.hasVerticalScroller = true
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
            nsView.textContainerInset = NSSize(width: horizontalInset(), height: 20)
            if model.reloadContent {
                model.reloadContent = false
                nsView.string = model.textDocument!
                
                // Seems like that let doesn't store the same ref
                (nsView.textStorage as! Storage).syntaxHighlight()
            }
        }
    }
    
    func horizontalInset() -> CGFloat {
        let maxDocumentWidth = 600
        let minInset = 25
        
        var inset = minInset
        print(frame.width, CGFloat(maxDocumentWidth + minInset * 2))
        if frame.width > CGFloat(maxDocumentWidth + minInset * 2) {
            inset = (Int(frame.width) - maxDocumentWidth) / 2
        }
        return CGFloat(inset)
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
        guard let storage = textView.textStorage as? Storage else {
            print("Wrong storage type attached to this textview")
            return
        }
        
        storage.syntaxHighlight()
        
        onTextChange(textView.string)
    }
}
