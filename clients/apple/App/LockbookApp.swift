import SwiftUI

let mainWindowId = "main"

@main
struct LockbookApp: App {
    @State private var billingState = BillingState()

    @Environment(\.openWindow) private var openWindow

    var body: some Scene {
        WindowGroup(id: mainWindowId) {
            ContentView()
                .environment(billingState)
        }
        .commands {
            CommandGroup(replacing: .newItem) {
                Button("New File") {
                    NotificationCenter.default.post(name: .createNewFile, object: nil)
                }
                .keyboardShortcut("n", modifiers: .command)

                #if os(macOS)
                    Button("New Window") {
                        openWindow(id: mainWindowId)
                    }
                    .keyboardShortcut("n", modifiers: [.command, .shift])
                #endif
            }
        }

        WindowGroup(id: documentWindowId, for: UUID.self) { $fileId in
            if let fileId {
                DocumentWindowView(fileId: fileId)
            }
        }
        #if os(macOS)
            .windowStyle(.hiddenTitleBar)
            .defaultSize(width: 640, height: 760)
        #endif

        #if os(macOS)
            Settings {
                SettingsView()
                    .environment(billingState)
            }
        #endif
    }
}

extension Notification.Name {
    static let createNewFile = Notification.Name("createNewFile")
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
