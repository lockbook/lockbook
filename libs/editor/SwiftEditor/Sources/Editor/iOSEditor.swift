#if os(iOS)
import SwiftUI
import MetalKit
import Combine

public struct MetalView: UIViewRepresentable {
    
    @ObservedObject public var editorState: EditorState
    let mtkView: iOSMTK = iOSMTK()
    
    public init(editorState: EditorState) {
        self.editorState = editorState
        
        mtkView.setInitialContent(editorState.text)
        mtkView.editorState = editorState
    }

    public func makeUIView(context: Context) -> iOSMTK {
        mtkView
    }
    
    public func updateUIView(_ uiView: iOSMTK, context: Context) {
        if editorState.reload {
            mtkView.updateText(editorState.text)
            editorState.reload = false
        }
    }
    
    public func header() {
        mtkView.header()
    }
    
    public func bulletedList() {
        mtkView.bulletedList()
    }
    
    public func numberedList() {
        mtkView.numberedList()
    }
    
    public func checkedList() {
        mtkView.checkedList()
    }
    
    public func bold() {
        mtkView.bold()
    }
    
    public func italic() {
        mtkView.italic()
    }
    
    public func underline() {
        mtkView.underline()
    }
    
    public func tab() {
        mtkView.tab()
    }
}
#endif
