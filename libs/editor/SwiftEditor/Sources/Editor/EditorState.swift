import SwiftUI
import Combine

public class EditorState: ObservableObject {
    
    @Published public var name: String
    @Published public var text: String
    @Published public var reload: Bool = false
    
    @Published public var isBulletListSelected: Bool = false
    @Published public var isNumberListSelected: Bool = false
    @Published public var isTodoListSelected: Bool = false
    @Published public var isHeadingSelected: Bool = false
    @Published public var isInlineCodeSelected: Bool = false
    @Published public var isBoldSelected: Bool = false
    @Published public var isItalicSelected: Bool = false
    
    @Published public var potentialTitle: String? = nil
    
    @Published public var focusLocation: MarkdownEditorFocus = .editor
    
    public init(text: String, name: String) {
        self.text = text
        self.name = name
    }
    
    deinit {
        print("bye editor state")
    }
}
