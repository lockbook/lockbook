import SwiftUI
import SwiftWorkspace

struct HomeView: View {
    @Environment(\.horizontalSizeClass) var horizontalSizeClass

    @StateObject var workspaceState = WorkspaceState()
    @StateObject var homeState = HomeState()
        
    var body: some View {
        NavigationStack {
            Group {
                if horizontalSizeClass == .compact {
                    DrawerView(homeState: homeState, menu: {
                        sidebar
                    }, content: {
                        DetailView()
                    })
                    .environment(\.isConstrainedLayout, true)
                } else {
                    NavigationSplitView(sidebar: {
                        sidebar
                    }, detail: {
                        DetailView()
                    })
                    .environment(\.isConstrainedLayout, false)
                }
            }
        }
        .environmentObject(homeState)
        .environmentObject(workspaceState)
    }
    
    @ViewBuilder
    var sidebar: some View {
        SidebarView(workspaceState)
            .toolbar {
                ToolbarItemGroup(placement: .topBarTrailing) {
                    HStack(spacing: 0) {
                        Button(action: {
                            homeState.showPendingShares = true
                        }, label: {
                            PendingSharesIcon(homeState: homeState)
                        })
                        
                        Button(action: {
                            homeState.showSettings = true
                        }, label: {
                            Image(systemName: "gearshape.fill")
                        })
                    }
                }
            }
    }
}

struct DetailView: View {
    @Environment(\.isPreview) var isPreview

    @EnvironmentObject var workspaceState: WorkspaceState
    @EnvironmentObject var homeState: HomeState
    
    @State var sheetHeight: CGFloat = 0

    var body: some View {
        Group {
            let _ = print(workspaceState.openTabs)
            if isPreview {
                Text("This is a preview.")
            } else {
                WorkspaceView(workspaceState, AppState.lb.lbUnsafeRawPtr)
            }
        }
        .toolbar {
            ToolbarItemGroup(placement: .topBarTrailing) {
                HStack(alignment: .center, spacing: 5) {
                    Button(action: {
                        self.runOnOpenDoc { file in
                            homeState.sheetInfo = .share(file: file)
                        }
                    }, label: {
                        Image(systemName: "person.wave.2.fill")
                    })
                    
                    Button(action: {
                        self.runOnOpenDoc { file in
                            exportFiles(homeState: homeState, files: [file])
                        }
                    }, label: {
                        Image(systemName: "square.and.arrow.up.fill")
                    })
                    
                    
                    if workspaceState.openTabs > 1 {
                        Button(action: {
                            self.showTabsSheet()
                        }, label: {
                            ZStack {
                                Label("Tabs", systemImage: "rectangle.fill")
                                
                                Text(workspaceState.openTabs < 100 ? String(workspaceState.openTabs) : ":D")
                                    .font(.callout)
                                    .foregroundColor(.white)
                            }
                        })
                        .foregroundColor(.blue)
                    }
                }
            }
        }
        .optimizedSheet(item: $homeState.tabsSheetInfo, constrainedSheetHeight: $sheetHeight) { info in
            TabsSheet(info: info.info)
        }
    }
    
    func showTabsSheet() {
            homeState.tabsSheetInfo = TabSheetInfo(info: workspaceState.getTabsIds().map({ id in
            switch AppState.lb.getFile(id: id) {
            case .success(let file):
                return (name: file.name, id: file.id)
            case .failure(_):
                return nil
            }
        }).compactMap({ $0 }))
    }
    
    func runOnOpenDoc(f: @escaping (File) -> Void) {
        guard let id = workspaceState.openDoc else {
            return
        }
        
        if let file =  try? AppState.lb.getFile(id: id).get() {
            f(file)
        }
    }
}

struct SidebarView: View {
    @EnvironmentObject var homeState: HomeState
    
    @ObservedObject var workspaceState: WorkspaceState
    @StateObject var filesModel: FilesViewModel
    @State var sheetHeight: CGFloat = 0
            
    init(_ workspaceState: WorkspaceState) {
        self.workspaceState = workspaceState
        self._filesModel = StateObject(wrappedValue: FilesViewModel(workspaceState: workspaceState))
    }
    
    var body: some View {
        if let error = filesModel.error {
            Text(error)
                .foregroundStyle(.red)
        } else if filesModel.loaded {
            if let root = filesModel.root {
                VStack(alignment: .leading) {
                    Section(header: Text("Suggested Docs")
                        .bold()
                        .foregroundColor(.primary)
                        .textCase(.none)
                        .font(.headline)
                        .padding(.bottom, 3)
                        .padding(.top, 8)) {
                            SuggestedDocsView(filesModel: filesModel)
                        }
                        .padding(.horizontal, 20)
                    
                    Section(header: Text("Files")
                        .bold()
                        .foregroundColor(.primary)
                        .textCase(.none)
                        .font(.headline)
                        .padding(.bottom, 3)
                        .padding(.top, 8)) {
                            FileTreeView(root: root, workspaceState: workspaceState, filesModel: filesModel)
                                .toolbar {
                                    selectionToolbarItem
                                }
                        }
                        .padding(.horizontal, 20)
                    
                    StatusBarView()
                    .fileOpSheets(workspaceState: workspaceState, constrainedSheetHeight: $sheetHeight)
                    .confirmationDialog(
                        "Are you sure? This action cannot be undone.",
                        isPresented: Binding(
                            get: { filesModel.isMoreThanOneFileInDeletion() },
                            set: { _ in filesModel.deleteFileConfirmation = nil }
                        ),
                        titleVisibility: .visible,
                        actions: {
                            if let files = filesModel.deleteFileConfirmation {
                                DeleteConfirmationButtons(files: files)
                            }
                        }
                    )
                }
                .environmentObject(workspaceState)
                .environmentObject(filesModel)
                .navigationTitle(root.name)
                .navigationDestination(isPresented: $homeState.showSettings) {
                    SettingsView()
                }
                .navigationDestination(isPresented: $homeState.showPendingShares) {
                    PendingSharesView()
                        .environmentObject(filesModel)
                }
            }
        } else {
            ProgressView()
        }
    }
    
    var selectionToolbarItem: ToolbarItem<(), Button<some View>> {
        switch filesModel.selectedFilesState {
        case .selected(explicitly: _, implicitly: _):
            ToolbarItem(placement: .topBarLeading) {
                Button(action: {
                    withAnimation {
                        filesModel.selectedFilesState = .unselected
                    }
                }, label: {
                    Text("Done")
                        .foregroundStyle(.blue)
                })
            }
        case .unselected:
            ToolbarItem(placement: .topBarLeading) {
                Button(action: {
                    withAnimation(.linear(duration: 0.2)) {
                        filesModel.selectedFilesState = .selected(explicitly: [], implicitly: [])
                    }
                }, label: {
                    Text("Edit")
                        .foregroundStyle(.blue)
                })
            }
        }
    }
}

#Preview("Home View") {
    HomeView()
        .environmentObject(BillingState())
}
