import Foundation
import SwiftUI

public struct EditorView: View {
    
    @State var editorState: EditorState
    @FocusState var focused: Bool
    
    public init(_ editorState: EditorState) {
        self.editorState = editorState
    }
    
    public var body: some View {
        MetalView(editorState: editorState)
            .focused($focused)
            .onAppear {
                focused = true
            }
    }
}
