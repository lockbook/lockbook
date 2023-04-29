import Combine
import SwiftLockbookCore
import Foundation

class BranchState: ObservableObject {
    @Published var open: Bool
    
    init(open: Bool) {
        self.open = open
    }
    
    init() {
        self.open = false
    }
}
