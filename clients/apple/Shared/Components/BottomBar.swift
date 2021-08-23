import SwiftUI
import SwiftLockbookCore

struct BottomBar: View {
    
    @EnvironmentObject var sync: SyncService
    @EnvironmentObject var status: StatusService
    
    #if os(iOS)
    var onCreating: () -> Void = {}
    #endif
    
    #if os(iOS)
    var menu: AnyView {
        AnyView(Button(action: {
            onCreating()
        }) {
            Image(systemName: "plus.circle.fill")
                .imageScale(.large)
                .foregroundColor(.blue)
                .frame(width: 40, height: 40, alignment: .center)
        })
    }
    #endif
    
    #if os(iOS)
    var syncButton: AnyView {
        if core.syncing {
            return AnyView(ProgressView())
        } else {
            if offline {
                return AnyView(Image(systemName: "xmark.icloud.fill")
                                .foregroundColor(Color.gray))
            } else {
                return AnyView(Button(action: {
                    core.syncing = true
                    core.work = 0
                }) {
                    Image(systemName: "arrow.triangle.2.circlepath.circle.fill")
                        .imageScale(.large)
                        .foregroundColor(.blue)
                        .frame(width: 40, height: 40, alignment: .center)
                })
            }
        }
    }
    #else
    var syncButton: AnyView {
        if sync.syncing || sync.offline {
            
            return AnyView(
                Text("")
                    .font(.callout)
                    .foregroundColor(Color.gray)
            )
            
        } else {
            return AnyView(Button(action: {
                sync.sync()
                status.work = 0
            }) {
                Text("Sync now")
                    .font(.callout)
                    .foregroundColor(Color.init(red: 0.3, green: 0.45, blue: 0.79))
            })
        }
    }
    #endif
    
    var localChangeText: String {
        if status.work == 0 { // not shown in this situation
            return ""
        } else if status.work == 1 {
            return "1 unsynced change"
        } else {
            return "\(status.work) unsynced changes"
        }
    }
    
    var statusText: AnyView {
        if sync.syncing {
            return AnyView(Text("Syncing...")
                            .foregroundColor(.secondary))
        } else {
            if sync.offline {
                return AnyView(Text("Offline")
                                .foregroundColor(.secondary)
                )
            } else {
                return AnyView(
                    Text(status.work == 0 ? "Last update: \(status.lastSynced)" : localChangeText)
                        .font(.callout)
                        .foregroundColor(.secondary)
                        .bold()
                )
            }
        }
    }
    
    var body: some View {
        #if os(iOS)
        syncButton
        Spacer()
        statusText
        Spacer()
        menu
        #else
        Divider()
        statusText
            .padding(4)
        syncButton
            .padding(.bottom, 7)
        #endif
    }
}

#if os(iOS)
struct SyncingPreview: PreviewProvider {
    
    static let core = GlobalState()
    
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
    
    static let core = GlobalState()
    
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
    
    static let core = GlobalState()
    
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
    
    static let core = GlobalState()
    
    static var previews: some View {
        NavigationView {
            HStack {
            }.toolbar {
                ToolbarItemGroup(placement: .bottomBar) {
                    BottomBar(core: core)
                        .onAppear {
                            core.work = 5
                        }
                }
            }
        }
        
        
    }
}
#endif
