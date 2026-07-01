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
    @State private var requestedSearchMode: SearchMode?

    init(workspaceOutput: WorkspaceOutputState, filesModel: FilesViewModel) {
        _homeState = StateObject(
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
                                case .search:
                                    NavigationStack {
                                        SearchTabView(
                                            filesModel: filesModel,
                                            requestedMode: $requestedSearchMode
                                        )
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
                                    case .search:
                                        SearchTabView(
                                            filesModel: filesModel,
                                            requestedMode: $requestedSearchMode
                                        )
                                    }
                                }
                            )
                            .introspectSplitViewController {
                                splitView in
                                syncFloatingState(
                                    splitView: splitView
                                )
                            }
                            .navigationSplitViewColumnWidth(
                                min: 270, ideal: 340, max: 400
                            )
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
        .background(
            Button("Content search") {
                requestedSearchMode = .content
                selectedTab = .search
            }
            .keyboardShortcut("f", modifiers: [.command, .shift])
            .hidden()
        )
        .environmentObject(homeState)
        .environmentObject(settingsModel)
    }

    var filesHome: some View {
        FilesHomeView()
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

    var detail: some View {
        DetailView()
            .navigationDestination(isPresented: $homeState.showSettings) {
                SettingsView(model: settingsModel)
            }
            .navigationDestination(isPresented: $homeState.showUpgradeAccount) {
                UpgradeAccountView(settingsModel: SettingsViewModel())
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
                ScrollView {
                    LazyVStack(
                        alignment: .leading,
                        pinnedViews: [.sectionHeaders]
                    ) {
                        if !filesModel.pinnedIds.isEmpty {
                            CollapsableSection(
                                id: "Pinned_Docs",
                                label: {
                                    Text("Pinned")
                                        .bold()
                                        .foregroundColor(.primary)
                                        .textCase(.none)
                                        .font(.headline)
                                        .padding(.bottom, 10)
                                        .padding(.top, 8)
                                },
                                content: {
                                    PinnedDocsView(filesModel: filesModel)
                                }
                            )
                        }

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
                }
                .refreshable {
                    if AppState.lb.events.status.outOfSpace {
                        DispatchQueue.main.asyncAfter(deadline: .now() + 1) {
                            homeState.triggerOutOfSpaceAlert()
                        }
                    }

                    workspaceInput.requestSync()
                }
                .environmentObject(filesModel)
                .navigationTitle(root.name)
                .navigationBarTitleDisplayMode(.large)
            } else {
                ProgressView()
            }
        }
        .toolbar {
            ToolbarItemGroup(placement: .secondaryAction) {
                if case .unselected = filesModel.selectedFilesState {
                    Button {
                        withAnimation(.linear(duration: 0.2)) {
                            filesModel.selectedFilesState = .selected(
                                explicitly: [],
                                implicitly: []
                            )
                        }
                    } label: {
                        Label("Edit", systemImage: "filemenu.and.selection")
                    }
                }

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

    @ToolbarContentBuilder
    var selectionToolbarItem: some ToolbarContent {
        if case .selected = filesModel.selectedFilesState {
            ToolbarItem(placement: .topBarLeading) {
                Button("Done") {
                    withAnimation {
                        filesModel.selectedFilesState = .unselected
                    }
                }
            }
        }
    }
}

#Preview("Home View") {
    HomeView(workspaceOutput: .preview, filesModel: .preview)
        .withCommonPreviewEnvironment()
}
