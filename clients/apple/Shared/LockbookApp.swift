import SwiftUI
import SwiftLockbookCore

@main
struct LockbookApp: App {
    @StateObject var core = GlobalState(documenstDirectory: ConfigHelper.getEnv(.lockbookLocation) ?? ConfigHelper.location)
    
    var body: some Scene {
        let windowGroup = WindowGroup {
            AppView(core: core)
                .buttonStyle(PlainButtonStyle())
                .ignoresSafeArea()
        }.commands {
            CommandMenu("Lockbook") {
                Button("Sync", action: {core.syncing = true} ).keyboardShortcut("S", modifiers: .command)
                Button("New File", action: {} ).keyboardShortcut("N", modifiers: .command)
            }
            SidebarCommands()
        }
        
        #if os(macOS)
        windowGroup
            .windowStyle(HiddenTitleBarWindowStyle())
        Settings {
            SettingsView(core: core)
        }
        #else
        windowGroup
        #endif
    }
}

extension View {
    func hideKeyboard() {
        #if os(iOS)
        UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
        #endif
    }
    
    /// Allows free use of .autocapitalization without having to if else it on macOS
    #if os(macOS)
    func autocapitalization(_ bunk: String?) -> some View {
        self
    }
    #endif
}
