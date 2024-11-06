import Foundation
import SwiftWorkspace

class CoreService: ObservableObject {
    let core: Lb
    let corePtr: UnsafeMutableRawPointer
    
    
    init(_ core: Lb) {
        self.core = core
        self.corePtr = UnsafeMutableRawPointer(core.lb!)
    }
}
