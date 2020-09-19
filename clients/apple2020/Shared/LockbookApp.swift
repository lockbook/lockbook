import SwiftUI
import SwiftLockbookCore

@main
struct LockbookApp: App {
    @StateObject var core = Core(documenstDirectory: FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).last!.path)
    
    var body: some Scene {
        switch core.account {
        case .none:
            return WindowGroup {
                Text("No account!")
            }
        case .some(let account):
            return WindowGroup {
                Text("Hello \(account.username)!")
            }
        }
    }
}
