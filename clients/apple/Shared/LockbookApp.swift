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
        WindowGroup {
            ContentView()
        }
        .windowToolbarStyle(.unifiedCompact)
        .commands {
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
    @StateObject var filesModel = FilesViewModel()
    @StateObject var workspaceInput = WorkspaceInputState()
    @StateObject var workspaceOutput = WorkspaceOutputState()
    
    var body: some View {
        HomeView(workspaceOutput: workspaceOutput, filesModel: filesModel)
            .environmentObject(AppState.billingState)
            .environmentObject(filesModel)
            .environmentObject(workspaceInput)
            .environmentObject(workspaceOutput)
    }
}

#Preview("Logged In") {
    ContentView()
}

#Preview("Onboarding") {
    ContentView()
}
