import SwiftUI

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
            HomeBillingWrapperView()
        } else {
            OnboardingView()
        }
    }
}

struct HomeBillingWrapperView: View {
    @StateObject var billingState = BillingState()
    
    var body: some View {
        HomeView()
            .environmentObject(billingState)
    }
}

#Preview("Logged In") {
    ContentView(isLoggedIn: .constant(true))
}

#Preview("Onboarding") {
    ContentView(isLoggedIn: .constant(false))
}
