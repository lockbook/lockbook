import SwiftUI

struct SettingsView: View {
    @ObservedObject var core: Core
    var body: some View {
        switch core.account {
        case .none:
            AnyView(VStack(spacing: 20) {
                Text("You need an account for settings!")
                Text("Default API: \(ConfigHelper.safeGet(.apiLocation) ?? "None!")")
            }.padding(100))
        case .some(let account):
            AnyView(AccountView(core: core, account: account).buttonStyle(PlainButtonStyle()).padding(100))
        }
    }
}

struct SettingsView_Previews: PreviewProvider {
    static var previews: some View {
        SettingsView(core: .init())
    }
}
