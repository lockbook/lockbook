import SwiftUI
import Foundation

struct DebugView: View {
    
    @EnvironmentObject var accounts: AccountService
    
    var body: some View {
        HStack {
            Text("Username")
            Text(accounts.account?.username)
        }
        
        HStack {
            Text("Version")
            Text(version())
        }
    }
    
    func version() -> String {
        switch Bundle.main.infoDictionary?["CFBundleShortVersionString"] as? String {
        case .some(let v):
            return v
        case .none:
            return "None: CFBundleShortVersionString"
        }
    }
}
