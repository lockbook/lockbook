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
            }
                        
            CommandMenu("Lockbook") {
                Button("Sync", action: { DI.sync.sync() }).keyboardShortcut("S", modifiers: .command)
                Button("Search Paths", action: { DI.search.startSearchThread(isPathAndContentSearch: false) }).keyboardShortcut("O", modifiers: .command)
                #if os(macOS)
                Button("Search Paths And Content") {
                    if let toolbar = NSApp.keyWindow?.toolbar, let search = toolbar.items.first(where: { $0.itemIdentifier.rawValue == "com.apple.SwiftUI.search" }) as? NSSearchToolbarItem {
                        search.beginSearchInteraction()
                    }
                }.keyboardShortcut("f", modifiers: [.command, .shift])
                #endif
                
                Button("Copy file link", action: {
                    if let id = DI.workspace.openDoc {
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
                            if DI.files.idsAndFiles[id] != nil {
                                DI.workspace.openDoc = id
                            } else {
                                DI.errors.errorWithTitle("File not found", "That file does not exist in your lockbook")
                            }
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
