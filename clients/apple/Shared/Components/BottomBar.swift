import SwiftUI
import SwiftLockbookCore

struct BottomBar: View {

    @ObservedObject var core: GlobalState

    @State var work: Int = 0
    @State var offline: Bool = false
    @State var lastSynced = "moments ago"

    #if os(iOS)
    var onNewDocument: () -> Void = {}
    var onNewFolder: () -> Void = {}
    #endif

    let calculateWorkTimer = Timer.publish(every: 3, on: .main, in: .common).autoconnect()
    let syncTimer = Timer.publish(every: 300, on: .main, in: .common).autoconnect()

    #if os(iOS)
    var menu: AnyView {
        if core.syncing {
            return AnyView(Label("Add", systemImage: "plus.circle.fill")
                            .imageScale(.large)
                            .frame(width: 40, height: 40)
                            .foregroundColor(Color.gray))
        } else {
            return AnyView(Menu {
                Button(action: onNewDocument) {
                    Label("Create a document", systemImage: "doc")
                }

                Button(action: onNewFolder) {
                    Label("Create a folder", systemImage: "folder")
                }
            }
            label: {
                Label("Add", systemImage: "plus.circle.fill")
                    .imageScale(.large)
                    .frame(width: 40, height: 40)
            }
            )
        }
    }
    #endif

    /// TODO: Consider syncing onAppear here

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
                    work = 0
                }) {
                    Image(systemName: "arrow.triangle.2.circlepath.circle.fill")
                })
            }
        }
    }
    #else
    var syncButton: AnyView {
        if core.syncing || offline {

            return AnyView(
                Text("")
                    .font(.callout)
                    .foregroundColor(Color.gray)
            )

        } else {
            return AnyView(Button(action: {
                core.syncing = true
                work = 0
            }) {
                Text("Sync now")
                    .font(.callout)
                    .foregroundColor(Color.init(red: 0.3, green: 0.45, blue: 0.79))
            })
        }
    }
    #endif

    var statusText: AnyView {
        if core.syncing {
            return AnyView(Text("Syncing...")
                            .foregroundColor(.secondary))
        } else {
            if offline {
                return AnyView(Text("Offline")
                                .foregroundColor(.secondary)
                                .onReceive(calculateWorkTimer) { _ in
                                    checkForNewWork()
                                })
            } else {
                return AnyView(
                    Text(work == 0 ? "Last synced: \(lastSynced)" : "\(work) items pending sync")
                        .font(.callout)
                        .foregroundColor(.secondary)
                        .bold()
                        .onReceive(calculateWorkTimer) { _ in
                            checkForNewWork()
                        }
                        .onReceive(syncTimer) { _ in
                            core.syncing = true
                        }
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

    func checkForNewWork() {
        DispatchQueue.global(qos: .background).async {
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

#if(iOS)
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
                    BottomBar(core: core, work: 5)
                }
            }
        }


    }
}
#endif
