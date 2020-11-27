import SwiftUI
import SwiftLockbookCore

@main
struct LockbookApp: App {
    @StateObject var core = Core(documenstDirectory: ConfigHelper.getEnv(.lockbookLocation) ?? ConfigHelper.location)

    var body: some Scene {
        WindowGroup {
            AppView(core: core)
                .buttonStyle(PlainButtonStyle())
                .ignoresSafeArea()
        }.commands {
            CommandMenu("Lockbook") {
                Button("Sync", action: core.sync).keyboardShortcut("S", modifiers: .command)
            }
        }

        #if os(macOS)
        Settings {
            SettingsView(core: core)
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
