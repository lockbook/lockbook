import SwiftUI

struct SettingsView: View {
    
    @EnvironmentObject var accounts: AccountService
    @EnvironmentObject var settings: SettingsService

    var body: some View {
        switch accounts.account {
        case .none:
            VStack(spacing: 20) {
                Text("You need an account for settings!")
                Text("Default API: \(ConfigHelper.safeGet(.apiLocation) ?? "None!")")
            }.padding(100)
        case .some(let account):
            TabView {
                AccountSettingsView(account: account)
                    .tabItem {
                        Label("Account", systemImage: "person")
                    }
                UsageSettingsView()
                    .tabItem {
                        Label("Usage", systemImage: "externaldrive")
                    }
                ManageSubscriptionView()
                    .tabItem {
                        Label("Premium", systemImage: "banknote")
                    }
                DebugView()
                    .tabItem {
                        Label("Debug", systemImage: "hammer")
                    }
            }
            .padding(20)
            .frame(width: 600)

        }
    }
}
