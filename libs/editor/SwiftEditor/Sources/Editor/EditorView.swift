import Foundation
import SwiftUI

public struct EditorView: View {
    
    @State var editorState: EditorState
    @FocusState var focused: Bool
    
    public init(_ editorState: EditorState) {
        self.editorState = editorState
    }
    
    public var body: some View {
        metalView
            .focused($focused)
            .onAppear {
                focused = true
            }
    }
    
    public var metalView: MetalView {
        MetalView(editorState: editorState)
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
    
    public func checkedList() {
        metalView.checkedList()
    }
    
    public func bold() {
        metalView.bold()
    }
    
    public func italic() {
        metalView.italic()
    }
    
    public func tab() {
        metalView.tab()
    }
}
