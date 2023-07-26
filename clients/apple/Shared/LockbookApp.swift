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
                
                Button("Close Tab", action: {
                    DI.currentDoc.closeDoc(DI.currentDoc.selectedDoc)
                }).keyboardShortcut("W", modifiers: .command)
            }
            
            CommandGroup(replacing: .textFormatting) {
                Button("Heading 1", action: {
                    DI.currentDoc.formatSelectedDocSelectedText(.Heading(1))
                }).keyboardShortcut("1", modifiers: [.command, .control])

                Button("Heading 2", action: {
                    DI.currentDoc.formatSelectedDocSelectedText(.Heading(2))
                }).keyboardShortcut("2", modifiers: [.command, .control])

                Button("Heading 3", action: {
                    DI.currentDoc.formatSelectedDocSelectedText(.Heading(3))
                }).keyboardShortcut("3", modifiers: [.command, .control])

                Button("Heading 4", action: {
                    DI.currentDoc.formatSelectedDocSelectedText(.Heading(4))
                }).keyboardShortcut("4", modifiers: [.command, .control])

                Button("Bold", action: {
                    DI.currentDoc.formatSelectedDocSelectedText(.Bold)
                }).keyboardShortcut("B", modifiers: .command)

                Button("Italic", action: {
                    DI.currentDoc.formatSelectedDocSelectedText(.Italic)
                }).keyboardShortcut("I", modifiers: .command)

                Button("Inline Code", action: {
                    DI.currentDoc.formatSelectedDocSelectedText(.InlineCode)
                }).keyboardShortcut("C", modifiers: [.command, .shift])
                
                Button("Number List", action: {
                    DI.currentDoc.formatSelectedDocSelectedText(.NumberList)
                }).keyboardShortcut("7", modifiers: [.command, .shift])
                
                Button("Bullet List", action: {
                    DI.currentDoc.formatSelectedDocSelectedText(.BulletList)
                }).keyboardShortcut("8", modifiers: [.command, .shift])
                
                Button("Todo List", action: {
                    DI.currentDoc.formatSelectedDocSelectedText(.TodoList)
                }).keyboardShortcut("9", modifiers: [.command, .shift])
            }
            
            CommandMenu("Tabs") {
                #if os(macOS)
                
                Button("Next Tab", action: {
                    DI.currentDoc.selectNextOpenDoc()
                }).keyboardShortcut("}", modifiers: [.command, .shift])
                
                Button("Previous Tab", action: {
                    DI.currentDoc.selectPreviousOpenDoc()
                }).keyboardShortcut("{", modifiers: [.command, .shift])
                
                #else
                
                Button("Next Tab", action: {
                    DI.currentDoc.selectNextOpenDoc()
                }).keyboardShortcut("]", modifiers: [.command, .shift])
                
                Button("Previous Tab", action: {
                    DI.currentDoc.selectPreviousOpenDoc()
                }).keyboardShortcut("[", modifiers: [.command, .shift])
                
                #endif
                
                Menu("Go to Tab") {
                    Button("Open Tab 1", action: {
                        DI.currentDoc.selectOpenDocByIndex(index: 0)
                    }).keyboardShortcut("1", modifiers: .command)
                    
                    Button("Open Tab 2", action: {
                        DI.currentDoc.selectOpenDocByIndex(index: 1)
                    }).keyboardShortcut("2", modifiers: .command)
                    
                    Button("Open Tab 3", action: {
                        DI.currentDoc.selectOpenDocByIndex(index: 2)
                    }).keyboardShortcut("3", modifiers: .command)
                    
                    Button("Open Tab 4", action: {
                        DI.currentDoc.selectOpenDocByIndex(index: 3)
                    }).keyboardShortcut("4", modifiers: .command)
                    
                    Button("Open Tab 5", action: {
                        DI.currentDoc.selectOpenDocByIndex(index: 4)
                    }).keyboardShortcut("5", modifiers: .command)
                    
                    Button("Open Tab 6", action: {
                        DI.currentDoc.selectOpenDocByIndex(index: 5)
                    }).keyboardShortcut("6", modifiers: .command)
                    
                    Button("Open Tab 7", action: {
                        DI.currentDoc.selectOpenDocByIndex(index: 6)
                    }).keyboardShortcut("7", modifiers: .command)
                    
                    Button("Open Tab 8", action: {
                        DI.currentDoc.selectOpenDocByIndex(index: 7)
                    }).keyboardShortcut("8", modifiers: .command)
                    
                    Button("Open Last Tab", action: {
                        DI.currentDoc.selectOpenDocByIndex(index: 8)
                    }).keyboardShortcut("9", modifiers: .command)
                }
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
