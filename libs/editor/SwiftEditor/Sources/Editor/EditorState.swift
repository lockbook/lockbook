import SwiftUI
import Combine

public class EditorState: ObservableObject {
    
    @Published public var text: String
    @Published public var reload: Bool = false
        
    
    public init(text: String) {
        self.text = text
    }
    
    deinit {
        print("bye editor state")
    }
}

public class ToolbarState: ObservableObject {
    @Published public var isBulletListSelected: Bool = false
    @Published public var isNumberListSelected: Bool = false
    @Published public var isTodoListSelected: Bool = false
    @Published public var isHeadingSelected: Bool = false
    @Published public var isInlineCodeSelected: Bool = false
    @Published public var isBoldSelected: Bool = false
    @Published public var isItalicSelected: Bool = false
    
    public var toggleBulletList: () -> Void = {}
    public var toggleNumberList: () -> Void = {}
    public var toggleTodoList: () -> Void = {}
    public var toggleHeading: (UInt32) -> Void = {_ in }
    public var toggleInlineCode: () -> Void = {}
    public var toggleBold: () -> Void = {}
    public var toggleItalic: () -> Void = {}
    public var tab: (Bool) -> Void = {_ in }
    
    public init() {
        print("initing toolbar state")
    }
    
    deinit {
        print("bye toolbar state")
    }
}

public class NameState: ObservableObject {
    @Published public var potentialTitle: String? = nil
    @Published public var focusLocation: MarkdownEditorFocus = .editor
    
    public init() {
        print("initing name state")
    }

    deinit {
        print("bye name state")
    }
}
