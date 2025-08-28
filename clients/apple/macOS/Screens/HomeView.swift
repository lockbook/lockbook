import SwiftUI
import SwiftWorkspace

struct HomeView: View {
    @StateObject var homeState = HomeState()
    @StateObject var filesModel = FilesViewModel()

    var body: some View {
        PathSearchContainerView(filesModel: filesModel) {
            NavigationSplitView(sidebar: {
                SearchContainerView(filesModel: filesModel) {
                    SidebarView()
                }
            }, detail: {
                NavigationStack {
                    DetailView(homeState: homeState, filesModel: filesModel)
                        .navigationDestination(isPresented: $homeState.showPendingShares) {
                            PendingSharesView()
                        }
                        .modifier(OutOfSpaceAlert())
                }
            })
            .confirmationDialog(
                "Are you sure? This action cannot be undone.",
                isPresented: Binding(
                    get: { filesModel.deleteFileConfirmation != nil },
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
        .environmentObject(homeState)
        .environmentObject(filesModel)
    }
}

struct SidebarView: View {
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var filesModel: FilesViewModel
    @StateObject var settingsModel = SettingsViewModel()
    @Environment(\.colorScheme) var colorScheme

    var body: some View {
        if let error = filesModel.error {
            Text(error)
                .foregroundStyle(.red)
        } else if filesModel.loaded {
            Form {
                CollapsableSection(id: "Suggested_Docs", label: {
                    Label("Suggested Documents", systemImage: "books.vertical.fill")
                    .bold()
                    .font(.callout)
                }, content: {
                    SuggestedDocsView(filesModel: filesModel)
                })
                
                Section(header:
                    Label("Files", systemImage: "folder")
                    .bold()
                    .padding(.horizontal)
                    .font(.callout)
                    .padding(.top, 8)) {
                    FileTreeView()
                        .padding(.horizontal, 8)
                }
                
                Spacer()
                
                UsageBar()
                    .environmentObject(settingsModel)
                    .padding(.horizontal)
                    .padding(.top, 8)
                
                StatusBarView()
            }
            .formStyle(.columns)
            .selectFolderSheets()
            .fileOpSheets(constrainedSheetHeight: .constant(0))
            .toolbar {
                ToolbarItem(placement: .confirmationAction) {
                    Button(action: {
                        homeState.showPendingShares = true
                    }, label: {
                        PendingSharesIcon(homeState: homeState)
                    })
                    .buttonStyle(.plain)
                }
            }
        }
    }
}

struct DetailView: View {
    @Environment(\.isPreview) var isPreview
    
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var workspaceState: WorkspaceState
    @State var wrappedWorkspaceState: WrappedWorkspaceState
    
    init(homeState: HomeState, filesModel: FilesViewModel) {
        wrappedWorkspaceState = WrappedWorkspaceState(homeState: homeState, filesModel: filesModel)
    }
    
    var body: some View {
        if isPreview {
            Text("This is a preview.")
        } else {
            WorkspaceView(AppState.workspaceState, AppState.lb.lbUnsafeRawPtr)
                .modifier(OnLbLinkViewModifier())
        }
    }
}

#Preview("Home View") {
    return HomeView()
        .environmentObject(AppState.workspaceState)
}
