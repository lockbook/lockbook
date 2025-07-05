import SwiftUI

struct OutOfSpaceAlert: ViewModifier {
    @EnvironmentObject var homeState: HomeState
    
    #if os(macOS)
    @Environment(\.openWindow) private var openWindow
    #endif
    
    func body(content: Content) -> some View {
        content
            .alert("You have run out of space", isPresented: $homeState.showOutOfSpaceAlert) {
                Button("Upgrade account") {
                    #if os(iOS)
                    homeState.showUpgradeAccount = true
                    #else
                    openWindow(id: "upgrade-account")
                    #endif
                }
                Button("Cancel", role: .cancel) {}
            } message: {
                Text("Purchase premium to access more space")
            }
            .navigationDestination(isPresented: $homeState.showUpgradeAccount) {
                UpgradeAccountView(settingsModel: SettingsViewModel())
            }
    }
}
