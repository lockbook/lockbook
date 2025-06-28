import SwiftUI
import SwiftWorkspace

@main struct LockbookApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
        .commands {
            // verify what shortcut its blocking
            CommandGroup(replacing: .saveItem) {}
            
            SidebarCommands()
        }
        
        #if os(macOS)
        Settings {
            SettingsView()
        }
        #endif
    }
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
    @StateObject var billingState = BillingState()
    
    var body: some View {
        HomeView()
            .environmentObject(billingState)
            .environmentObject(AppState.workspaceState)
    }
}

#Preview("Logged In") {
    ContentView()
}

#Preview("Onboarding") {
    ContentView()
}
