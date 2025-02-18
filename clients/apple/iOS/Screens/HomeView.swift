import SwiftUI
import SwiftWorkspace

struct HomeView: View {
    @StateObject var workspaceState = WorkspaceState()
    @StateObject var homeState = HomeState()
    
    @Environment(\.horizontalSizeClass) var horizontalSizeClass
    @Environment(\.isPreview) var isPreview
    
    var body: some View {
        NavigationStack {
            Group {
                if horizontalSizeClass == .compact {
                    DrawerView(isOpened: true, menu: {
                        sidebar
                    }, content: {
                        detail
                    })
                    .environment(\.isConstrainedLayout, true)
                } else {
                    NavigationSplitView(sidebar: {
                        sidebar
                    }, detail: {
                        detail
                    })
                    .environment(\.isConstrainedLayout, false)
                }
            }
        }
        .environmentObject(homeState)
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
                            Image(systemName: "gearshape.fill").foregroundColor(.accentColor)
                        })
                    }
                }
            }
    }
    
    @ViewBuilder
    var detail: some View {
        if isPreview {
            Text("This is a preview.")
        } else {
            WorkspaceView(workspaceState, AppState.lb.lbUnsafeRawPtr)
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
                                .environmentObject(workspaceState)
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
                                .environmentObject(workspaceState)
                                .environmentObject(filesModel)
                        }
                        .padding(.horizontal, 20)
                    
                    StatusBarView(filesModel: filesModel, workspaceState: workspaceState)
                }
                .navigationTitle(root.name)
                .navigationDestination(isPresented: $homeState.showSettings) {
                    SettingsView()
                }
                .navigationDestination(isPresented: $homeState.showPendingShares) {
                    PendingSharesView()
                        .environmentObject(filesModel)
                }
                .fileOpSheets(constrainedSheetHeight: $sheetHeight)
            }
        } else {
            ProgressView()
        }
    }
}

#Preview("Home View") {
    HomeView()
        .environmentObject(BillingState())
}
