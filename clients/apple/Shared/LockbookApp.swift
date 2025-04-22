import SwiftUI
import SwiftWorkspace

@main struct LockbookApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
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
        if appState.isLoggedIn {
            HomeContextWrapper()
        } else {
            OnboardingView()
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
