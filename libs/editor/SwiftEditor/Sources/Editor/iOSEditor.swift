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
}
#endif
