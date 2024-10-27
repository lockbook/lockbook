import Foundation
import SwiftWorkspace

class CoreService: ObservableObject {
    let core: Lb
    let corePtr: UnsafeMutableRawPointer
    
    
    init(_ core: LockbookApi) {
        self.core = core
        self.corePtr = get_core_ptr()
    }
}
