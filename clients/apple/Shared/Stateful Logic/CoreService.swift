import Foundation
import SwiftLockbookCore

class CoreService: ObservableObject {
    let api: LockbookApi
    
    init() {
        print("Fake core")
        self.api = FakeApi()
    }
    
    init(documentsDirectory: String) {
        print("core located at \(documentsDirectory)")
        self.api = CoreApi(documentsDirectory: documentsDirectory)
    }
}
