import SwiftUI
import SwiftWorkspace

@main
struct LockbookApp: App {
    @State private var billingState = BillingState()

    var body: some Scene {
        WindowGroup {
            ContentView()
                .environment(billingState)
        }

        #if os(macOS)
            Settings {
                SettingsView()
                    .environment(billingState)
            }
        #endif
    }
}

struct ContentView: View {
    @State private var appState = AppState.shared

    var body: some View {
        Group {
            if appState.isLoggedIn {
                HomeView()
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

#Preview {
    ContentView()
}
