import SwiftUI
import SwiftLockbookCore

struct BottomBar: View {
    
    @ObservedObject var core: Core
    
    @State var work: Int = 0
    @State var offline: Bool = false
    @State var lastSynced = "moments ago"
    
    var onNewDocument: () -> Void = {}
    var onNewFolder: () -> Void = {}
    
    let calculateWorkTimer = Timer.publish(every: 2, on: .main, in: .common).autoconnect()
    let syncTimer = Timer.publish(every: 30, on: .main, in: .common).autoconnect()
    
    var menu: some View {
        Menu {
            Button(action: onNewDocument) {
                Label("Create a document", systemImage: "doc")
            }
            
            Button(action: onNewDocument) {
                Label("Create a folder", systemImage: "folder")
            }
        }
        label: {
            Label("Add", systemImage: "plus.circle.fill")
                .imageScale(.large)
                .frame(width: 40, height: 40)
        }
    }
    
    var body: some View {
        
        // If syncing, disable the sync button, and the ability to create new files
        if core.syncing {
            
            ProgressView()
            
            Spacer()
            Text("Syncing...")
                .foregroundColor(.secondary)
            Spacer()
            
            Label("Add", systemImage: "plus.circle.fill")
                .imageScale(.large)
                .frame(width: 40, height: 40)
                .foregroundColor(Color.gray)
            
        } else {
            if offline {
                Image(systemName: "xmark.icloud.fill")
                    .foregroundColor(Color.gray)
                
                Spacer()
                Text("Offline")
                    .foregroundColor(.secondary)
                    .onReceive(calculateWorkTimer) { _ in
                        checkForNewWork()
                    }
                Spacer()
                
                menu
            } else {
                Button(action: {
                    core.syncing = true
                    work = 0
                    
                }) {
                    Image(systemName: "arrow.triangle.2.circlepath.circle.fill")
                }
                
                Spacer()
                
                Text(work == 0 ? "Last synced: \(lastSynced)" : "\(work) items pending sync")
                    .foregroundColor(.secondary)
                    .onReceive(calculateWorkTimer) { _ in
                        checkForNewWork()
                    }
                    .onReceive(syncTimer) { _ in
                        core.syncing = true
                    }
                Spacer()
                menu
            }
        }
    }
    
    func checkForNewWork() {
        DispatchQueue.main.async {
            print("Checking")
            switch core.api.calculateWork() {
            case .success(let work):
                self.work = work.workUnits.count
            case .failure(let err):
                switch err.kind {
                case .UiError(let error):
                    if error == .CouldNotReachServer {
                        offline = true
                    }
                case .Unexpected(_):
                    core.handleError(err)
                }
            }
        }
    }
}

#if os(iOS)
struct SyncingPreview: PreviewProvider {
    
    static let core = Core()
    
    static var previews: some View {
        NavigationView {
            HStack {
            }.toolbar {
                ToolbarItemGroup(placement: .bottomBar) {
                    BottomBar(core: core)
                }
            }
        }.onAppear {
            core.syncing = true
        }
        
        
    }
}

struct NonSyncingPreview: PreviewProvider {
    
    static let core = Core()
    
    static var previews: some View {
        NavigationView {
            HStack {
            }.toolbar {
                ToolbarItemGroup(placement: .bottomBar) {
                    BottomBar(core: core)
                }
            }
        }.onAppear {
            core.syncing = false
        }
        
        
    }
}

struct OfflinePreview: PreviewProvider {
    
    static let core = Core()
    
    static var previews: some View {
        NavigationView {
            HStack {
            }.toolbar {
                ToolbarItemGroup(placement: .bottomBar) {
                    BottomBar(core: core, offline: true)
                }
            }
        }
        
        
    }
}

struct WorkItemsPreview: PreviewProvider {
    
    static let core = Core()
    
    static var previews: some View {
        NavigationView {
            HStack {
            }.toolbar {
                ToolbarItemGroup(placement: .bottomBar) {
                    BottomBar(core: core, work: 5)
                }
            }
        }
        
        
    }
}

#endif
