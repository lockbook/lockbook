import SwiftUI
import SwiftLockbookCore

@main struct LockbookApp: App {

    @Environment(\.scenePhase) private var scenePhase
    
    var body: some Scene {

        WindowGroup {
            AppView()
                .realDI()
                .buttonStyle(PlainButtonStyle())    
                .ignoresSafeArea()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .onBackground {
                    DI.sync.sync()
                }
                .onForeground {
                    DI.sync.sync()
                }
        }.commands {
            CommandGroup(replacing: CommandGroupPlacement.newItem) {
            }
            CommandMenu("Lockbook") {
                Button("Sync", action: { DI.sync.sync() }).keyboardShortcut("S", modifiers: .command)
            }
            SidebarCommands()
        }
        
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

extension View {
    #if os(iOS)
    func onBackground(_ f: @escaping () -> Void) -> some View {
        self.onReceive(
            NotificationCenter.default.publisher(for: UIApplication.willResignActiveNotification),
            perform: { _ in f() }
        )
    }
    
    func onForeground(_ f: @escaping () -> Void) -> some View {
        self.onReceive(
            NotificationCenter.default.publisher(for: UIApplication.didBecomeActiveNotification),
            perform: { _ in f() }
        )
    }
    #else
    func onBackground(_ f: @escaping () -> Void) -> some View {
        self.onReceive(
            NotificationCenter.default.publisher(for: NSApplication.willResignActiveNotification),
            perform: { _ in f() }
        )
    }
    
    func onForeground(_ f: @escaping () -> Void) -> some View {
        self.onReceive(
            NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification),
            perform: { _ in f() }
        )
    }
    #endif
}
