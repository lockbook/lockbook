import SwiftUI

@main struct LockbookApp: App {
    
    @State private var isLoggedIn: Bool = MainState.shared.isLoggedIn
    
    var body: some Scene {
        WindowGroup {
            ContentView(isLoggedIn: $isLoggedIn)
                .environmentObject(MainState.shared)
                .mapState(MainState.shared.$isLoggedIn, to: $isLoggedIn)
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
            OnboardingView()
        } else {
            PlatformView()
        }
    }
}

#Preview("Logged In") {
    ContentView(isLoggedIn: .constant(true))
}

#Preview("Onboarding") {
    ContentView(isLoggedIn: .constant(false))
}
