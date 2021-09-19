import Foundation
import SwiftLockbookCore

class CoreService: ObservableObject {
    let core: LockbookApi

    init(_ core: LockbookApi) {
        self.core = core
    }
}
