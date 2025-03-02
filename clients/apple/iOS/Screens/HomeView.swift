import SwiftUI
import SwiftWorkspace

struct HomeView: View {
    @Environment(\.horizontalSizeClass) var horizontalSizeClass

    @EnvironmentObject var workspaceState: WorkspaceState
    @StateObject var homeState: HomeState
    
    init(workspaceState: WorkspaceState) {
        self._homeState = StateObject(wrappedValue: HomeState(workspaceState: workspaceState))
    }
        
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
                    .selectFolderSheets()
                }
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
    let workspaceState = WorkspaceState()
    
    return HomeView(workspaceState: workspaceState)
        .environmentObject(BillingState())
        .environmentObject(workspaceState)
}
