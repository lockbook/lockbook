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
                .handlesExternalEvents(preferring: ["*"], allowing: ["*"])
        }
        #if os(macOS)
            .handlesExternalEvents(matching: ["*"])
        #endif
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

            CommandGroup(after: .sidebar) {
                Button("Show Sidebar") {
                    NotificationCenter.default.post(name: .toggleSidebar, object: nil)
                }
                .keyboardShortcut("s", modifiers: .command)
            }

            #if os(iOS)
                CommandGroup(after: .textEditing) {
                    Button("Find in Document") {
                        NotificationCenter.default.post(name: .findInDocument, object: nil)
                    }
                    .keyboardShortcut("f", modifiers: .command)

                    Button("Search Everywhere") {
                        NotificationCenter.default.post(name: .searchEverywhere, object: nil)
                    }
                    .keyboardShortcut("f", modifiers: [.command, .shift])

                    Button("Open by Name") {
                        NotificationCenter.default.post(name: .openByName, object: nil)
                    }
                    .keyboardShortcut("o", modifiers: .command)
                }

                CommandGroup(replacing: .saveItem) {
                    Button("Close Tab") {
                        NotificationCenter.default.post(name: .closeActiveTab, object: nil)
                    }
                    .keyboardShortcut("w", modifiers: .command)
                }
            #endif
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
    static let toggleSidebar = Notification.Name("toggleSidebar")
    static let findInDocument = Notification.Name("findInDocument")
    static let searchEverywhere = Notification.Name("searchEverywhere")
    static let closeActiveTab = Notification.Name("closeActiveTab")
    static let focusSearchField = Notification.Name("focusSearchField")
    static let openByName = Notification.Name("openByName")
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
