import SwiftUI
import SwiftLockbookCore

struct SettingsView: View {
    @ObservedObject var core: GlobalState
    
    var body: some View {
        switch core.account {
        case .none:
            VStack(spacing: 20) {
                Text("You need an account for settings!")
                Text("Default API: \(ConfigHelper.safeGet(.apiLocation) ?? "None!")")
            }.padding(100)
        case .some(let account):
            TabView {
                AccountSettingsView(core: core, account: account)
                    .tabItem {
                        Label("Account", systemImage: "person")
                    }
                UsageSettingsView(core: core)
                    .tabItem {
                        Label("Usage", systemImage: "externaldrive")
                    }
            }
            .padding(20)
            .frame(width: 600)
            
        }
        
    }
}

