import Foundation
import SwiftUI
import BackgroundTasks

#if os(macOS)
import AppKit 
#endif

@main struct A: App {
    @Environment(\.scenePhase) private var scenePhase
    
    #if os(macOS)
    @NSApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    #else
    @UIApplicationDelegateAdaptor(AppDelegate.self) var appDelegate
    #endif
    
    var app: some View {
        AppView()
            .realDI()
            .buttonStyle(PlainButtonStyle())
            .frame(maxWidth: .infinity, maxHeight: .infinity)
            .registeriOSBackgroundTasks(scenePhase: scenePhase, appDelegate: appDelegate)
    }
    
    var window: some Scene {
        #if os(macOS)
        Window("Lockbook", id: "main") {
            app
        }
        #else
        WindowGroup {
            app
        }
        #endif
    }
    
    var body: some Scene {
        window
        .commands {
            CommandGroup(replacing: .saveItem) {}
            
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
                Button("Sync", action: { DI.workspace.requestSync() }).keyboardShortcut("S", modifiers: .command)
                Button("Search Paths", action: { DI.search.startSearchThread(isPathAndContentSearch: false) }).keyboardShortcut("O", modifiers: .command)
                Button("Copy file link", action: {
                    if let id = DI.workspace.openDoc {
                        DI.files.copyFileLink(id: id)
                    }
                }).keyboardShortcut("L", modifiers: [.command, .shift])
                
                #if os(macOS)
                Button("Logout", action: {
                    WindowManager.shared.openLogoutConfirmationWindow()
                })
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
    func registeriOSBackgroundTasks(scenePhase: ScenePhase, appDelegate: AppDelegate) -> some View {
        #if os(iOS)
        self
            .onChange(of: scenePhase, perform: { newValue in
                switch newValue {
                case .background:
                    appDelegate.scheduleBackgroundTask(initialRun: true)
                case .active:
                    appDelegate.endBackgroundTasks()
                default:
                    break
                }
            })
        #else
        self
        #endif
    }
    
}

#if os(macOS)

class AppDelegate: NSObject, NSApplicationDelegate, NSWindowDelegate {
    func applicationShouldTerminateAfterLastWindowClosed(_ sender: NSApplication) -> Bool {
        return true
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
            
            DispatchQueue.main.async { [weak self] in
                DI.sync.backgroundSync(onSuccess: {
                    task.setTaskCompleted(success: true)

                    self?.scheduleBackgroundTask(initialRun: false)
                }, onFailure: {
                    task.setTaskCompleted(success: false)

                    self?.scheduleBackgroundTask(initialRun: false)
                })
                
                self?.scheduleBackgroundTask(initialRun: false)
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
