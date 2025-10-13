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
                NavigationStack {
                    NewDrawerView(
                        homeState: homeState,
                        mainView: {
                            detail
                        },
                        sideView: {
                            SearchContainerView(filesModel: filesModel) {
                                sidebar
                            }
                        }
                    )
                }
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
                                    .introspectSplitViewController(customize: {
                                        splitView in
                                        DispatchQueue.main.async {
                                            homeState.isSidebarFloating =
                                                splitView.displayMode
                                                == .oneOverSecondary
                                                || splitView.displayMode
                                                    == .twoOverSecondary
                                        }
                                    })
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
            .toolbar {
                ToolbarItemGroup(placement: .topBarTrailing) {
                    HStack(spacing: 0) {
                        Button {
                            homeState.sheetInfo = .importPicker
                        } label: {
                            Label(
                                "Import",
                                systemImage: "square.and.arrow.down.fill"
                            )
                        }

                        Button {
                            homeState.showPendingShares = true
                        } label: {
                            PendingSharesIcon(homeState: homeState)
                        }

                        Button {
                            homeState.showSettings = true
                        } label: {
                            Label("Settings", systemImage: "gearshape.fill")
                        }
                    }
                }
            }
            .modifier(OutOfSpaceAlert())
    }

    @ViewBuilder
    var detail: some View {
        DetailView()
            .navigationDestination(isPresented: $homeState.showSettings) {
                SettingsView(model: settingsModel)
            }
            .navigationDestination(isPresented: $homeState.showPendingShares) {
                PendingSharesView()
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

    @Environment(\.horizontalSizeClass) var horizontalSizeClass

    var body: some View {
        if let error = filesModel.error {
            Text(error)
                .foregroundStyle(.red)
        } else if filesModel.loaded {
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

                    Spacer()

                    VStack(spacing: 0) {
                        UsageBar()
                            .padding(.horizontal, 16)

                        StatusBarView()
                            .confirmationDialog(
                                "Are you sure? This action cannot be undone.",
                                isPresented: Binding(
                                    get: {
                                        filesModel.isMoreThanOneFileInDeletion()
                                    },
                                    set: { _ in
                                        filesModel.deleteFileConfirmation = nil
                                    }
                                ),
                                titleVisibility: .visible,
                                actions: {
                                    if let files = filesModel
                                        .deleteFileConfirmation
                                    {
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
                .navigationBarTitleDisplayMode(.large)
            }
        } else {
            ProgressView()
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
                            Label("Edit", image: "filemenu.and.selection")
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
