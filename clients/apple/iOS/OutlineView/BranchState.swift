import Combine
import SwiftLockbookCore

class BranchState: ObservableObject {
    @Published var open: Bool = false
}
