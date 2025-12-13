import Introspect
import SwiftUI
import SwiftWorkspace

struct HomeView: View {
    @Environment(\.horizontalSizeClass) var horizontalSizeClass

    @EnvironmentObject var filesModel: FilesViewModel
    @EnvironmentObject var workspaceInput: WorkspaceInputState
    @EnvironmentObject var workspaceOutput: WorkspaceOutputState

    @StateObject var homeState: HomeState
    @StateObject var settingsModel = SettingsViewModel()

    @State var selectedTab: TabType = .home

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
                        MobileCustomTabView(
                            selectedTab: $selectedTab,
                            tabContent: { tabType in
                                switch tabType {
                                    case .home:
                                    NavigationStack {
                                        filesHome
                                            .closeSidebarToolbar()
                                    }
                                case .sharedWithMe:
                                    NavigationStack {
                                        sharedWithMe
                                            .closeSidebarToolbar()
                                    }
                                }
                            }
                        )
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
                            CustomTabView(
                                selectedTab: $selectedTab,
                                tabContent: { tabType in
                                    switch tabType {
                                    case .home:
                                        filesHome
                                    case .sharedWithMe:
                                        sharedWithMe
                                    }
                                }
                            )
                            .introspectSplitViewController {
                                splitView in
                                self.syncFloatingState(
                                    splitView: splitView
                                )
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
        .environmentObject(homeState)
        .environmentObject(settingsModel)
    }

    @ViewBuilder
    var filesHome: some View {
        SearchContainerView(filesModel: filesModel) {
            FilesHomeView()
        }
        .overlay(
            alignment: .bottom,
            content: {
                VStack {
                    UsageBar()
                    
                    StatusBarView()
                }
            }
        )
        .modifier(OutOfSpaceAlert())
    }

    var sharedWithMe: some View {
        SharedWithMeView(
            filesModel: filesModel,
            workspaceInput: workspaceInput,
            workspaceOutput: workspaceOutput
        )
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

struct FilesHomeView: View {
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
            } else {
                ProgressView()
            }
        }
        .toolbar {
            ToolbarItemGroup(placement: .secondaryAction) {
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
                                .labelStyle(.titleOnly)
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
