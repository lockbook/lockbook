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
