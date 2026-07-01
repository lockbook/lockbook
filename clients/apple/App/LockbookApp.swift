import SwiftUI
import SwiftWorkspace

@main
struct LockbookApp: App {
    var body: some Scene {
        WindowGroup {
            ContentView()
        }
    }
}

struct ContentView: View {
    @State private var appState = AppState.shared

    var body: some View {
        Group {
            if appState.isLoggedIn {
                SignedInView()
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

struct SignedInView: View {
    var body: some View {
        VStack(spacing: 12) {
            Image(systemName: "checkmark.seal.fill")
                .font(.system(size: 48))
                .foregroundStyle(.tint)
            Text("Signed in")
                .font(.title)
            if let username = AppState.shared.account?.username {
                Text(username)
                    .font(.title3)
                    .foregroundStyle(.secondary)
            }
            Button("Sign out (local)") {
                AppState.shared.isLoggedIn = false
            }
            .padding(.top)
        }
        .padding()
        .frame(maxWidth: .infinity, maxHeight: .infinity)
    }
}

#Preview {
    ContentView()
}
