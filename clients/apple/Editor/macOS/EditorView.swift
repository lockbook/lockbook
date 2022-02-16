import Foundation
import SwiftUI

struct EditorView: NSViewRepresentable {
    
    @EnvironmentObject var model: DocumentLoader
    let storage = Storage()
    
    func makeNSView(context: Context) -> NSScrollView {
        let scrollView = NSScrollView()

        let layoutManager = NSLayoutManager()
        storage.addLayoutManager(layoutManager)

        let textContainer = NSTextContainer(containerSize: scrollView.frame.size)
        textContainer.widthTracksTextView = true
        textContainer.containerSize = NSSize(
            width: scrollView.contentSize.width,
            height: CGFloat.greatestFiniteMagnitude
        )
        layoutManager.addTextContainer(textContainer)
        
        let textView = NSTextView(frame: .zero, textContainer: textContainer)
        textView.autoresizingMask = .width
        textView.backgroundColor = NSColor.textBackgroundColor
        textView.drawsBackground = true
        textView.isHorizontallyResizable = false
        textView.isVerticallyResizable = true
        textView.maxSize = NSSize(width: CGFloat.greatestFiniteMagnitude, height: CGFloat.greatestFiniteMagnitude)
        textView.minSize = NSSize(width: 0, height: scrollView.contentSize.height)
        textView.textColor = NSColor.labelColor
        textView.delegate = context.coordinator
        textView.string = model.textDocument!
        textView.allowsUndo = true
        
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
