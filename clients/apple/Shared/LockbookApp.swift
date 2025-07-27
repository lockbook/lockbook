import SwiftUI
import SwiftWorkspace

@main struct LockbookApp: App {
    var body: some Scene {
        #if os(macOS)
        macOS
        #else
        iOS
        #endif
    }
    
    #if os(macOS)
    @SceneBuilder
    var macOS: some Scene {
        Window("Lockbook", id: "main") {
            ContentView()
        }
        .windowToolbarStyle(.unifiedCompact)
        .commands {
            // verify what shortcut its blocking
            CommandGroup(replacing: .saveItem) {}
            
            SidebarCommands()
        }
        
        Settings {
            SettingsView()
                .environmentObject(AppState.billingState)
        }
        
        Window("Upgrade Account", id: "upgrade-account") {
            UpgradeAccountView(settingsModel: SettingsViewModel())
                .environmentObject(AppState.billingState)
        }
    }
    #else
    @SceneBuilder
    var iOS: some Scene {
        WindowGroup {
            ContentView()
        }
        .commands {
            // verify what shortcut its blocking
            CommandGroup(replacing: .saveItem) {}
            
            SidebarCommands()
        }
    }
    #endif
}

struct ContentView: View {
    @StateObject var appState = AppState.shared
    
    var body: some View {
        Group {
            if appState.isLoggedIn {
                HomeContextWrapper()
            } else {
                OnboardingView()
            }
        }
        .alert(item: $appState.error) { err in
            Alert(
                title: Text(err.title),
                message: Text(err.message),
                dismissButton: .default(Text("Ok"), action: {
                    AppState.shared.error = nil
                })
            )
        }
    }
}

struct HomeContextWrapper: View {
    var body: some View {
        HomeView()
            .environmentObject(AppState.billingState)
            .environmentObject(AppState.workspaceState)
    }
}

#Preview("Logged In") {
    ContentView()
}

#Preview("Onboarding") {
    ContentView()
}
