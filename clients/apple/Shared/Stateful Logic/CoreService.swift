import Foundation
import SwiftLockbookCore
import CLockbookCore

class CoreService: ObservableObject {
    let core: LockbookApi
    let corePtr: UnsafeMutableRawPointer
    
    
    init(_ core: LockbookApi) {
        self.core = core
        self.corePtr = get_core_ptr()
    }
}
