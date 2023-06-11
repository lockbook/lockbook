#if os(macOS)
import SwiftUI
import MetalKit
import Combine

public struct MetalView: NSViewRepresentable {
    
    @ObservedObject public var editorState: EditorState
    
    let mtkView: MacMTK = MacMTK()
    
    public init(editorState: EditorState) {
        self.editorState = editorState
        
        mtkView.setInitialContent(editorState.text)
        mtkView.editorState = editorState
    }
    
    public func docChanged(_ s: String) {
        editorState.text = s
    }

    public func makeNSView(context: NSViewRepresentableContext<MetalView>) -> MTKView {
        mtkView
    }
    
    public func updateNSView(_ nsView: MTKView, context: NSViewRepresentableContext<MetalView>) {
        if editorState.reload {
            mtkView.updateText(editorState.text)
            editorState.reload = false
        }
    }
    
    public func header(headingSize: UInt32) {
        mtkView.header(headingSize: headingSize)
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
    
    public func code() {
        mtkView.code()
    }
}
#endif
