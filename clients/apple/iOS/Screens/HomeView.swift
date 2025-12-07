import Introspect
import SwiftUI
import SwiftWorkspace

struct HomeView: View {
    @Environment(\.horizontalSizeClass) var horizontalSizeClass

    @EnvironmentObject var filesModel: FilesViewModel
    @EnvironmentObject var workspaceInput: WorkspaceInputState

    @StateObject var homeState: HomeState
    @StateObject var settingsModel = SettingsViewModel()

    init(workspaceOutput: WorkspaceOutputState, filesModel: FilesViewModel) {
        self._homeState = StateObject(
            wrappedValue: HomeState(
                workspaceOutput: workspaceOutput,
                filesModel: filesModel
            )
        )
    }

    var body: some View {
        Group {
            if horizontalSizeClass == .compact {
                DrawerView(
                    homeState: homeState,
                    mainView: {
                        detail
                    },
                    sideView: {
                        sidebar
                    }
                )
            } else {
                PathSearchContainerView(
                    filesModel: filesModel,
                    workspaceInput: workspaceInput
                ) {
                    NavigationSplitView(
                        columnVisibility: homeState.splitViewVisibility,
                        sidebar: {
                            SearchContainerView(filesModel: filesModel) {
                                sidebar
                                    .introspectSplitViewController {
                                        splitView in
                                        self.syncFloatingState(
                                            splitView: splitView
                                        )
                                    }
                            }
                        },
                        detail: {
                            NavigationStack {
                                detail
                            }
                        }
                    )
                }
            }
        }
        .onChange(
            of: horizontalSizeClass,
            perform: { newValue in
                if newValue == .compact {
                    DispatchQueue.main.async {
                        homeState.isSidebarFloating = true
                    }
                }
            }
        )
        .environmentObject(homeState)
        .environmentObject(settingsModel)
    }

    @ViewBuilder
    var sidebar: some View {
        SidebarView()
            .modifier(OutOfSpaceAlert())
            .selectFolderSheets()
    }

    @ViewBuilder
    var detail: some View {
        DetailView()
            .navigationDestination(isPresented: $homeState.showSettings) {
                SettingsView(model: settingsModel)
            }
    }

    func syncFloatingState(splitView: UISplitViewController) {
        let isFloating =
            splitView.displayMode == .oneOverSecondary
            || splitView.displayMode == .twoOverSecondary

        if homeState.isSidebarFloating != isFloating {
            DispatchQueue.main.async {
                homeState.isSidebarFloating = isFloating
            }
        }
    }
}

struct SidebarView: View {
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var filesModel: FilesViewModel
    @EnvironmentObject var workspaceInput: WorkspaceInputState
    @EnvironmentObject var workspaceOutput: WorkspaceOutputState

    var body: some View {
        if let error = filesModel.error {
            Text(error)
                .foregroundStyle(.red)
        } else if filesModel.loaded {
            TabView {
                Tab("Home", systemImage: "house") {
                    NavigationStack {
                        SearchContainerView(filesModel: filesModel) {
                            HomeSubView()
                        }
                    }
                    .overlay(
                        alignment: .bottom,
                        content: {
                            StatusBarView()
                        }
                    )
                }

                Tab("Shares", systemImage: "person.2.fill") {
                    NavigationStack {
                        PendingSharesView(
                            filesModel: filesModel,
                            workspaceInput: workspaceInput,
                            workspaceOutput: workspaceOutput
                        )
                    }
                }
            }
            .tabViewStyle(.sidebarAdaptable)
        } else {
            ProgressView()
        }
    }
}

struct HomeSubView: View {
    @EnvironmentObject var homeState: HomeState
    @EnvironmentObject var filesModel: FilesViewModel
    @EnvironmentObject var workspaceInput: WorkspaceInputState
    @EnvironmentObject var workspaceOutput: WorkspaceOutputState

    var body: some View {
        Group {
            if let root = filesModel.root {
                Form {
                    CollapsableSection(
                        id: "Suggested_Docs",
                        label: {
                            Text("Suggested")
                                .bold()
                                .foregroundColor(.primary)
                                .textCase(.none)
                                .font(.headline)
                                .padding(.bottom, 10)
                                .padding(.top, 8)
                        },
                        content: {
                            SuggestedDocsView(filesModel: filesModel)
                        }
                    )

                    Section(
                        header: Text("Files")
                            .bold()
                            .foregroundColor(.primary)
                            .textCase(.none)
                            .font(.headline)
                            .padding(.bottom, 3)
                            .padding(.top, 8)
                    ) {
                        FileTreeView(
                            root: root,
                            filesModel: filesModel,
                            workspaceInput: workspaceInput,
                            workspaceOutput: workspaceOutput
                        )
                        .toolbar {
                            selectionToolbarItem
                        }
                    }
                    .padding(.horizontal, 16)
                }
                .formStyle(.columns)
                .environmentObject(filesModel)
                .navigationTitle(root.name)
                .navigationBarTitleDisplayMode(.large)
            }
        }
        .toolbar {
            ToolbarItemGroup(placement: .topBarTrailing) {
                Button {
                    homeState.sheetInfo = .importPicker
                } label: {
                    Label("Import", systemImage: "square.and.arrow.down.fill")
                }

                Button {
                    homeState.sidebarState = .closed
                    homeState.showSettings = true
                } label: {
                    Label("Settings", systemImage: "gearshape.fill")
                }
            }
        }
    }

    var selectionToolbarItem: ToolbarItem<(), Button<AnyView>> {
        switch filesModel.selectedFilesState {
        case .selected(explicitly: _, implicitly: _):
            ToolbarItem(placement: .topBarLeading) {
                Button(
                    action: {
                        withAnimation {
                            filesModel.selectedFilesState = .unselected
                        }
                    },
                    label: {
                        AnyView(
                            Text("Done")
                        )
                    }
                )
            }
        case .unselected:
            ToolbarItem(placement: .topBarLeading) {
                Button(
                    action: {
                        withAnimation(.linear(duration: 0.2)) {
                            filesModel.selectedFilesState = .selected(
                                explicitly: [],
                                implicitly: []
                            )
                        }
                    },
                    label: {
                        AnyView(
                            Label("Edit", systemImage: "filemenu.and.selection")
                        )
                    }
                )
            }
        }
    }
}

#Preview("Home View") {
    HomeView(workspaceOutput: .preview, filesModel: .preview)
        .withCommonPreviewEnvironment()
}
