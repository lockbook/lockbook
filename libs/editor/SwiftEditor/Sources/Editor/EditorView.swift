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
        
        if editorState.focused {
            mtkView.becomeFirstResponder()
            editorState.focused = false
        }
    }
}
#else
public struct EditorView: View {
    @FocusState var focused: Bool
    @ObservedObject var editorState: EditorState
    
    let nsEditorView: NSEditorView
    
    public init(_ editorState: EditorState, _ toolbarState: ToolbarState, _ nameState: NameState) {
        self.editorState = editorState
        nsEditorView = NSEditorView(editorState, toolbarState, nameState)
    }
    
    public var body: some View {
        nsEditorView
            .focused($focused)
            .onAppear {
                focused = true
            }
            .onChange(of: editorState.focused, perform: { newValue in
                if newValue {
                    focused = true
                }
            })

    }
}

public struct NSEditorView: NSViewRepresentable {
    
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

    public func makeNSView(context: NSViewRepresentableContext<NSEditorView>) -> MTKView {
        mtkView
    }
    
    public func updateNSView(_ nsView: MTKView, context: NSViewRepresentableContext<NSEditorView>) {
        if editorState.reload {
            mtkView.updateText(editorState.text)
            editorState.reload = false
        }
        
        if editorState.focused {
            editorState.focused = false
        }
    }
}
#endif





