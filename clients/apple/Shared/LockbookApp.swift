import SwiftUI
import SwiftWorkspace

@main struct LockbookApp: App {
    
    @State private var isLoggedIn: Bool = AppState.shared.isLoggedIn
    @State private var isPreview: Bool = false
    
    var body: some Scene {
        WindowGroup {
            ContentView(isLoggedIn: $isLoggedIn)
                .environmentObject(AppState.shared)
                .mapState(AppState.shared.$isLoggedIn, to: $isLoggedIn)
        }
        
        #if os(macOS)
        Settings {
//            TODO: checkout `.scenePadding()`
//            SettingsView()
        }
        #endif
    }
}

struct ContentView: View {
    @Binding var isLoggedIn: Bool
    
    var body: some View {
        if isLoggedIn {
            HomeContextWrapper()
        } else {
            OnboardingView()
        }
    }
}

struct HomeContextWrapper: View {
    @StateObject var billingState = BillingState()
    @StateObject var workspaceState = WorkspaceState()
    
    var body: some View {
        HomeView(workspaceState: workspaceState)
            .environmentObject(billingState)
            .environmentObject(workspaceState)
    }
}

#Preview("Logged In") {
    ContentView(isLoggedIn: .constant(true))
}

#Preview("Onboarding") {
    ContentView(isLoggedIn: .constant(false))
}
