import Foundation
import SwiftUI
import SwiftLockbookCore

#if os(macOS)
import AppKit
#endif

@main struct LockbookApp: App {

    @Environment(\.scenePhase) private var scenePhase
    
    #if os(macOS)
    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    #endif
    
    var body: some Scene {
        
        WindowGroup {
            AppView()
                .realDI()
                .buttonStyle(PlainButtonStyle())
                .ignoresSafeArea()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .onBackground {
                    DI.sync.sync()
                }
                .onForeground {
                    DI.sync.foregroundSync()
                }
        }.commands {
            CommandGroup(replacing: CommandGroupPlacement.newItem) {
                Button("New Doc", action: {
                    DI.files.createDoc(isDrawing: false)
                }).keyboardShortcut("N", modifiers: .command)
                
                #if os(iOS)
                Button("New Drawing", action: {
                    DI.files.createDoc(isDrawing: true)
                }).keyboardShortcut("N", modifiers: [.command, .control])
                #endif
                
                Button("New Folder", action: {
                    DI.sheets.creatingFolderInfo = CreatingFolderInfo(parentPath: DI.files.getPathByIdOrParent() ?? "Error", maybeParent: nil)
                }).keyboardShortcut("N", modifiers: [.command, .shift])
            }
            CommandMenu("Lockbook") {
                Button("Sync", action: { DI.sync.sync() }).keyboardShortcut("S", modifiers: .command)
                Button("Search Paths", action: { DI.search.startPathSearch() }).keyboardShortcut("O", modifiers: .command)
                #if os(macOS)
                Button("Search Paths And Content") {
                    if let toolbar = NSApp.keyWindow?.toolbar, let search = toolbar.items.first(where: { $0.itemIdentifier.rawValue == "com.apple.SwiftUI.search" }) as? NSSearchToolbarItem {
                            search.beginSearchInteraction()
                        }
                }.keyboardShortcut("f", modifiers: [.command, .shift])
                #endif
            }
            SidebarCommands()
        }
        
        #if os(macOS)
        Settings {
            SettingsView().realDI()
        }
        
        #endif
    }
}

extension View {
    func hideKeyboard() {
        #if os(iOS)
        UIApplication.shared.sendAction(#selector(UIResponder.resignFirstResponder), to: nil, from: nil, for: nil)
        #endif
    }
    
    /// Allows free use of .autocapitalization without having to if else it on macOS
    #if os(macOS)
    func autocapitalization(_ bunk: String?) -> some View {
        self
    }
    #endif
}

extension View {
    #if os(iOS)
    func onBackground(_ f: @escaping () -> Void) -> some View {
        self.onReceive(
            NotificationCenter.default.publisher(for: UIApplication.willResignActiveNotification),
            perform: { _ in f() }
        )
    }
    
    func onForeground(_ f: @escaping () -> Void) -> some View {
        self.onReceive(
            NotificationCenter.default.publisher(for: UIApplication.didBecomeActiveNotification),
            perform: { _ in f() }
        )
    }
    #else
    func onBackground(_ f: @escaping () -> Void) -> some View {
        self.onReceive(
            NotificationCenter.default.publisher(for: NSApplication.willResignActiveNotification),
            perform: { _ in f() }
        )
    }
    
    func onForeground(_ f: @escaping () -> Void) -> some View {
        self.onReceive(
            NotificationCenter.default.publisher(for: NSApplication.didBecomeActiveNotification),
            perform: { _ in f() }
        )
    }
    #endif
}

#if os(macOS)
class AppDelegate: NSObject, NSApplicationDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        return true
    }
}
#endif
