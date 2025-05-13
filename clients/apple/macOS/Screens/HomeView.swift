import SwiftUI
import SwiftWorkspace

struct HomeView: View {
    @StateObject var homeState = HomeState()
    @StateObject var filesModel = FilesViewModel()

    var body: some View {
        PathSearchContainerView {
            NavigationSplitView(sidebar: {
                SearchContainerView {
                    SidebarView()
                }
            }, detail: {
                NavigationStack {
                    DetailView()
                        .navigationDestination(isPresented: $homeState.showPendingShares) {
                            PendingSharesView()
                        }
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
    
    var body: some View {
        if isPreview {
            Text("This is a preview.")
        } else {
            WorkspaceView(AppState.workspaceState, AppState.lb.lbUnsafeRawPtr)
                .modifier(OnLbLinkViewModifier())
                .toolbar {
                    HStack(alignment: .bottom, spacing: 5) {
                        if workspaceState.openDoc != nil {
                            Button(action: {
                                runOnOpenDoc { file in
                                    homeState.sheetInfo = .share(file: file)
                                }
                            }, label: {
                                Image(systemName: "person.wave.2.fill")
                                    .foregroundStyle(Color.accentColor)
                            })
                            
                            Button(action: {
                                runOnOpenDoc { file in
                                    exportFiles(homeState: homeState, files: [file])
                                }
                            }, label: {
                                Image(systemName: "square.and.arrow.up.fill")
                                    .foregroundStyle(Color.accentColor)
                            })
                        }
                    }
                }
        }
    }
}

#Preview("Home View") {
    return HomeView()
        .environmentObject(AppState.workspaceState)
}
