import SwiftUI
import Combine

public class EditorState: ObservableObject {
    @Published public var text: String
    @Published public var reload: Bool = false
    
    @Published public var isBulletListSelected: Bool = false
    @Published public var isNumberListSelected: Bool = false
    @Published public var isChecklistSelected: Bool = false
    @Published public var isHeadingSelected: Bool = false

    
    public init(text: String) {
        self.text = text
    }
    
    deinit {
        print("bye editor state")
    }
}
