import Foundation
import SwiftUI
import MetalKit
import Combine

#if os(iOS)
public struct EditorView: UIViewRepresentable {
    
    @ObservedObject public var editorState: EditorState
    let mtkView: iOSMTK = iOSMTK()
    
    public init(_ editorState: EditorState, _ toolbarState: ToolbarState, _ nameState: NameState) {
        self.editorState = editorState
        mtkView.editorState = editorState
        mtkView.toolbarState = toolbarState
        mtkView.nameState = nameState
        
        mtkView.setInitialContent(editorState.text)
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
}
#else
public struct EditorView: NSViewRepresentable {
    
    @ObservedObject public var editorState: EditorState
    
    let mtkView: MacMTK = MacMTK()
    
    public init(_ editorState: EditorState, _ toolbarState: ToolbarState, _ nameState: NameState) {
        self.editorState = editorState
        mtkView.editorState = editorState
        mtkView.toolbarState = toolbarState
        mtkView.nameState = nameState
        
        mtkView.setInitialContent(editorState.text)
    }
    
    public func docChanged(_ s: String) {
        editorState.text = s
    }

    public func makeNSView(context: NSViewRepresentableContext<EditorView>) -> MTKView {
        mtkView
    }
    
    public func updateNSView(_ nsView: MTKView, context: NSViewRepresentableContext<EditorView>) {
        if editorState.reload {
            mtkView.updateText(editorState.text)
            editorState.reload = false
        }
    }
}
#endif





