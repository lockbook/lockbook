import Foundation
import SwiftUI

public struct EditorView: View {
    
    @State var editorState: EditorState
    var toolbarState: ToolbarState
    var nameState: NameState
    
    @FocusState var focused: Bool
    private let metalView: MetalView
    
    public init(_ editorState: EditorState, _ toolbarState: ToolbarState, _ nameState: NameState) {
        self.editorState = editorState
        self.toolbarState = toolbarState
        self.nameState = nameState
        
        self.metalView = MetalView(editorState: editorState, toolbarState: toolbarState, nameState: nameState)
    }
    
    public var body: some View {
        metalView
    }
}

public enum MarkdownEditorFocus {
    case editor
    case title
}
