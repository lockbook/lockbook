import Foundation
import SwiftUI
import SwiftLockbookCore
import BackgroundTasks

#if os(macOS)
import AppKit
#endif

@main struct LockbookApp: App {
    @Environment(\.scenePhase) private var scenePhase
    
    #if os(macOS)
    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    #else
    @UIApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    #endif
    
    @StateObject var search = DI.search
        
    var body: some Scene {
        WindowGroup {
            AppView()
                .realDI()
                .buttonStyle(PlainButtonStyle())
                .ignoresSafeArea()
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .registerBackgroundTasks(scenePhase: scenePhase, appDelegate: appDelegate)
                .onOpenURL() { url in
                    onUrlOpen(url: url)
                }
                .handlesExternalEvents(preferring: ["lb"], allowing: ["lb"])
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
                Menu("Headings") {
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
                }

                Button("Bold", action: {
                    DI.currentDoc.formatSelectedDocSelectedText(.Bold)
                }).keyboardShortcut("B", modifiers: .command)

                Button("Italic", action: {
                    DI.currentDoc.formatSelectedDocSelectedText(.Italic)
                }).keyboardShortcut("I", modifiers: .command)

                Button("Inline Code", action: {
                    DI.currentDoc.formatSelectedDocSelectedText(.InlineCode)
                }).keyboardShortcut("C", modifiers: [.command, .shift])
                
                Button("Strikethrough", action: {
                    DI.currentDoc.formatSelectedDocSelectedText(.Strikethrough)
                }).keyboardShortcut("S", modifiers: [.command, .shift])
                
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
                    })
                    .keyboardShortcut("1", modifiers: .command)
                    .disabled(search.pathSearchState != .NotSearching)
                    
                    Button("Open Tab 2", action: {
                        DI.currentDoc.selectOpenDocByIndex(index: 1)
                    })
                    .keyboardShortcut("2", modifiers: .command)
                    .disabled(search.pathSearchState != .NotSearching)
                    
                    Button("Open Tab 3", action: {
                        DI.currentDoc.selectOpenDocByIndex(index: 2)
                    })
                    .keyboardShortcut("3", modifiers: .command)
                    .disabled(search.pathSearchState != .NotSearching)
                    
                    Button("Open Tab 4", action: {
                        DI.currentDoc.selectOpenDocByIndex(index: 3)
                    })
                    .keyboardShortcut("4", modifiers: .command)
                    .disabled(search.pathSearchState != .NotSearching)
                    
                    Button("Open Tab 5", action: {
                        DI.currentDoc.selectOpenDocByIndex(index: 4)
                    })
                    .keyboardShortcut("5", modifiers: .command)
                    .disabled(search.pathSearchState != .NotSearching)
                    
                    Button("Open Tab 6", action: {
                        DI.currentDoc.selectOpenDocByIndex(index: 5)
                    })
                    .keyboardShortcut("6", modifiers: .command)
                    .disabled(search.pathSearchState != .NotSearching)
                    
                    Button("Open Tab 7", action: {
                        DI.currentDoc.selectOpenDocByIndex(index: 6)
                    })
                    .keyboardShortcut("7", modifiers: .command)
                    .disabled(search.pathSearchState != .NotSearching)
                    
                    Button("Open Tab 8", action: {
                        DI.currentDoc.selectOpenDocByIndex(index: 7)
                    })
                    .keyboardShortcut("8", modifiers: .command)
                    .disabled(search.pathSearchState != .NotSearching)
                    
                    Button("Open Last Tab", action: {
                        DI.currentDoc.selectOpenDocByIndex(index: 8)
                    })
                    .keyboardShortcut("9", modifiers: .command)
                    .disabled(search.pathSearchState != .NotSearching)
                }
            }
            
            CommandMenu("Search Bar") {
                Button("Open result 1", action: {
                    DI.search.openPathAtIndex(index: 0)
                })
                .keyboardShortcut("1", modifiers: .command)
                .disabled(search.pathSearchState == .NotSearching)
                
                Button("Open result 2", action: {
                    DI.search.openPathAtIndex(index: 1)
                })
                .keyboardShortcut("2", modifiers: .command)
                .disabled(search.pathSearchState == .NotSearching)
                
                Button("Open result 3", action: {
                    DI.search.openPathAtIndex(index: 2)
                })
                .keyboardShortcut("3", modifiers: .command)
                .disabled(search.pathSearchState == .NotSearching)
                
                Button("Open result 4", action: {
                    DI.search.openPathAtIndex(index: 3)
                })
                .keyboardShortcut("4", modifiers: .command)
                .disabled(search.pathSearchState == .NotSearching)
                
                Button("Open result 5", action: {
                    DI.search.openPathAtIndex(index: 4)
                })
                .keyboardShortcut("5", modifiers: .command)
                .disabled(search.pathSearchState == .NotSearching)
                
                Button("Open result 6", action: {
                    DI.search.openPathAtIndex(index: 5)
                })
                .keyboardShortcut("6", modifiers: .command)
                .disabled(search.pathSearchState == .NotSearching)
                
                Button("Open result 7", action: {
                    DI.search.openPathAtIndex(index: 6)
                })
                .keyboardShortcut("7", modifiers: .command)
                .disabled(search.pathSearchState == .NotSearching)
                
                Button("Open result 8", action: {
                    DI.search.openPathAtIndex(index: 7)
                })
                .keyboardShortcut("8", modifiers: .command)
                .disabled(search.pathSearchState == .NotSearching)
                
                Button("Open result 9", action: {
                    DI.search.openPathAtIndex(index: 8)
                })
                .keyboardShortcut("9", modifiers: .command)
                .disabled(search.pathSearchState == .NotSearching)
                
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
                
                Button("Copy file link", action: {
                    if let id = DI.currentDoc.selectedDoc {
                        DI.files.copyFileLink(id: id)
                    }
                }).keyboardShortcut("L", modifiers: [.command, .shift])
            }
            SidebarCommands()
        }
        
        #if os(macOS)
        Settings {
            SettingsView().realDI()
        }
        
        #endif
    }

    func onUrlOpen(url: URL) {
        DispatchQueue.global(qos: .userInitiated).async {
            if url.scheme == "lb" {
                if let uuidString = url.host,
                   let id = UUID(uuidString: uuidString) {
                    while true {
                        if DI.accounts.account == nil && DI.accounts.calculated {
                            return
                        }
                        
                        if DI.files.root != nil {
                            if let meta = DI.files.idsAndFiles[id] {
                                Thread.sleep(until: .now + 0.1)
                                DispatchQueue.main.sync {
                                    var laterOpenForIphone = false
                                    if let docInfo = DI.currentDoc.openDocuments.values.first,
                                       docInfo.isiPhone {
                                        docInfo.dismissForLink = meta
                                        laterOpenForIphone.toggle()
                                    }
                                    
                                    DI.currentDoc.cleanupOldDocs()
                                    if !laterOpenForIphone {
                                        DI.currentDoc.justOpenedLink = meta
                                    }
                                    
                                    DI.currentDoc.openDoc(id: id)
                                    DI.currentDoc.setSelectedOpenDocById(maybeId: id)
                                }
                            } else {
                                DI.errors.errorWithTitle("File not found", "That file does not exist in your lockbook")
                            }
                            
                            return
                        }
                    }
                } else {
                    DI.errors.errorWithTitle("Malformed link", "Cannot open file")
                }
            } else {
                DI.errors.errorWithTitle("Error", "An unexpected error has occurred")
            }
        }
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
    func registerBackgroundTasks(scenePhase: ScenePhase, appDelegate: AppDelegate) -> some View {
        #if os(iOS)
        self
            .onChange(of: scenePhase, perform: { newValue in
                switch newValue {
                case .background:
                    if !DI.onboarding.initialSyncing {
                        appDelegate.scheduleBackgroundTask(initialRun: true)
                    }
                case .active:
                    appDelegate.endBackgroundTasks()
                default:
                    break
                }
            })
        #else
        self
            .onReceive(
                NotificationCenter.default.publisher(for: NSApplication.willResignActiveNotification),
                perform: { _ in
                    if !DI.onboarding.initialSyncing {
                        appDelegate.scheduleBackgroundTask(initialRun: true)
                    }
                })
            .onReceive(
                NotificationCenter.default.publisher(for: NSApplication.willBecomeActiveNotification),
                perform: { _ in
                    appDelegate.endBackgroundTasks()
                })
        #endif
    }
    
}

#if os(macOS)

class AppDelegate: NSObject, NSApplicationDelegate {
    let backgroundSyncStartSecs = 60 * 5
    let backgroundSyncContSecs = 60 * 60
    
    var currentSyncTask: DispatchWorkItem? = nil
    
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        return true
    }
        
    func scheduleBackgroundTask(initialRun: Bool) {
        let newSyncTask = DispatchWorkItem {
            DI.sync.backgroundSync(onSuccess: {
                self.scheduleBackgroundTask(initialRun: false)
            }, onFailure: {
                self.scheduleBackgroundTask(initialRun: false)
            })
        }
        
        DispatchQueue.main.asyncAfter(deadline: .now() + .seconds((initialRun ? backgroundSyncStartSecs : backgroundSyncContSecs)), execute: newSyncTask)
        
        currentSyncTask = newSyncTask
    }
    
    func endBackgroundTasks() {
        currentSyncTask?.cancel()
    }
}

#else

class AppDelegate: NSObject, UIApplicationDelegate {
    
    let backgroundSyncStartSecs = 60.0 * 5
    let backgroundSyncContSecs = 60.0 * 60
    
    let backgroundSyncIdentifier = "app.lockbook.backgroundSync"

    func application(_ application: UIApplication, didFinishLaunchingWithOptions launchOptions: [UIApplication.LaunchOptionsKey : Any]? = nil) -> Bool {
        self.registerBackgroundTask()
        
        return true
    }
    
    func registerBackgroundTask() {
        BGTaskScheduler.shared.register(forTaskWithIdentifier: backgroundSyncIdentifier, using: nil) { task in
            task.expirationHandler = {
                task.setTaskCompleted(success: false)
            }
            
            DispatchQueue.main.async {
                DI.sync.backgroundSync(onSuccess: {
                    task.setTaskCompleted(success: true)

                    self.scheduleBackgroundTask(initialRun: false)
                }, onFailure: {
                    task.setTaskCompleted(success: false)

                    self.scheduleBackgroundTask(initialRun: false)
                })
                
                self.scheduleBackgroundTask(initialRun: false)
            }
        }
    }
    
    func scheduleBackgroundTask(initialRun: Bool) {
        let request = BGProcessingTaskRequest(identifier: backgroundSyncIdentifier)
        request.earliestBeginDate = Date(timeIntervalSinceNow: initialRun ? backgroundSyncStartSecs : backgroundSyncContSecs)
        request.requiresExternalPower = false
        request.requiresNetworkConnectivity = true
        
        do {
            try BGTaskScheduler.shared.submit(request)
            print("scheduled background task")
            
        } catch {
            print("could not schedule background task")
        }
    }
    
    func endBackgroundTasks() {
        BGTaskScheduler.shared.cancelAllTaskRequests()
    }
}

#endif
