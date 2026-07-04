import SwiftUI
import SwiftWorkspace

struct HomeView: View {
    @Environment(\.isPreview) private var isPreview

    @State private var homeState = HomeState()
    @State private var selectedTab: SidebarTab = .files
    #if os(iOS)
        @State private var showSettings = false
    #endif

    @State private var filesModel: FilesModel
    @State private var fileTreeModel: FileTreeModel
    @State private var sharedTreeModel: FileTreeModel
    @State private var recentsModel = RecentsModel()
    #if os(iOS)
        @State private var searchModel: SearchModel
    #endif

    @State private var workspaceInput = WorkspaceInputState(coreHandle: AppState.lb.lbUnsafeRawPtr)
    @State private var workspaceOutput = WorkspaceOutputState()

    init() {
        let filesModel = FilesModel()

        _filesModel = State(initialValue: filesModel)
        _fileTreeModel = State(initialValue: FileTreeModel(filesModel: filesModel))
        _sharedTreeModel = State(initialValue: FileTreeModel(filesModel: filesModel))
        #if os(iOS)
            _searchModel = State(initialValue: SearchModel(filesModel: filesModel))
        #endif
    }

    var body: some View {
        @Bindable var homeState = homeState

        NavigationSplitView(
            columnVisibility: $homeState.splitViewVisibility,
            preferredCompactColumn: $homeState.compactColumn
        ) {
            sidebar
                .navigationSplitViewColumnWidth(min: 250, ideal: 300)
        } detail: {
            NavigationStack {
                workspace
            }
        }
        .environment(homeState)
        .environment(filesModel)
        .environment(workspaceInput)
        .environment(workspaceOutput)
        .onChange(of: workspaceOutput.openDoc) { _, openDoc in
            guard let openDoc else { return }
            fileTreeModel.docOpened(openDoc)
            sharedTreeModel.docOpened(openDoc)
        }
        .onChange(of: workspaceOutput.selectedFolder) { _, selectedFolder in
            guard let selectedFolder else { return }
            fileTreeModel.folderSelected(selectedFolder)
            sharedTreeModel.folderSelected(selectedFolder)
        }
        .onChange(of: AppState.lb.events.metadataVersion) {
            filesModel.loadFiles()
        }
        .onChange(of: AppState.lb.events.status) { _, status in
            filesModel.recomputeStatusDots(status: status)
        }
    }

    private var sidebar: some View {
        VStack(spacing: 0) {
            #if os(macOS)
                sidebarActions
            #endif

            sidebarContent
        }
    }

    private var sidebarContent: some View {
        Group {
            switch selectedTab {
            case .files:
                FileTreeView(fileTreeModel: fileTreeModel)
            case .recents:
                RecentsView(model: recentsModel)
            case .sharedWithMe:
                SharedWithMeView(fileTreeModel: sharedTreeModel)
            #if os(iOS)
                case .search:
                    SearchTabView(model: searchModel)
            #endif
            }
        }
        .toolbar {
            ToolbarItem(placement: tabstripPlacement) {
                Picker("Tabs", selection: $selectedTab) {
                    ForEach([SidebarTab.files, .recents, .sharedWithMe]) { tab in
                        Label(tab.title, systemImage: tab.systemImage)
                            .tag(tab)
                    }
                }
                .pickerStyle(.segmented)
                .fixedSize()
            }

            #if os(iOS)
                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        showSettings = true
                    } label: {
                        Image(systemName: "gearshape")
                    }
                }

                ToolbarItem(placement: .topBarTrailing) {
                    Button {
                        homeState.splitViewVisibility = .detailOnly
                        homeState.compactColumn = .detail
                    } label: {
                        Image(systemName: "sidebar.left")
                            .imageScale(.large)
                    }
                }

                ToolbarItem(placement: .bottomBar) {
                    Button {
                        selectedTab = .search
                    } label: {
                        Image(systemName: "magnifyingglass")
                    }
                }

                ToolbarSpacer(.flexible, placement: .bottomBar)

                ToolbarItem(placement: .bottomBar) {
                    Button {
                        createDocInRoot()
                    } label: {
                        Image(systemName: "square.and.pencil")
                    }
                }
            #endif
        }
        #if os(iOS)
            .sheet(isPresented: $showSettings) {
                NavigationStack {
                    SettingsView()
                }
            }
        #endif
    }

    #if os(macOS)
        private var sidebarActions: some View {
            HStack(spacing: 8) {
                actionChip("New", systemImage: "square.and.pencil") {
                    createDocInRoot()
                }

                actionChip("Search", systemImage: "magnifyingglass") {
                    workspaceInput.showSearch()
                }
            }
            .padding(.horizontal, 10)
            .padding(.vertical, 8)
        }

        private func actionChip(
            _ title: String, systemImage: String, action: @escaping () -> Void
        ) -> some View {
            Button(action: action) {
                Label(title, systemImage: systemImage)
                    .font(.callout)
                    .frame(maxWidth: .infinity)
                    .padding(.vertical, 6)
                    .background(.quaternary.opacity(0.6), in: RoundedRectangle(cornerRadius: 7))
                    .contentShape(RoundedRectangle(cornerRadius: 7))
            }
            .buttonStyle(.plain)
        }
    #endif

    private func createDocInRoot() {
        guard let root = filesModel.root else { return }

        workspaceInput.createDocAt(parent: root.id, drawing: false)
        homeState.compactColumn = .detail
    }

    private var tabstripPlacement: ToolbarItemPlacement {
        #if os(macOS)
            .principal
        #else
            .topBarLeading
        #endif
    }

    @ViewBuilder
    private var workspace: some View {
        if isPreview {
            Text("Workspace")
                .frame(maxWidth: .infinity, maxHeight: .infinity)
        } else {
            #if os(iOS)
                WorkspaceView()
            #else
                WorkspaceView(workspaceInput, workspaceOutput, AppState.lb.lbUnsafeRawPtr)
            #endif
        }
    }
}

enum SidebarTab: CaseIterable, Identifiable {
    case files
    case recents
    case sharedWithMe
    #if os(iOS)
        case search
    #endif

    var id: Self {
        self
    }

    var title: String {
        switch self {
        case .files: "Files"
        case .recents: "Recents"
        case .sharedWithMe: "Shared"
        #if os(iOS)
            case .search: "Search"
        #endif
        }
    }

    var systemImage: String {
        switch self {
        case .files: "folder.fill"
        case .recents: "clock.fill"
        case .sharedWithMe: "person.2.fill"
        #if os(iOS)
            case .search: "magnifyingglass"
        #endif
        }
    }
}

#Preview {
    HomeView()
}
