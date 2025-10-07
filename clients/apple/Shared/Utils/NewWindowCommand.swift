import SwiftUI

struct NewWindowCommand: Commands {
    var body: some Commands {
        CommandGroup(replacing: .newItem) {
            Button("New Window") {
                #if os(iOS)
                UIApplication.shared.requestSceneSessionActivation(nil, userActivity: nil, options: nil, errorHandler: nil)
                #elseif os(macOS)
                NSApp.sendAction(#selector(NSApplication.newWindowForTab(_:)), to: nil, from: nil)
                #endif
            }
            .keyboardShortcut("N", modifiers: [.command, .shift])
        }
    }
}
