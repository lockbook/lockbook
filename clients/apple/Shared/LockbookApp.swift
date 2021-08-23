import SwiftUI
import SwiftLockbookCore

@main struct LockbookApp: App {

    
    var body: some Scene {

        let windowGroup = WindowGroup {
            AppView()
                .buttonStyle(PlainButtonStyle())
                .ignoresSafeArea()
                .realDI()
        }.commands {
            CommandMenu("Lockbook") {
                Button("Sync", action: { DI.sync.sync() }).keyboardShortcut("S", modifiers: .command)
            }
            SidebarCommands()
        }
        
        windowGroup
        
        #if os(macOS)
        Settings {
            SettingsView().realDI()
        }
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
