import SwiftUI
import SwiftWorkspace

struct HomeView: View {
    @Environment(\.horizontalSizeClass) var horizontalSizeClass

    @StateObject var homeState = HomeState()
    @StateObject var filesModel = FilesViewModel()
    @StateObject var settingsModel = SettingsViewModel()
            
    var body: some View {
        Group {
            if horizontalSizeClass == .compact {
                NavigationStack {
                    NewDrawerView(homeState: homeState, mainView: {
                        detail
                    }, sideView: {
                        SearchContainerView(filesModel: filesModel) {
                            sidebar
                        }
                    })
                    .environment(\.isConstrainedLayout, true)
                }
            } else {
                PathSearchContainerView(filesModel: filesModel) {
                    NavigationSplitView(sidebar: {
                        SearchContainerView(filesModel: filesModel) {
                            sidebar
                        }
                    }, detail: {
                        NavigationStack {
                            detail
                        }
                    })
                    .environment(\.isConstrainedLayout, false)
                }
            }
        }
        .environmentObject(homeState)
        .environmentObject(filesModel)
        .environmentObject(settingsModel)
    }
    
    @ViewBuilder
    var sidebar: some View {
        SidebarView()
            .toolbar {
                ToolbarItemGroup(placement: .topBarTrailing) {
                    HStack(spacing: 0) {
                        Button(action: {
                            homeState.sheetInfo = .importPicker
                        }, label: {
                            Image(systemName: "square.and.arrow.down.fill")
                        })
                        
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
            .modifier(OutOfSpaceAlert())
    }
    
    @ViewBuilder
    var detail: some View {
        DetailView(homeState: homeState, filesModel: filesModel)
            .navigationDestination(isPresented: $homeState.showSettings) {
                SettingsView(model: settingsModel)
            }
            .navigationDestination(isPresented: $homeState.showPendingShares) {
                PendingSharesView()
            }
    }
}

struct SidebarView: View {
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var filesModel: FilesViewModel
        
    var body: some View {
        if let error = filesModel.error {
            Text(error)
                .foregroundStyle(.red)
        } else if filesModel.loaded {
            if let root = filesModel.root {
                Form {
                    CollapsableSection(id: "Suggested_Docs", label: {
                        Text("Suggested")
                            .bold()
                            .foregroundColor(.primary)
                            .textCase(.none)
                            .font(.headline)
                            .padding(.bottom, 10)
                            .padding(.top, 8)
                    }, content: {
                        SuggestedDocsView(filesModel: filesModel)
                    })
                    
                    Section(header: Text("Files")
                        .bold()
                        .foregroundColor(.primary)
                        .textCase(.none)
                        .font(.headline)
                        .padding(.bottom, 3)
                        .padding(.top, 8)) {
                            FileTreeView(root: root, filesModel: filesModel)
                                .toolbar {
                                    selectionToolbarItem
                                }
                        }
                        .padding(.horizontal, 16)
                    
                    Spacer()
                    
                    VStack(spacing: 0) {
                        UsageBar()
                            .padding(.horizontal, 16)
                        
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
                }
                .formStyle(.columns)
                .environmentObject(filesModel)
                .navigationTitle(root.name)
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
                        .foregroundStyle(Color.accentColor)
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
                        .foregroundStyle(Color.accentColor)
                })
            }
        }
    }
}

#Preview("Home View") {
    let workspaceState = WorkspaceState()
    
    return HomeView()
        .environmentObject(BillingState())
        .environmentObject(workspaceState)
}
