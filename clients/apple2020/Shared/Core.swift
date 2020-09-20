import Foundation
import SwiftLockbookCore
import SwiftUI

class Core: ObservableObject {
    let documenstDirectory: String
    let api: LockbookApi
    @Published var account: Account?
    @Published var message: Message? = nil
    
    func purge() {
        let lockbookDir = URL(fileURLWithPath: documenstDirectory).appendingPathComponent("lockbook.sled")
        if let _ = try? FileManager.default.removeItem(at: lockbookDir) {
            print("Deleted \(lockbookDir) and logging out")
            self.account = nil
        }
    }
    
    func displayError(error: ApplicationError) {
        switch error {
        case .Lockbook(_):
            self.message = Message(words: error.message(), icon: "xmark.shield.fill", color: .yellow)
        case .Serialization(_):
            self.message = Message(words: error.message(), icon: "square.fill.and.line.vertical.square.fill", color: .purple)
        case .State(_):
            self.message = Message(words: error.message(), icon: "burst.fill", color: .red)
        case .General(_):
            self.message = Message(words: error.message(), icon: "exclamationmark.square.fill", color: .red)
        }
    }
    
    init(documenstDirectory: String) {
        self.documenstDirectory = documenstDirectory
        let api = CoreApi(documentsDirectory: documenstDirectory)
        api.initializeLogger()
        switch api.getAccount() {
        case .success(let acc):
            self.account = acc
        case .failure(let err):
            print(err)
        }
        self.api = api
    }
    
    init() {
        self.documenstDirectory = "<USING-FAKE-API>"
        self.api = FakeApi()
    }
}



struct Message {
    let words: String
    let icon: String?
    let color: Color
}


