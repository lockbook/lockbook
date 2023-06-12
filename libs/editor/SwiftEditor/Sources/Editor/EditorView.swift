import Foundation
import SwiftUI

public struct EditorView: View {
    
    @State var editorState: EditorState
    @FocusState var focused: Bool
    private let metalView: MetalView
    
    public init(_ editorState: EditorState) {
        self.editorState = editorState
        self.metalView = MetalView(editorState: editorState)
    }
    
    public var body: some View {
        metalView
            .focused($focused)
            .onAppear {
                focused = true
            }
    }

    public func header(headingSize: UInt32) {
        metalView.header(headingSize: headingSize)
    }

    public func bulletedList() {
        metalView.bulletedList()
    }

    public func numberedList() {
        metalView.numberedList()
    }

    public func todoList() {
        metalView.todoList()
    }

    public func bold() {
        metalView.bold()
    }

    public func italic() {
        metalView.italic()
    }

    public func inlineCode() {
        metalView.inlineCode()
    }

    #if os(iOS)
    public func tab(deindent: Bool) {
        metalView.tab(deindent: deindent)
    }
    #endif
}
