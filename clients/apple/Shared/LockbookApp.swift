import SwiftUI
import SwiftLockbookCore

@main
struct LockbookApp: App {
    #if os(macOS)
    @StateObject var core = Core(documenstDirectory: FileManager.default.homeDirectoryForCurrentUser.path + "/.lockbook")
    #else
    @StateObject var core = Core(documenstDirectory: FileManager.default.urls(for: .documentDirectory, in: .userDomainMask).last!.path)
    #endif
    
    var body: some Scene {
        #if os(macOS)
        WindowGroup {
            AppView(core: core)
                .buttonStyle(PlainButtonStyle())
                .ignoresSafeArea()
        }
        
        Settings {
            SettingsView(core: core)
        }
        #else
        WindowGroup {
            AppView(core: core)
                .ignoresSafeArea()
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
}
