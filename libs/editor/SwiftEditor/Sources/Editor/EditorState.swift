import SwiftUI
import Combine

public class EditorState: ObservableObject {
    
    @Published public var text: String
    @Published public var reload: Bool = false
    @Published public var focused: Bool = true
    
    public var isiPhone: Bool
    
    public init(text: String, isiPhone: Bool) {
        self.text = text
        self.isiPhone = isiPhone
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
    
    public init() {}
}

public class NameState: ObservableObject {
    @Published public var potentialTitle: String? = nil
    
    public init() {}
}
